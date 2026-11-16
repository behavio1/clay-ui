//! The commands: init, search, show, get, sync, sources.

use crate::embeddings::{cosine, embed_query, read_key, VectorStore};
use crate::fsafe::{ensure_real_folder, path_taken, write_no_follow};
use crate::js::{self, coalesce, template, truthy, Object};
use crate::net::{self, map_limit, Fail};
use crate::project::{self, ProjectError};
use crate::search::{default_ranking, expand_query, rank_default, rank_semantic, Limits};
use crate::sources::{preview_status, source_by_id, Source, SOURCES};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::OnceLock;

const STALE_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const EXIT_NO_CODE: i32 = 3;
const SKILL: &str = include_str!("../skills/clay-ui/SKILL.md");

const HELP: &str =
    "Clay UI — find proven UI components, compare variants and download their code as examples

  clayui init [--global] [--target agents|claude|all] [--no-sync]
  clayui search \"<description>\" [--source s] [--limit n] [--details n] [--available-only] [--code] [--json]
  clayui show <id> [--project p] [--code] [--json]
  clayui get <id> [--out folder] [--force] [--json]
  clayui sync [--source s] [--json]
  clayui sources [--json]

Nothing is ever installed into your project: get only downloads example code to a local folder.
Set GEMINI_API_KEY for semantic search. Code from 21st.dev needs API_KEY_21ST.";

enum CliError {
    Usage(String),
    Other(String),
}

type CliResult<T = i32> = Result<T, CliError>;

fn other(message: impl Into<String>) -> CliError {
    CliError::Other(message.into())
}

fn usage(message: impl Into<String>) -> CliError {
    CliError::Usage(message.into())
}

// --- Output: everything printed or cached passes through the secret filter ---

fn secrets() -> &'static [String] {
    static SECRETS: OnceLock<Vec<String>> = OnceLock::new();
    SECRETS.get_or_init(|| {
        let mut list: Vec<String> = ["GEMINI_API_KEY", "API_KEY_21ST", "GITHUB_TOKEN"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .collect();
        list.extend(read_key());
        // Even short credential values must never be persisted.
        list.retain(|secret| !secret.is_empty());
        list.sort();
        list.dedup();
        list
    })
}

pub(crate) fn scrub(text: &str) -> String {
    secrets().iter().fold(text.to_string(), |current, secret| {
        current.replace(secret.as_str(), "[redacted]")
    })
}

pub(crate) fn scrub_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(scrub(&text)),
        Value::Array(values) => Value::Array(values.into_iter().map(scrub_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (scrub(&key), scrub_value(value)))
                .collect(),
        ),
        other => other,
    }
}

fn say(text: &str) {
    let _ = writeln!(std::io::stdout(), "{}", scrub(text));
}

fn warn(text: &str) {
    let _ = writeln!(std::io::stderr(), "{}", scrub(text));
}

#[derive(Clone, Copy)]
enum Log {
    Silent,
    Stdout,
    Stderr,
}

impl Log {
    fn line(self, text: &str) {
        match self {
            Log::Silent => {}
            Log::Stdout => say(text),
            Log::Stderr => warn(text),
        }
    }
}

// --- Arguments: the strict parseArgs behaviour of the Node version ---

#[derive(Default)]
struct Args {
    values: HashMap<String, String>,
    flags: HashSet<String>,
    positionals: Vec<String>,
}

const STRING_OPTIONS: [&str; 5] = ["source", "limit", "details", "project", "out"];
const STRING_OPTIONS_INIT: &str = "target";
const BOOLEAN_OPTIONS: [&str; 6] = [
    "json",
    "code",
    "force",
    "global",
    "no-sync",
    "available-only",
];

fn takes_value(name: &str) -> bool {
    STRING_OPTIONS.contains(&name) || name == STRING_OPTIONS_INIT
}

fn parse_args(arguments: &[String]) -> CliResult<Args> {
    let mut args = Args::default();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        index += 1;
        if argument == "--" {
            args.positionals.extend_from_slice(&arguments[index..]);
            break;
        }
        if argument.len() >= 2 && argument.starts_with('-') && !argument.starts_with("--") {
            let short: String = argument.chars().take(2).collect();
            return Err(usage(format!("Unknown option '{short}'. To specify a positional argument starting with a '-', place it at the end of the command after '--', as in '-- \"{argument}\"")));
        }
        let Some(body) = argument.strip_prefix("--") else {
            args.positionals.push(argument.clone());
            continue;
        };
        let (name, inline) = match body.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (body, None),
        };
        if takes_value(name) {
            let value = match inline {
                Some(value) => value,
                None => {
                    let Some(next) = arguments.get(index) else {
                        return Err(usage(format!("Option '--{name} <value>' argument missing")));
                    };
                    index += 1;
                    if next.len() > 1 && next.starts_with('-') {
                        return Err(usage(format!("Option '--{name}' argument is ambiguous.\nDid you forget to specify the option argument for '--{name}'?\nTo specify an option argument starting with a dash use '--{name}=-XYZ'.")));
                    }
                    next.clone()
                }
            };
            args.values.insert(name.to_string(), value);
        } else if BOOLEAN_OPTIONS.contains(&name) {
            if inline.is_some() {
                return Err(usage(format!(
                    "Option '--{name}' does not take an argument"
                )));
            }
            args.flags.insert(name.to_string());
        } else {
            return Err(usage(format!("Unknown option '--{name}'. To specify a positional argument starting with a '-', place it at the end of the command after '--', as in '-- \"--{name}\"")));
        }
    }
    Ok(args)
}

impl Args {
    fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
    fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
    /// `--source` and friends count only when non-empty, like a truthiness check in JS.
    fn non_empty(&self, name: &str) -> Option<&str> {
        self.value(name).filter(|value| !value.is_empty())
    }
}

fn parse_count(value: Option<&str>, name: &str, fallback: usize, minimum: u64) -> CliResult<usize> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let number = js::number_from_text(value);
    if !js::is_safe_integer(number) || number < minimum as f64 {
        return Err(usage(format!(
            "{name} must be an integer of at least {minimum}"
        )));
    }
    Ok(number as usize)
}

// --- Index ---

fn home_dir() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    if let Some(home) = std::env::var_os(var) {
        return PathBuf::from(home);
    }
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

fn cache_dir() -> PathBuf {
    match std::env::var_os("CLAYUI_CACHE_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => home_dir().join(".cache").join("clayui"),
    }
}

fn index_path() -> PathBuf {
    cache_dir().join("index.json")
}

fn valid_candidate(item: &Value) -> bool {
    item.get("id").is_some_and(Value::is_string)
        && item.get("name").is_some_and(Value::is_string)
        && item.get("source").is_some_and(Value::is_string)
        && item.get("gaps").is_some_and(Value::is_array)
}

/// A damaged source entry is dropped so that healthy sources keep working.
fn read_index() -> Option<Object> {
    let mut index: Object = serde_json::from_slice::<Value>(&std::fs::read(index_path()).ok()?)
        .ok()?
        .as_object()?
        .clone();
    let sources = index.get_mut("sources")?.as_object_mut()?;
    sources.retain(|_, entry| entry.get("items").is_some_and(Value::is_array));
    for entry in sources.values_mut() {
        if let Some(Value::Array(items)) = entry.get_mut("items") {
            items.retain(valid_candidate);
        }
    }
    Some(index)
}

fn empty_index() -> Object {
    let mut index = Object::new();
    index.insert("sources".into(), json!({}));
    index
}

fn index_sources(index: &Object) -> &Object {
    static EMPTY: OnceLock<Object> = OnceLock::new();
    index
        .get("sources")
        .and_then(Value::as_object)
        .unwrap_or_else(|| EMPTY.get_or_init(Object::new))
}

fn write_index(index: &Object) -> CliResult<()> {
    crate::cache::write_state(
        &index_path(),
        scrub_value(Value::Object(index.clone()))
            .to_string()
            .as_bytes(),
    )
    .map_err(other)
}

fn select_sources(id: Option<&str>) -> CliResult<Vec<&'static Source>> {
    match id {
        None => Ok(SOURCES.iter().collect()),
        Some(id) => source_by_id(id)
            .map(|source| vec![source])
            .ok_or_else(|| usage(format!("Unknown source: {id}"))),
    }
}

fn all_items(index: &Object) -> Vec<Object> {
    index_sources(index)
        .values()
        .filter_map(|entry| entry.get("items")?.as_array())
        .flatten()
        .filter_map(|item| item.as_object().cloned())
        .collect()
}

fn sync(sources: &[&'static Source], log: Log, implicit: bool) -> CliResult<Object> {
    let mut index = read_index().unwrap_or_else(empty_index);
    let targets: Vec<&'static Source> = sources
        .iter()
        .copied()
        .filter(|source| !source.live)
        .collect();
    let mut static_sources = HashSet::new();
    if let Some(base) = crate::catalog::configured_base() {
        log.line("static catalog: fetching index");
        match crate::catalog::fetch(&base) {
            Ok(items) => {
                for source in &targets {
                    let items: Vec<Value> = items
                        .iter()
                        .filter(|item| {
                            item.get("source").and_then(Value::as_str) == Some(source.id)
                        })
                        .cloned()
                        .map(Value::Object)
                        .collect();
                    if !items.is_empty() {
                        for item in &items {
                            if let Some(item) = item.as_object() {
                                let _ =
                                    crate::cache::component(
                                        &cache_dir(),
                                        item,
                                        || Ok(item.clone()),
                                    );
                            }
                        }
                        log.line(&format!("{}: {} (static catalog)", source.id, items.len()));
                        index
                            .get_mut("sources")
                            .and_then(Value::as_object_mut)
                            .unwrap()
                            .insert(
                                source.id.into(),
                                json!({"items":items,"syncedAt":js::iso_now(),"error":null}),
                            );
                        static_sources.insert(source.id);
                    }
                }
            }
            Err(error) => log.line(&format!("Warning: {error}; using current sources")),
        }
    }
    let targets: Vec<_> = targets
        .into_iter()
        .filter(|source| !static_sources.contains(source.id))
        .collect();
    for source in &targets {
        if let Some(entries) = index.get_mut("sources").and_then(Value::as_object_mut) {
            entries
                .entry(source.id)
                .or_insert_with(|| json!({ "items": [], "syncedAt": null, "error": null }));
        }
    }
    let loaded = map_limit(&targets, targets.len().max(1), |source| {
        let outcome = catch_unwind(AssertUnwindSafe(|| source.load()))
            .unwrap_or_else(|_| Err(Fail::other("internal error")));
        match &outcome {
            Ok(items) => log.line(&format!("{}: {}", source.id, items.len())),
            Err(error) => log.line(&format!("{}: ERROR {}", source.id, error.message)),
        }
        Ok((source.id, outcome, js::iso_now()))
    })
    .map_err(|error| other(error.message))?;
    if let Some(entries) = index.get_mut("sources").and_then(Value::as_object_mut) {
        for (id, outcome, finished) in loaded {
            let Some(entry) = entries.get_mut(id).and_then(Value::as_object_mut) else {
                continue;
            };
            match outcome {
                Ok(items) => {
                    entry.insert(
                        "items".into(),
                        Value::Array(items.into_iter().map(Value::Object).collect()),
                    );
                    entry.insert("syncedAt".into(), json!(finished));
                    entry.insert("error".into(), Value::Null);
                }
                // A failing source keeps its last good data.
                Err(error) => {
                    entry.insert("error".into(), json!(error.message));
                }
            }
        }
    }
    if let Err(error) = write_index(&index) {
        if !implicit {
            return Err(error);
        }
        warn("Warning: cannot save index; continuing with the in-memory index");
    }
    match read_key() {
        Some(key) => {
            let mut store = VectorStore::new(&cache_dir());
            match store.ensure(&all_items(&index), &key) {
                Ok(_) => log.line("embeddings: ready"),
                Err(error) => {
                    log.line(&format!("embeddings: ERROR {error}"));
                    index.insert("embeddingError".into(), json!(error));
                }
            }
            store.save();
        }
        None => log.line("embeddings: no key, using keyword search"),
    }
    Ok(index)
}

// --- Details ---

fn has_code(detail: &Object) -> bool {
    detail
        .get("files")
        .and_then(Value::as_array)
        .is_some_and(|files| {
            files.iter().any(|file| {
                file.get("content")
                    .and_then(Value::as_str)
                    .is_some_and(|content| !content.is_empty())
            })
        })
}

fn gap_list(detail: &Object) -> Vec<Value> {
    detail
        .get("gaps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn has_gap(detail: &Object, gap: &str) -> bool {
    gap_list(detail)
        .iter()
        .any(|value| value.as_str() == Some(gap))
}

// Only paths that get can actually deliver participate in the module inventory.
pub(crate) fn delivered_files(detail: &Object) -> Vec<(String, String)> {
    detail
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|file| {
            let content = file.get("content")?.as_str().filter(|s| !s.is_empty())?;
            let label = if truthy(file.get("target")) {
                file.get("target")
            } else {
                file.get("path")
            };
            Some((
                project::safe_relative(label?.as_str()?)?,
                content.to_string(),
            ))
        })
        .collect()
}

/// Gaps reflect what the detail read actually found, not what the index assumed.
fn with_observed_gaps(detail: &Object, status: &str) -> Object {
    let mut gaps: Vec<Value> = Vec::new();
    for gap in gap_list(detail) {
        if !gaps.contains(&gap) {
            gaps.push(gap);
        }
    }
    let add = |gaps: &mut Vec<Value>, gap: &str| {
        if !gaps.iter().any(|value| value.as_str() == Some(gap)) {
            gaps.push(json!(gap));
        }
    };
    if !has_code(detail) && !has_gap(detail, "pro-only") && !has_gap(detail, "needs-key") {
        add(&mut gaps, "no-source-listing");
    }
    if status == "broken" {
        add(&mut gaps, "preview-broken");
    }
    let missing = crate::imports::missing(&delivered_files(detail));
    if !missing.is_empty() {
        add(&mut gaps, "local-imports-missing");
    }
    let mut observed = detail.clone();
    observed.insert("missingLocalImports".into(), json!(missing));
    observed.insert("gaps".into(), Value::Array(gaps));
    let status = if status == "unverified" && truthy(detail.get("fromCache")) {
        "unknown"
    } else {
        status
    };
    observed.insert("previewStatus".into(), json!(status));
    observed
}

fn without_code(detail: &Object) -> Object {
    let mut out = detail.clone();
    out.shift_remove("usage");
    if let Some(Value::Array(files)) = out.get_mut("files") {
        for file in files.iter_mut() {
            if let Some(entries) = file.as_object_mut() {
                entries.shift_remove("content");
            }
        }
    }
    out
}

fn join_or(value: Option<&Value>, separator: &str, fallback: &str) -> String {
    let text = value
        .and_then(Value::as_array)
        .map_or(String::new(), |items| js::join(items, separator));
    if text.is_empty() {
        fallback.to_string()
    } else {
        text
    }
}

fn file_label(file: &Value) -> String {
    let target = file.get("target");
    template(if truthy(target) {
        target
    } else {
        file.get("path")
    })
}

fn print_files(detail: &Object, code: bool) {
    say(&format!(
        "deps:      {}",
        join_or(detail.get("dependencies"), ", ", "-")
    ));
    say(&format!(
        "registry:  {}",
        join_or(detail.get("registryDependencies"), ", ", "-")
    ));
    for warning in detail
        .get("sourceWarnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        say(&format!("WARNING:   {}", js::string_of(warning)));
    }
    for file in detail
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        say(&format!("file:      {}", file_label(file)));
        if code && truthy(file.get("content")) {
            say(&format!("\n{}\n", template(file.get("content"))));
        }
    }
    if code && truthy(detail.get("usage")) {
        say(&format!(
            "\n--- usage ---\n{}\n--- end ---\n",
            template(detail.get("usage"))
        ));
    }
}

fn license_line(detail: &Object) -> String {
    format!(
        "{}{}",
        js::string_of(&coalesce(detail.get("license"), json!("unknown"))),
        if truthy(detail.get("licenseAssumed")) {
            " (assumed)"
        } else {
            ""
        }
    )
}

fn json_output(value: &Value) -> CliResult<()> {
    say(&serde_json::to_string_pretty(value).map_err(|error| other(error.to_string()))?);
    Ok(())
}

// --- Commands ---

fn cmd_sync(args: &Args) -> CliResult {
    let selected = select_sources(args.non_empty("source"))?;
    let index = sync(
        &selected,
        if args.flag("json") {
            Log::Stderr
        } else {
            Log::Stdout
        },
        false,
    )?;
    let failed: Vec<Value> = selected
        .iter()
        .filter(|source| !source.live)
        .filter_map(|source| {
            let error = index_sources(&index)
                .get(source.id)?
                .get("error")?
                .as_str()?;
            Some(json!({ "source": source.id, "reason": error }))
        })
        .collect();
    let embedding_error = index.get("embeddingError").cloned().unwrap_or(Value::Null);
    let ok = failed.is_empty() && embedding_error.is_null();
    if args.flag("json") {
        json_output(&json!({ "ok": ok, "failed": failed, "embeddingError": embedding_error }))?;
    }
    Ok(if ok { 0 } else { 1 })
}

fn cmd_sources(args: &Args) -> CliResult {
    let index = read_index().unwrap_or_else(empty_index);
    let entries = index_sources(&index);
    let rows: Vec<Value> = SOURCES
        .iter()
        .map(|source| {
            let entry = entries.get(source.id);
            let items = if source.live { Value::Null } else { json!(entry.and_then(|entry| entry.get("items")?.as_array().map(Vec::len)).unwrap_or(0)) };
            let field = |key: &str| entry.and_then(|entry| entry.get(key)).filter(|value| !value.is_null()).cloned().unwrap_or(Value::Null);
            json!({ "id": source.id, "title": source.title, "access": source.access, "items": items, "syncedAt": field("syncedAt"), "error": field("error") })
        })
        .collect();
    if args.flag("json") {
        json_output(&Value::Array(rows))?;
        return Ok(0);
    }
    for row in &rows {
        let count = row["items"]
            .as_u64()
            .map_or("live".to_string(), |items| format!("{items} items"));
        let synced = row["syncedAt"]
            .as_str()
            .map_or("-".to_string(), |text| js::slice_utf16(text, 10));
        let error = row["error"]
            .as_str()
            .map_or(String::new(), |error| format!("  ERROR: {error}"));
        say(&format!(
            "{:<13} {count:<10} {:<24} {synced}{error}",
            js::string_of(&row["id"]),
            js::string_of(&row["access"])
        ));
    }
    Ok(0)
}

fn cmd_search(args: &Args) -> CliResult {
    let limit = parse_count(args.value("limit"), "--limit", 20, 1)?;
    let detailed = parse_count(args.value("details"), "--details", 0, 0)?;
    let query = js::trim(&args.positionals.join(" ")).to_string();
    if query.is_empty() {
        return Err(usage("Describe the component you need"));
    }
    let chosen = args.non_empty("source");
    let sources = select_sources(chosen)?;
    let json_mode = args.flag("json");
    let index = match read_index() {
        Some(index) => index,
        None => sync(
            &SOURCES.iter().collect::<Vec<_>>(),
            if json_mode { Log::Silent } else { Log::Stderr },
            true,
        )?,
    };
    let entries = index_sources(&index);
    let mut warnings: Vec<String> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    let mut pool: Vec<Object> = Vec::new();
    for source in sources.iter().filter(|source| !source.live) {
        let entry = entries.get(source.id);
        let items = entry
            .and_then(|entry| entry.get("items")?.as_array().cloned())
            .unwrap_or_default();
        let has_items = !items.is_empty();
        pool.extend(
            items
                .into_iter()
                .filter_map(|item| item.as_object().cloned()),
        );
        let error = entry
            .and_then(|entry| entry.get("error"))
            .filter(|error| truthy(Some(error)));
        if let Some(error) = error {
            skipped.push(json!({ "source": source.id, "reason": error }));
        } else if !has_items {
            skipped.push(json!({ "source": source.id, "reason": "not indexed yet, run sync" }));
        }
        let age = entry
            .and_then(|entry| entry.get("syncedAt")?.as_str())
            .and_then(js::parse_iso)
            .map(|synced| js::now_millis() - synced);
        if age.is_some_and(|age| age > STALE_MS) {
            warnings.push(format!(
                "The {} index is over 7 days old, run sync",
                source.id
            ));
        }
    }
    let groups = expand_query(&query);
    if let Some(live) = sources
        .iter()
        .find(|source| source.live)
        .filter(|_| !groups.is_empty())
    {
        match live.search_live(&groups) {
            Ok((items, live_warnings)) => {
                pool.extend(items);
                warnings.extend(live_warnings);
            }
            Err(error) => skipped.push(json!({ "source": live.id, "reason": error.message })),
        }
    }
    let available_only = args.flag("available-only");
    let limits = Limits {
        limit: if available_only { usize::MAX } else { limit },
        per_source: if available_only || chosen.is_some() {
            usize::MAX
        } else {
            5
        },
    };
    let mut results: Option<Vec<Object>> = None;
    let key = read_key();
    match &key {
        Some(key) if !groups.is_empty() => {
            let mut store = VectorStore::new(&cache_dir());
            let (vectors, query_vector) = net::join(
                || store.ensure(&pool, key),
                || embed_query(&query, key, &cache_dir()),
            );
            match vectors.and_then(|vectors| Ok((vectors, query_vector?))) {
                Ok((vectors, query_vector)) => {
                    let similarity: HashMap<String, f64> = pool
                        .iter()
                        .filter_map(|item| {
                            let id = item.get("id")?.as_str()?;
                            Some((id.to_string(), cosine(&query_vector, vectors.get(id)?)))
                        })
                        .collect();
                    results = Some(rank_semantic(
                        &pool,
                        &similarity,
                        limits,
                        default_ranking(pool.len()),
                    ));
                }
                Err(error) => warnings.push(format!(
                    "Semantic search unavailable ({error}), used keyword search"
                )),
            }
            store.save();
        }
        None => warnings.push(
            "No Gemini key, used keyword search. Set GEMINI_API_KEY for much better results"
                .to_string(),
        ),
        _ => {}
    }
    let mut results = results.unwrap_or_else(|| rank_default(&pool, &query, limits));
    if available_only {
        let mut counts = HashMap::new();
        results.retain(|item| {
            if ["needs-key", "pro-only", "no-source-listing"]
                .iter()
                .any(|g| has_gap(item, g))
            {
                return false;
            }
            let count = counts.entry(template(item.get("source"))).or_insert(0);
            if chosen.is_none() && *count >= 5 {
                return false;
            }
            *count += 1;
            true
        });
        results.truncate(limit);
    }
    // Metadata only: enrich after ranking so default membership and scores stay frozen.
    results = map_limit(&results, 8, |item| Ok(crate::sources::search_license(item)))
        .map_err(|error| other(error.message))?;
    if detailed > 0 && !results.is_empty() {
        let indices: Vec<usize> = (0..results.len()).collect();
        let snapshot = results.clone();
        let code = args.flag("code");
        results = map_limit(&indices, indices.len(), |index| {
            let item = &snapshot[*index];
            if *index >= detailed {
                return Ok(item.clone());
            }
            Ok(detail_for_search(item, code))
        })
        .map_err(|error| other(error.message))?;
    }
    if available_only {
        results.retain(|item| {
            !truthy(item.get("detailError"))
                && !["needs-key", "pro-only", "no-source-listing"]
                    .iter()
                    .any(|g| has_gap(item, g))
        });
    }
    if !args.flag("code") {
        results = results.iter().map(without_code).collect();
    }
    if detailed == 0 {
        for item in &mut results {
            item.shift_remove("card");
            item.shift_remove("evidence");
            item.shift_remove("files");
            item.shift_remove("usage");
        }
    }
    let result_values: Vec<Value> = results.iter().cloned().map(Value::Object).collect();
    if json_mode {
        json_output(
            &json!({ "query": query, "results": result_values, "skipped": skipped, "warnings": warnings }),
        )?;
        return Ok(0);
    }
    if results.is_empty() {
        say("No matches.");
    }
    let code = args.flag("code");
    for item in &results {
        print_cache_warning(item);
        let gaps = gap_list(item);
        let flag = if gaps.is_empty() {
            "complete".to_string()
        } else {
            format!("gaps: {}", js::join(&gaps, ", "))
        };
        let description = if truthy(item.get("description")) {
            format!(
                " — {}",
                js::slice_utf16(&template(item.get("description")), 100)
            )
        } else {
            String::new()
        };
        let preview = js::string_of(&coalesce(item.get("previewUrl"), json!("none")));
        let status = if truthy(item.get("previewStatus")) {
            format!(" ({})", template(item.get("previewStatus")))
        } else {
            String::new()
        };
        say(&format!(
            "{}  [{flag}]\n    {}{description}\n    preview: {preview}{status}",
            template(item.get("id")),
            template(item.get("title"))
        ));
        if truthy(item.get("detailError")) {
            say(&format!(
                "    details error: {}",
                template(item.get("detailError"))
            ));
        }
        if truthy(item.get("files")) {
            print_files(item, code);
        }
    }
    for warning in &warnings {
        warn(&format!("Warning: {warning}"));
    }
    for entry in &skipped {
        warn(&format!(
            "Skipped {}: {}",
            template(entry.get("source")),
            template(entry.get("reason"))
        ));
    }
    Ok(0)
}

fn detail_for_search(item: &Object, code: bool) -> Object {
    let outcome = (|| -> Result<Object, String> {
        let source_id = item.get("source").and_then(Value::as_str).unwrap_or("");
        let source = source_by_id(source_id)
            .ok_or("Cannot read properties of undefined (reading 'detail')")?;
        let detail = cached_detail(source, item).map_err(|error| error.message)?;
        let observed = with_observed_gaps(&detail, preview_status(detail.get("previewUrl")));
        let mut out = if code {
            observed
        } else {
            without_code(&observed)
        };
        out.insert(
            "score".into(),
            item.get("score").cloned().unwrap_or(Value::Null),
        );
        Ok(out)
    })();
    outcome.unwrap_or_else(|message| {
        let mut out = item.clone();
        out.insert("detailError".into(), json!(message));
        out
    })
}

fn parse_id(id: Option<&str>) -> CliResult<(&'static Source, String)> {
    let shown = id.unwrap_or("(none)");
    let invalid = || usage(format!("Invalid id: {shown}"));
    let (source_id, name) = id.and_then(|id| id.split_once('/')).ok_or_else(invalid)?;
    let source_ok = !source_id.is_empty()
        && source_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let name_ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '/' | '-'));
    if !source_ok || !name_ok {
        return Err(invalid());
    }
    let source =
        source_by_id(source_id).ok_or_else(|| usage(format!("Unknown source: {source_id}")))?;
    if name
        .split('/')
        .any(|part| part == ".." || part == "." || part.starts_with('-'))
    {
        return Err(invalid());
    }
    Ok((source, name.to_string()))
}

fn load_detail(id: Option<&str>) -> CliResult<Object> {
    let (source, name) = parse_id(id)?;
    let item = if source.live {
        source.from_id(&name)
    } else {
        let index = match read_index() {
            Some(index) => index,
            None => sync(&SOURCES.iter().collect::<Vec<_>>(), Log::Stderr, true)?,
        };
        index_sources(&index)
            .get(source.id)
            .and_then(|entry| entry.get("items")?.as_array())
            .and_then(|items| {
                items
                    .iter()
                    .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name.as_str()))
            })
            .and_then(|item| item.as_object().cloned())
    };
    let item = item.ok_or_else(|| {
        usage(format!(
            "Not found: {}. Run clayui sync or search",
            id.unwrap_or("")
        ))
    })?;
    cached_detail(source, &item).map_err(|error| other(error.message))
}

fn positional(args: &Args) -> Option<&str> {
    args.positionals.first().map(String::as_str)
}

fn clean_description(mut detail: Object) -> Object {
    if let Some(text) = detail.get("description").and_then(Value::as_str) {
        let mut end = text.len();
        for marker in [
            "npx ",
            "pnpm dlx ",
            "pnpm add ",
            "npm install ",
            "yarn add ",
            "bunx ",
            "bun add ",
        ] {
            if let Some(at) = text.find(marker) {
                end = end.min(at);
            }
        }
        if end == text.len() {
            return detail;
        }
        {
            let lower = text[..end].to_ascii_lowercase();
            for label in [
                "install with",
                "install via",
                "install:",
                "installation:",
                "```",
            ] {
                if let Some(at) = lower.rfind(label) {
                    end = end.min(at);
                }
            }
        }
        let cleaned = text[..end]
            .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '`' | ':' | '-'))
            .trim();
        detail.insert("description".into(), json!(cleaned));
    }
    detail
}

fn cmd_show(args: &Args) -> CliResult {
    let project = match args.non_empty("project") {
        Some(directory) => Some(project::inspect_project(directory).map_err(
            |error| match error {
                ProjectError::Usage(message) => CliError::Usage(message),
                ProjectError::Other(message) => CliError::Other(message),
            },
        )?),
        None => None,
    };
    let detail = load_detail(positional(args))?;
    print_cache_warning(&detail);
    let observed = clean_description(with_observed_gaps(
        &detail,
        preview_status(detail.get("previewUrl")),
    ));
    let code = args.flag("code");
    let mut output = if code {
        observed
    } else {
        without_code(&observed)
    };
    output.insert(
        "project".into(),
        project.as_ref().map_or(Value::Null, |project| {
            json!({ "react": project.react, "tailwind": project.tailwind, "missingDependencies": project::missing_dependencies(&detail, project) })
        }),
    );
    if args.flag("json") {
        json_output(&Value::Object(output))?;
        return Ok(0);
    }
    let description = if truthy(output.get("description")) {
        format!(" — {}", template(output.get("description")))
    } else {
        String::new()
    };
    say(&format!(
        "{}\n{}{description}",
        template(output.get("id")),
        template(output.get("title"))
    ));
    say(&format!(
        "preview: {} ({})",
        js::string_of(&coalesce(output.get("previewUrl"), json!("none"))),
        template(output.get("previewStatus"))
    ));
    say(&format!("license:   {}", license_line(&output)));
    say(&format!(
        "gaps:      {}",
        join_or(output.get("gaps"), ", ", "none")
    ));
    print_files(&output, code);
    if truthy(output.get("project")) {
        say(&format!("project:   {}", output["project"]));
    }
    Ok(0)
}

fn name_parts(name: &str) -> Vec<&str> {
    name.split('/').collect()
}

// Downloads example source only. Content is untrusted: it is written as data and never executed.
fn cmd_get(args: &Args) -> CliResult {
    let loaded = load_detail(positional(args))?;
    let observed = clean_description(with_observed_gaps(
        &loaded,
        preview_status(loaded.get("previewUrl")),
    ));
    let detail = scrub_value(Value::Object(observed))
        .as_object()
        .cloned()
        .ok_or_else(|| other("Invalid detail"))?;
    let base = project::resolve(args.value("out").unwrap_or("clayui-examples"));
    let source = detail.get("source").and_then(Value::as_str).unwrap_or("");
    let name = detail.get("name").and_then(Value::as_str).unwrap_or("");
    let mut parts = vec![source];
    parts.extend(name_parts(name).into_iter().filter(|part| !part.is_empty()));
    let destination = parts
        .iter()
        .fold(base.clone(), |path, part| path.join(part));
    if path_taken(&destination) && !args.flag("force") {
        return Err(usage(format!(
            "Folder already exists: {}. Use --force",
            destination.display()
        )));
    }
    let mut skipped: Vec<Value> = detail
        .get("skipped")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut unsafe_paths = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();
    for file in detail
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let target = file.get("target");
        let label = if truthy(target) {
            target
        } else {
            file.get("path")
        };
        let Some(content) = file
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            skipped.push(label.cloned().unwrap_or(Value::Null));
            continue;
        };
        match label
            .and_then(Value::as_str)
            .and_then(project::safe_relative)
        {
            Some(relative) => files.push((relative, content.to_string())),
            None => {
                let path = label.cloned().unwrap_or(Value::Null);
                unsafe_paths.push(path.clone());
                skipped.push(path);
            }
        }
    }
    print_cache_warning(&detail);
    for warning in detail
        .get("sourceWarnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        warn(&format!("Warning: {}", js::string_of(warning)));
    }
    for file in &unsafe_paths {
        warn(&format!(
            "Skipped unsafe path: {}",
            if file.is_null() {
                "undefined".to_string()
            } else {
                js::string_of(file)
            }
        ));
    }
    let usage_text = detail
        .get("usage")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty());
    if files.is_empty() {
        let reason = if has_gap(&detail, "needs-key") {
            ": requires API_KEY_21ST"
        } else if has_gap(&detail, "pro-only") {
            ": paid item, requires a license from its author"
        } else {
            ""
        };
        warn(&format!("Code for {} is unavailable{reason}; no source files were downloaded. Usage documentation alone is not source code. Preview: {}", template(detail.get("id")), js::string_of(&coalesce(detail.get("previewUrl"), json!("none")))));
        return Ok(EXIT_NO_CODE);
    }
    ensure_real_folder(&base, &parts).map_err(other)?;
    let mut written: Vec<String> = Vec::new();
    let mut hashes = Object::new();
    for (relative, content) in &files {
        let segments: Vec<&str> = relative.split('/').collect();
        let (file_name, folders) = segments.split_last().ok_or_else(|| other("Invalid path"))?;
        let folder = ensure_real_folder(&destination, folders).map_err(other)?;
        write_no_follow(&folder.join(file_name), scrub(content).as_bytes()).map_err(other)?;
        hashes.insert(
            relative.clone(),
            json!(crate::cache::hash(scrub(content).as_bytes())),
        );
        written.push(relative.clone());
    }
    if let Some(text) = usage_text {
        write_no_follow(&destination.join("USAGE.md"), scrub(text).as_bytes()).map_err(other)?;
        hashes.insert(
            "USAGE.md".into(),
            json!(crate::cache::hash(scrub(text).as_bytes())),
        );
        written.push("USAGE.md".to_string());
    }
    let mut meta = Object::new();
    meta.insert(
        "id".into(),
        detail.get("id").cloned().unwrap_or(Value::Null),
    );
    for key in [
        "title",
        "description",
        "previewUrl",
        "previewStatus",
        "sourceUrl",
        "sourceRef",
        "version",
        "missingLocalImports",
        "license",
        "licenseUrl",
        "registryType",
        "registryItems",
        "example",
        "css",
        "cssVars",
        "tailwind",
        "cacheWarning",
        "cacheAgeDays",
    ] {
        if let Some(value) = detail.get(key) {
            meta.insert(key.into(), value.clone());
        }
    }
    meta.insert(
        "licenseAssumed".into(),
        json!(truthy(detail.get("licenseAssumed"))),
    );
    meta.insert("gaps".into(), Value::Array(gap_list(&detail)));
    meta.insert(
        "sourceWarnings".into(),
        coalesce(detail.get("sourceWarnings"), json!([])),
    );
    if let Some(dependencies) = detail.get("dependencies") {
        meta.insert("dependencies".into(), dependencies.clone());
    }
    meta.insert(
        "registryDependencies".into(),
        coalesce(detail.get("registryDependencies"), json!([])),
    );
    meta.insert("envVars".into(), coalesce(detail.get("envVars"), json!([])));
    meta.insert("skipped".into(), Value::Array(skipped));
    meta.insert("files".into(), json!(written));
    meta.insert("fileSha256".into(), Value::Object(hashes));
    for key in ["fetchedAt", "fromCache"] {
        if let Some(value) = detail.get(key) {
            meta.insert(key.into(), value.clone());
        }
    }
    let meta_text = serde_json::to_string_pretty(&Value::Object(meta.clone()))
        .map_err(|error| other(error.to_string()))?;
    write_no_follow(
        &destination.join("clayui.json"),
        scrub(&meta_text).as_bytes(),
    )
    .map_err(other)?;
    if args.flag("json") {
        let mut output = Object::new();
        output.insert(
            "destination".into(),
            json!(destination.display().to_string()),
        );
        output.extend(meta);
        json_output(&Value::Object(output))?;
        return Ok(0);
    }
    say(&format!(
        "{} → {}\nlicense: {}\nfiles:    {}",
        template(detail.get("id")),
        destination.display(),
        license_line(&detail),
        written.join(", ")
    ));
    Ok(0)
}

const SKILL_FOLDERS: [(&str, [&str; 3]); 2] = [
    ("agents", [".agents", "skills", "clay-ui"]),
    ("claude", [".claude", "skills", "clay-ui"]),
];

// Writes the embedded agent skill so a coding agent can use Clay UI straight away.
fn cmd_init(args: &Args) -> CliResult {
    let target = args.value("target").unwrap_or("all");
    let chosen: Vec<_> = SKILL_FOLDERS
        .iter()
        .filter(|(name, _)| target == "all" || target == *name)
        .collect();
    if chosen.is_empty() {
        return Err(usage("--target must be agents, claude or all"));
    }
    let base = if args.flag("global") {
        home_dir()
    } else {
        std::env::current_dir().map_err(|error| other(error.to_string()))?
    };
    // Preflight every destination before installing any skill.
    for (_, parts) in &chosen {
        crate::fsafe::preflight(&base, &parts[..], "SKILL.md").map_err(other)?;
    }
    for (_, parts) in chosen {
        let folder = ensure_real_folder(&base, parts).map_err(other)?;
        write_no_follow(&folder.join("SKILL.md"), SKILL.as_bytes()).map_err(other)?;
        say(&format!("Skill installed: {}", folder.display()));
    }
    if !args.flag("no-sync") {
        let code = cmd_sync(&Args::default())?;
        if code != 0 {
            return Ok(code);
        }
    }
    let tip = if read_key().is_some() {
        ""
    } else {
        "\nTip: set GEMINI_API_KEY or CLAYUI_GEMINI_ENV_FILE for semantic search. Without it Clay UI falls back to keyword search."
    };
    say(&format!("\nDone. Ask your coding agent for a component, for example: \"find me a nice data table with Clay UI\".{tip}"));
    Ok(0)
}

fn dispatch(arguments: Vec<String>) -> CliResult {
    let Some(name) = arguments.first().map(String::as_str) else {
        say(HELP);
        return Ok(0);
    };
    if name == "--version" {
        say(env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if home_dir().as_os_str().is_empty() {
        return Err(other(
            "Home directory is empty; set HOME (USERPROFILE on Windows)",
        ));
    }
    crate::cache::prune_all(&cache_dir());
    if name == "--help" || name == "-h" {
        say(HELP);
        return Ok(0);
    }
    let command: fn(&Args) -> CliResult = match name {
        "init" => cmd_init,
        "search" => cmd_search,
        "show" => cmd_show,
        "get" => cmd_get,
        "sync" => cmd_sync,
        "sources" => cmd_sources,
        _ => return Err(usage(format!("Unknown command: {name}"))),
    };
    command(&parse_args(&arguments[1..])?)
}

/// Runs the CLI and returns the process exit code.
pub fn run(arguments: Vec<String>) -> i32 {
    // A panic message could quote data, so it is never printed.
    std::panic::set_hook(Box::new(|_| {}));
    match catch_unwind(AssertUnwindSafe(|| dispatch(arguments))) {
        Ok(Ok(code)) => code,
        Ok(Err(CliError::Usage(message))) => {
            warn(message.lines().next().unwrap_or(&message));
            2
        }
        Ok(Err(CliError::Other(message))) => {
            warn(&format!("Error: {message}"));
            1
        }
        Err(_) => {
            warn("Error: internal error");
            1
        }
    }
}

fn print_cache_warning(detail: &Object) {
    if let Some(warning) = detail.get("cacheWarning").and_then(Value::as_str) {
        warn(warning);
    }
}

fn cached_detail(source: &Source, item: &Object) -> net::Res<Object> {
    crate::cache::component(&cache_dir(), item, || {
        if item.get("staticSnapshot").and_then(Value::as_bool) == Some(true) {
            let base = crate::catalog::configured_base()
                .ok_or_else(|| Fail::other("Static catalog is not configured; run sync"))?;
            crate::catalog::fetch(&base)
                .map_err(Fail::other)?
                .into_iter()
                .find(|candidate| candidate.get("id") == item.get("id"))
                .ok_or_else(|| Fail::other("Component is no longer in the static catalog"))
        } else {
            source.detail(item)
        }
    })
    .map(crate::sources::normalize_license)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_cleanup_keeps_non_install_text_exactly() {
        for text in ["Button:", "A menu -", "`Label`", "Żółć: "] {
            let detail = json!({"description": text}).as_object().unwrap().clone();
            assert_eq!(clean_description(detail.clone()), detail);
        }
        for command in [
            "npx x",
            "pnpm dlx x",
            "pnpm add x",
            "npm install x",
            "yarn add x",
            "bunx x",
            "bun add x",
        ] {
            let detail = json!({"description": format!("Nice button. Install with `{command}`")})
                .as_object()
                .unwrap()
                .clone();
            assert_eq!(clean_description(detail)["description"], "Nice button.");
        }
    }
}
