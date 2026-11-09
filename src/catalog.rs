//! Optional static catalog. All downloaded content remains untrusted data.
use crate::js::Object;
use crate::{net, project, sources};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Read;

/// Release owner sets this once the audited artifacts have been uploaded.
pub const DEFAULT_CDN_BASE_URL: &str = "";
const MAX_BYTES: u64 = 16 * 1024 * 1024;

pub fn configured_base() -> Option<String> {
    let value =
        std::env::var("CLAYUI_CDN_BASE_URL").unwrap_or_else(|_| DEFAULT_CDN_BASE_URL.into());
    (!value.is_empty()).then_some(value)
}

fn index_url(base: &str) -> Result<String, String> {
    let authority = base
        .strip_prefix("https://")
        .ok_or("Static catalog requires HTTPS")?
        .split('/')
        .next()
        .unwrap_or("");
    if authority.is_empty()
        || base.contains(['@', '?', '#', '\\'])
        || base.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(
            "Invalid static catalog URL; credentials, queries and fragments are forbidden".into(),
        );
    }
    Ok(format!("{}/index.json", base.trim_end_matches('/')))
}

pub fn validate(value: Value) -> Result<Vec<Object>, String> {
    let invalid = || "Invalid static catalog schema or source evidence".to_string();
    if value.get("schema").and_then(Value::as_u64) != Some(1) {
        return Err(invalid());
    }
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if items.is_empty() || items.len() > 5000 {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    let mut result = Vec::new();
    for value in items {
        let item = value.as_object().ok_or_else(invalid)?;
        let text = |key: &str| item.get(key).and_then(Value::as_str).ok_or_else(invalid);
        let (id, source, name) = (text("id")?, text("source")?, text("name")?);
        if sources::source_by_id(source).is_none()
            || id != format!("{source}/{name}")
            || !ids.insert(id)
            || project::safe_relative(name).as_deref() != Some(name)
            || name.split('/').any(|s| s.starts_with('-'))
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        {
            return Err(invalid());
        }
        for key in ["title", "description"] {
            if text(key)?.len() > 16000 {
                return Err(invalid());
            }
        }
        for key in ["gaps", "keywords", "dependencies", "registryDependencies"] {
            let entries = item
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(invalid)?;
            if entries.len() > 200
                || entries
                    .iter()
                    .any(|v| v.as_str().map_or(true, |s| s.len() > 2000))
            {
                return Err(invalid());
            }
        }
        let files = item
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        if files.is_empty() || files.len() > 120 {
            return Err(invalid());
        }
        for file in files {
            let path = file
                .get("target")
                .or_else(|| file.get("path"))
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            if project::safe_relative(path).is_none()
                || file
                    .get("content")
                    .and_then(Value::as_str)
                    .map_or(true, |s| s.is_empty() || s.len() > 1024 * 1024)
            {
                return Err(invalid());
            }
        }
        // Keep data fields only; executable instructions or arbitrary index keys are discarded.
        let mut clean = Object::new();
        for key in [
            "id",
            "source",
            "name",
            "title",
            "description",
            "keywords",
            "gaps",
            "dependencies",
            "registryDependencies",
            "files",
            "usage",
            "previewUrl",
            "license",
            "licenseAssumed",
            "sourceWarnings",
            "card",
            "evidence",
        ] {
            if let Some(value) = item.get(key) {
                clean.insert(key.into(), value.clone());
            }
        }
        clean.insert("staticSnapshot".into(), json!(true));
        result.push(clean);
    }
    Ok(result)
}

pub fn fetch(base: &str) -> Result<Vec<Object>, String> {
    let url = index_url(base)?;
    // A dedicated no-redirect client: no inherited auth, cookies, GitHub token or Gemini key.
    let response = net::agent(0)
        .get(&net::transport_url(&url))
        .timeout(net::timeout(15000))
        .call()
        .map_err(|_| {
            "Static catalog request failed (HTTPS only; redirects are refused)".to_string()
        })?;
    if response.status() != 200 {
        return Err(format!("Static catalog HTTP {}", response.status()));
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Static catalog read failed".to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Static catalog exceeds size limit".into());
    }
    validate(serde_json::from_slice(&bytes).map_err(|_| "Static catalog is not JSON".to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_https_without_credentials_or_queries() {
        for base in [
            "http://unit.invalid",
            "https://a:b@unit.invalid",
            "https://unit.invalid?q=x",
            "https://unit.invalid#x",
            "https://unit.invalid\\other",
            "https:///missing",
        ] {
            assert!(index_url(base).is_err(), "{base}");
        }
        assert_eq!(
            index_url("https://unit.invalid/v1/").unwrap(),
            "https://unit.invalid/v1/index.json"
        );
    }
    #[test]
    fn rejects_invalid_shape_duplicates_and_unsafe_files() {
        let item = json!({"id":"shadcn/table", "source":"shadcn", "name":"table", "title":"Table", "description":"", "gaps":[], "keywords":[], "dependencies":[], "registryDependencies":[], "files":[{"path":"table.tsx","content":"source"}]});
        assert!(validate(json!({"schema":1,"items":[item.clone()]})).is_ok());
        assert!(validate(json!({"schema":2,"items":[item.clone()]})).is_err());
        assert!(validate(json!({"schema":1,"items":[item.clone(),item.clone()]})).is_err());
        let mut bad = item.clone();
        bad["files"][0]["path"] = json!("../escape");
        assert!(validate(json!({"schema":1,"items":[bad]})).is_err());
        let mut bad = item.clone();
        bad["source"] = json!("unknown");
        assert!(validate(json!({"schema":1,"items":[bad]})).is_err());
    }
}
