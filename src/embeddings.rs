//! Gemini embeddings with a local vector cache. The key is read at runtime and only ever sent as a header.

use crate::js::{self, Object};
use crate::net;
use ring::digest::{digest, SHA256};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DIMENSIONS: usize = 768;
const BATCH_SIZE: usize = 100;
const TIMEOUT_MS: u64 = 30_000;
const MAX_ATTEMPTS: u32 = 4;
const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

pub type Vector = Vec<f32>;

fn model() -> &'static str {
    static MODEL: OnceLock<String> = OnceLock::new();
    MODEL.get_or_init(|| {
        std::env::var("CLAYUI_EMBED_MODEL").unwrap_or_else(|_| "gemini-embedding-001".into())
    })
}

/// Pure so tests can pass their own environment: `GEMINI_API_KEY`, then `GOOGLE_API_KEY` in the env file.
pub fn read_key_from(gemini_key: Option<String>, env_file: Option<String>) -> Option<String> {
    if let Some(key) = gemini_key.filter(|key| !key.is_empty()) {
        return Some(key);
    }
    let text = std::fs::read(env_file.filter(|file| !file.is_empty())?).ok()?;
    let text = String::from_utf8_lossy(&text).into_owned();
    let raw = js::lines(&text).find_map(|line| line.strip_prefix("GOOGLE_API_KEY="))?;
    let value = strip_quotes(js::trim(raw));
    (!value.is_empty()).then(|| value.to_string())
}

pub fn read_key() -> Option<String> {
    read_key_from(
        std::env::var("GEMINI_API_KEY").ok(),
        std::env::var("CLAYUI_GEMINI_ENV_FILE").ok(),
    )
}

/// `text.replace(/^["']|["']$/g, '')`.
pub fn strip_quotes(text: &str) -> &str {
    let quote = ['"', '\''];
    let without_start = text.strip_prefix(quote).unwrap_or(text);
    without_start.strip_suffix(quote).unwrap_or(without_start)
}

pub fn embed_text(item: &Object) -> String {
    let mut parts = Vec::new();
    let mut add = |value: Option<&Value>| {
        if js::truthy(value) {
            if let Some(value) = value {
                parts.push(js::string_of(value));
            }
        }
    };
    add(item.get("title"));
    add(item.get("description"));
    if let Some(Value::Array(keywords)) = item.get("keywords") {
        for keyword in keywords {
            add(Some(keyword));
        }
    }
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .replace(['-', '/'], " ");
    if !name.is_empty() {
        parts.push(name);
    }
    js::slice_utf16(&parts.join(". "), 1500)
}

fn hash(text: &str) -> String {
    let sum = digest(
        &SHA256,
        format!("{}:{DIMENSIONS}:{text}", model()).as_bytes(),
    );
    js::base64url_encode(sum.as_ref())
        .chars()
        .take(22)
        .collect()
}

fn valid_numbers(values: &[Value]) -> Option<Vec<f64>> {
    if values.len() != DIMENSIONS {
        return None;
    }
    let numbers: Option<Vec<f64>> = values
        .iter()
        .map(|value| value.as_f64().filter(|n| n.is_finite()))
        .collect();
    numbers.filter(|list| list.iter().any(|n| *n != 0.0))
}

fn normalized(values: &[f64]) -> Vector {
    let hypot = js::hypot(values);
    let length = if hypot == 0.0 || hypot.is_nan() {
        1.0
    } else {
        hypot
    };
    values.iter().map(|value| (value / length) as f32).collect()
}

fn to_base64(vector: &[f32]) -> String {
    let bytes: Vec<u8> = vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    js::base64_encode(&bytes)
}

fn from_base64(text: &str) -> Option<Vector> {
    let bytes = js::base64_decode(text);
    if bytes.len() != DIMENSIONS * 4 {
        return None;
    }
    let vector: Vector = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    (vector.iter().all(|value| value.is_finite()) && vector.iter().any(|value| *value != 0.0))
        .then_some(vector)
}

fn gemini_error(what: impl std::fmt::Display) -> String {
    format!("Gemini: {what}")
}

fn embed_batch(texts: &[String], task_type: &str, key: &str) -> Result<Vec<Vector>, String> {
    let model = model();
    let body = json!({
        "requests": texts.iter().map(|text| json!({
            "model": format!("models/{model}"),
            "content": { "parts": [{ "text": text }] },
            "taskType": task_type,
            "outputDimensionality": DIMENSIONS,
        })).collect::<Vec<_>>()
    })
    .to_string();
    let url = net::transport_url(&format!("{GEMINI_BASE}/{model}:batchEmbedContents"));
    // Redirects are never followed, so the key header cannot travel to another host.
    let agent = net::agent(0);
    let mut attempt = 1;
    loop {
        let outcome = agent
            .post(&url)
            .timeout(net::timeout(TIMEOUT_MS))
            .set("content-type", "application/json")
            .set("x-goog-api-key", key)
            .send_string(&body);
        let response = match outcome {
            Ok(response) => response,
            Err(ureq::Error::Status(status, _)) => {
                if (status == 429 || status >= 500) && attempt < MAX_ATTEMPTS {
                    net::backoff(1000 * 2_u64.pow(attempt));
                    attempt += 1;
                    continue;
                }
                return Err(gemini_error(format_args!("HTTP {status}")));
            }
            Err(ureq::Error::Transport(error)) => {
                if attempt >= MAX_ATTEMPTS {
                    return Err(gemini_error(net::error_code(&error)));
                }
                net::backoff(500 * 2_u64.pow(attempt));
                attempt += 1;
                continue;
            }
        };
        if !(200..300).contains(&response.status()) {
            return Err(gemini_error(format_args!("HTTP {}", response.status())));
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
            .map_err(|_| gemini_error("invalid JSON"))?;
        let parsed: Value =
            serde_json::from_slice(&bytes).map_err(|_| gemini_error("invalid JSON"))?;
        return parse_embeddings(&parsed, texts.len())
            .ok_or_else(|| gemini_error("invalid response"));
    }
}

fn parse_embeddings(parsed: &Value, expected: usize) -> Option<Vec<Vector>> {
    let embeddings = parsed.get("embeddings")?.as_array()?;
    if embeddings.len() != expected {
        return None;
    }
    embeddings
        .iter()
        .map(|entry| {
            Some(normalized(&valid_numbers(
                entry.get("values")?.as_array()?,
            )?))
        })
        .collect()
}

pub fn embed_query(query: &str, key: &str, directory: &Path) -> Result<Vector, String> {
    let digest = crate::cache::hash(json!([model(), DIMENSIONS, query]).to_string().as_bytes());
    let store = crate::cache::Store::open(directory, "queries", 2000);
    if let Some(vector) = store
        .as_ref()
        .and_then(|s| s.get(&digest))
        .and_then(|(v, _)| from_base64(v.as_str()?))
    {
        return Ok(vector);
    }
    let vector = embed_batch(&[query.to_string()], "RETRIEVAL_QUERY", key)?
        .pop()
        .ok_or_else(|| gemini_error("invalid response"))?;
    if let Some(store) = store {
        store.put(&digest, json!(to_base64(&vector)));
    }
    Ok(vector)
}

pub struct VectorStore {
    file: PathBuf,
    vectors: Object,
    dirty: bool,
}

impl VectorStore {
    pub fn new(directory: &Path) -> Self {
        let file = directory.join("embeddings.json");
        let vectors = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        VectorStore {
            file,
            vectors,
            dirty: false,
        }
    }

    fn cached(&self, digest: &str) -> Option<Vector> {
        from_base64(self.vectors.get(digest)?.as_str()?)
    }

    /// Embeds whatever is missing and returns item id -> vector. Finished batches stay in the store even on failure.
    pub fn ensure(
        &mut self,
        items: &[Object],
        key: &str,
    ) -> Result<HashMap<String, Vector>, String> {
        let texts: Vec<String> = items.iter().map(embed_text).collect();
        let hashes: Vec<String> = texts.iter().map(|text| hash(text)).collect();
        let mut seen = std::collections::HashSet::new();
        let mut missing = Vec::new();
        for (digest, text) in hashes.iter().zip(&texts) {
            if self.cached(digest).is_none() && seen.insert(digest.as_str()) {
                missing.push((digest.clone(), text.clone()));
            }
        }
        for batch in missing.chunks(BATCH_SIZE) {
            let batch_texts: Vec<String> = batch.iter().map(|(_, text)| text.clone()).collect();
            let vectors = embed_batch(&batch_texts, "RETRIEVAL_DOCUMENT", key)?;
            for ((digest, _), vector) in batch.iter().zip(&vectors) {
                self.vectors
                    .insert(digest.clone(), Value::String(to_base64(vector)));
            }
            self.dirty = true;
        }
        let mut result = HashMap::new();
        for (item, digest) in items.iter().zip(&hashes) {
            if let (Some(id), Some(vector)) =
                (item.get("id").and_then(Value::as_str), self.cached(digest))
            {
                result.insert(id.to_string(), vector);
            }
        }
        Ok(result)
    }

    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        if crate::cache::write_state(
            &self.file,
            Value::Object(self.vectors.clone()).to_string().as_bytes(),
        )
        .is_ok()
        {
            self.dirty = false;
        }
    }
}

/// Dot product in f64 over f32 components, like the Float32Array loop in Node.
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum()
}
