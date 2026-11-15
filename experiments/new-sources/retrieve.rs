//! Dev-only fixed-catalog measurement using the production Rust ranker and key reader.
use clayui_lib::{
    embeddings::{self, VectorStore},
    search::{self, Limits, RANKING},
};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::Path};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(&args[1]);
    let queries: Value = serde_json::from_slice(&fs::read(&args[2]).unwrap()).unwrap();
    let cache = Path::new(&args[3]);
    let key = embeddings::read_key();
    eprintln!(
        "configuration: production ranker; concise EN dev; Gemini key present={}",
        key.is_some()
    );
    let mut store = VectorStore::new(cache);
    for arm in ["baseline", "expanded"] {
        let value: Value =
            serde_json::from_slice(&fs::read(root.join(format!("{arm}-index.json"))).unwrap())
                .unwrap();
        let raw_items: Vec<_> = value["sources"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|v| {
                v["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|v| v.as_object().cloned())
            })
            .collect();
        // The scorer requires unique full IDs. Match the previous experiment's Map
        // normalization on both arms; the live Aceternity index currently repeats IDs.
        let mut items: Vec<clayui_lib::js::Object> = Vec::new();
        let mut positions: HashMap<String, usize> = HashMap::new();
        for item in raw_items {
            let id = item["id"].as_str().unwrap().to_string();
            if let Some(position) = positions.get(&id) {
                items[*position] = item;
            } else {
                positions.insert(id, items.len());
                items.push(item);
            }
        }
        let vectors = if let Some(key) = &key {
            eprintln!(
                "{arm}: embedding {} documents with checkpoint after each batch",
                items.len()
            );
            // Existing VectorStore keeps finished batches on failure. Save after every <=100 items.
            let mut all = HashMap::new();
            for (n, batch) in items.chunks(100).enumerate() {
                let outcome = store.ensure(batch, key);
                store.save();
                match outcome {
                    Ok(v) => {
                        all.extend(v);
                        eprintln!("{arm}: document batch {} accept", n + 1);
                    }
                    Err(e) => {
                        eprintln!("{arm}: embedding error: {e}");
                        std::process::exit(1)
                    }
                }
            }
            Some(all)
        } else {
            None
        };
        let mut bm25 = Vec::new();
        let mut gemini = Vec::new();
        for q in queries["queries"].as_array().unwrap() {
            let id = q["id"].as_str().unwrap();
            let text = q["concise"].as_str().unwrap();
            let candidates: Vec<_> = items
                .iter()
                .filter(|i| q["source"].as_str().is_none_or(|s| i["source"] == s))
                .cloned()
                .collect();
            let ranked = search::rank(
                &candidates,
                text,
                Limits {
                    limit: 5,
                    per_source: 5,
                },
            );
            bm25.push(json!({"id":id,"status":"ok","components":ranked.iter().map(|i|i["id"].clone()).collect::<Vec<_>>()}));
            fs::write(
                root.join(format!("{arm}-bm25.results.json")),
                json!({"schema":1,"results":bm25}).to_string(),
            )
            .unwrap();
            if let (Some(key), Some(vectors)) = (&key, &vectors) {
                eprintln!("{arm}/{id}: embed query start");
                let vector = embeddings::embed_query(text, key, cache).unwrap();
                let sims = vectors
                    .iter()
                    .map(|(id, v)| (id.clone(), embeddings::cosine(v, &vector)))
                    .collect();
                let ranked = search::rank_semantic(
                    &candidates,
                    &sims,
                    Limits {
                        limit: 5,
                        per_source: 5,
                    },
                    RANKING,
                );
                gemini.push(json!({"id":id,"status":"ok","components":ranked.iter().map(|i|i["id"].clone()).collect::<Vec<_>>()}));
                fs::write(
                    root.join(format!("{arm}-gemini.results.json")),
                    json!({"schema":1,"results":gemini}).to_string(),
                )
                .unwrap();
                eprintln!("{arm}/{id}: accept");
            }
        }
    }
}
