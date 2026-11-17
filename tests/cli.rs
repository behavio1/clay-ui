//! Runs the real binary against a local mock HTTP server. Ports test/cli.test.mjs and adds `init`.
//! Every process gets a temporary HOME, cache and working directory.

use clayui_lib::embeddings::read_key_from;
use clayui_lib::js::Object;
use clayui_lib::project::safe_relative;
use clayui_lib::search::{expand_query, rank, rank_semantic, Limits, Ranking, RANKING};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "clayui-rs-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&path).unwrap();
    let path = path.canonicalize().unwrap();
    TEMP_PATHS.with(|paths| paths.0.borrow_mut().push(path.clone()));
    path
}

struct TempPaths(std::cell::RefCell<Vec<PathBuf>>);
impl Drop for TempPaths {
    fn drop(&mut self) {
        for path in self.0.get_mut().drain(..) {
            let _ = fs::remove_dir_all(path);
        }
    }
}
thread_local! { static TEMP_PATHS: TempPaths = const { TempPaths(std::cell::RefCell::new(Vec::new())) }; }

// --- Mock server ------------------------------------------------------------

#[derive(Clone, Debug)]
struct Call {
    path: String,
    headers: HashMap<String, String>,
}

enum Reply {
    Respond(u16, String),
    Hang,
}

fn ok(body: impl Into<String>) -> Reply {
    Reply::Respond(200, body.into())
}

type Handler = dyn Fn(&Call) -> Reply + Send + Sync;

struct Mock {
    base: String,
    calls: Arc<Mutex<Vec<Call>>>,
}

fn serve(mut stream: TcpStream, handler: &Handler, calls: &Mutex<Vec<Call>>) -> Option<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let path = lines.next()?.split(' ').nth(1)?.to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body_read = buffer.len() - header_end - 4;
    while body_read < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        body_read += read;
    }
    let call = Call { path, headers };
    calls.lock().unwrap().push(call.clone());
    match handler(&call) {
        Reply::Hang => std::thread::sleep(Duration::from_secs(5)),
        Reply::Respond(status, body) => {
            let response = format!(
                "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    }
    Some(())
}

fn mock(handler: impl Fn(&Call) -> Reply + Send + Sync + 'static) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let handler: Arc<Handler> = Arc::new(handler);
    let log = calls.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (handler, log) = (handler.clone(), log.clone());
            std::thread::spawn(move || serve(stream, handler.as_ref(), &log));
        }
    });
    Mock { base, calls }
}

fn hanging() -> Mock {
    mock(|_| Reply::Hang)
}

// --- Running the CLI --------------------------------------------------------

struct Run {
    code: i32,
    out: String,
    err: String,
}

impl Run {
    fn json(&self) -> Value {
        serde_json::from_str(&self.out).unwrap_or_else(|_| panic!("not JSON: {}", self.out))
    }
}

struct Options<'a> {
    cache: &'a Path,
    env: Vec<(&'a str, &'a str)>,
    cwd: Option<&'a Path>,
    home: Option<&'a Path>,
}

fn cli_with(args: &[&str], mock: &Mock, options: Options) -> Run {
    let home = temp();
    let working = temp();
    let output = Command::new(env!("CARGO_BIN_EXE_clayui"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap())
        // Windows sockets fail to start without SYSTEMROOT.
        .envs(std::env::var_os("SYSTEMROOT").map(|root| ("SYSTEMROOT", root)))
        .env("HOME", options.home.unwrap_or(&home))
        .env("USERPROFILE", options.home.unwrap_or(&home))
        .env("CLAYUI_CACHE_DIR", options.cache)
        .env("CLAYUI_TEST_BASE_URL", &mock.base)
        .env("CLAYUI_TEST_TIMEOUT_MS", "400")
        .envs(options.env.iter().copied())
        .current_dir(options.cwd.unwrap_or(&working))
        .output()
        .unwrap();
    Run {
        code: output.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn cli(args: &[&str], mock: &Mock, cache: &Path) -> Run {
    cli_with(
        args,
        mock,
        Options {
            cache,
            env: vec![],
            cwd: None,
            home: None,
        },
    )
}

fn cli_env(args: &[&str], mock: &Mock, cache: &Path, env: Vec<(&str, &str)>) -> Run {
    cli_with(
        args,
        mock,
        Options {
            cache,
            env,
            cwd: None,
            home: None,
        },
    )
}

fn item_with(source: &str, name: &str, extra: Value) -> Object {
    let mut item = json!({ "id": format!("{source}/{name}"), "source": source, "name": name, "title": name, "description": "", "gaps": [] }).as_object().unwrap().clone();
    item.extend(extra.as_object().unwrap().clone());
    item
}

fn item(source: &str, name: &str) -> Object {
    item_with(source, name, json!({}))
}

fn iso_now() -> String {
    clayui_lib::js::iso_now()
}

fn seed_index(cache: &Path, source: &str, items: Vec<Object>) {
    let body =
        json!({ "sources": { source: { "items": items, "syncedAt": iso_now(), "error": null } } });
    fs::write(cache.join("index.json"), body.to_string()).unwrap();
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn entries(path: &Path) -> Vec<String> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

fn no_gaps(limit: usize) -> Limits {
    Limits {
        limit,
        per_source: 5,
    }
}

fn magic_item() -> Object {
    item_with(
        "magicui",
        "button",
        json!({ "previewUrl": "https://x.dev/a" }),
    )
}

fn registry_mock(body: Value) -> Mock {
    mock(move |_| ok(body.to_string()))
}

// --- Ported from test/cli.test.mjs ------------------------------------------

#[test]
fn s1_s2_english_words_expand_to_component_synonyms() {
    assert_eq!(expand_query("carousel")[0].synonyms, vec!["slider"]);
    let candidates = [item("a", "carousel"), item("a", "tabs")];
    assert_eq!(
        rank(&candidates, "carousel", no_gaps(20))[0]["name"],
        "carousel"
    );
}

#[test]
fn s6_no_hits_gives_an_empty_result_and_exit_0() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "dialog")]);
    let result = cli(
        &["search", "qqqzzz", "--source", "shadcn", "--json"],
        &hanging(),
        &cache,
    );
    assert_eq!(result.code, 0);
    assert_eq!(result.json()["results"], json!([]));
}

#[test]
fn s4_a_timing_out_source_keeps_its_last_good_data_and_reports_the_error() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "dialog")]);
    let result = cli(&["sync", "--source", "shadcn"], &hanging(), &cache);
    assert_eq!(result.code, 1);
    let entry = read_json(cache.join("index.json"))["sources"]["shadcn"].clone();
    assert_eq!(entry["items"].as_array().unwrap().len(), 1);
    assert!(
        entry["error"].as_str().unwrap().contains("Timeout"),
        "{}",
        entry["error"]
    );
}

#[test]
fn s5_invalid_json_does_not_overwrite_the_index() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![item("magicui", "marquee")]);
    cli(
        &["sync", "--source", "magicui"],
        &mock(|_| ok("<html>nope")),
        &cache,
    );
    let entry = read_json(cache.join("index.json"))["sources"]["magicui"].clone();
    assert_eq!(entry["items"][0]["name"], "marquee");
    assert!(entry["error"].as_str().unwrap().contains("invalid JSON"));
}

#[test]
fn s8_a_missing_21st_tag_skips_21st_without_failing_the_search() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "dialog")]);
    let result = cli(
        &["search", "dialog", "--json"],
        &mock(|_| Reply::Respond(404, "nope".into())),
        &cache,
    );
    assert_eq!(result.code, 0);
    let body = result.json();
    assert_eq!(body["results"][0]["id"], "shadcn/dialog");
    assert!(body["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["source"] == "21st"));
}

#[test]
fn s18_21st_without_a_key_get_exits_3_and_writes_nothing() {
    let (cache, out) = (temp(), temp());
    let server = mock(|_| {
        ok("# X\n\n> d\n\n- License: mit\n- Live preview: https://21st.dev/@a/components/b\n")
    });
    let result = cli(
        &["get", "21st/a/b", "--out", out.to_str().unwrap()],
        &server,
        &cache,
    );
    assert_eq!(result.code, 3);
    assert!(entries(&out).is_empty());
}

#[test]
fn s19_paths_leaving_the_example_folder_are_rejected() {
    for unsafe_path in [
        "../../x",
        "/etc/passwd",
        "~/x",
        "C:\\x",
        "a/../../b",
        "a/./b",
    ] {
        assert_eq!(safe_relative(unsafe_path), None, "{unsafe_path}");
    }
    assert_eq!(
        safe_relative("components/ui/a.tsx").as_deref(),
        Some("components/ui/a.tsx")
    );
}

#[test]
fn s20_malformed_ids_fail_before_any_network_call() {
    for id in ["magicui/../secret", "magicui/-rf", "nope"] {
        let server = hanging();
        let result = cli(&["show", id], &server, &temp());
        assert_eq!(result.code, 2, "{id}");
        assert!(server.calls.lock().unwrap().is_empty(), "{id}");
    }
}

#[test]
fn s21_project_without_package_json_exits_2() {
    let directory = temp();
    assert_eq!(
        cli(
            &[
                "show",
                "shadcn/dialog",
                "--project",
                directory.to_str().unwrap()
            ],
            &hanging(),
            &temp()
        )
        .code,
        2
    );
}

#[test]
fn s23_unknown_command_or_source_exits_2_install_commands_do_not_exist() {
    let server = hanging();
    assert_eq!(cli(&["frobnicate"], &server, &temp()).code, 2);
    assert_eq!(cli(&["add", "shadcn/dialog"], &server, &temp()).code, 2);
    assert_eq!(
        cli(&["search", "x", "--source", "nope"], &server, &temp()).code,
        2
    );
}

fn all_files(directory: &Path) -> String {
    entries(directory)
        .iter()
        .map(|file| {
            let path = directory.join(file);
            if path.is_dir() {
                all_files(&path)
            } else {
                String::from_utf8_lossy(&fs::read(path).unwrap()).into_owned()
            }
        })
        .collect()
}

#[test]
fn s22_secrets_never_reach_stdout_stderr_or_the_cache() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "dialog")]);
    let server = hanging();
    let result = cli_env(
        &["search", "dialog", "--json"],
        &server,
        &cache,
        vec![
            ("API_KEY_21ST", "TEST_KEY_123"),
            ("GEMINI_API_KEY", "TEST_GEMINI_456"),
        ],
    );
    let written = all_files(&cache);
    for leak in ["TEST_KEY_123", "TEST_GEMINI_456"] {
        assert!(
            !result.out.contains(leak) && !result.err.contains(leak) && !written.contains(leak),
            "{leak}"
        );
    }
    let calls = server.calls.lock().unwrap();
    assert!(!calls.is_empty());
    for call in calls.iter() {
        assert!(!call.path.contains("TEST_"), "{}", call.path);
    }
}

#[test]
fn the_gemini_key_is_sent_only_in_the_x_goog_api_key_header() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|_| Reply::Respond(500, "{}".into()));
    cli_env(
        &["search", "button", "--source", "magicui", "--json"],
        &server,
        &cache,
        vec![("GEMINI_API_KEY", "TEST_GEMINI_456")],
    );
    let calls = server.calls.lock().unwrap();
    assert!(calls
        .iter()
        .any(|call| call.path.contains("batchEmbedContents")));
    for call in calls.iter() {
        assert!(!call.path.contains("TEST_GEMINI"));
        for (name, value) in &call.headers {
            if value.contains("TEST_GEMINI") {
                assert_eq!(name, "x-goog-api-key");
            }
        }
        if call.path.contains("batchEmbedContents") {
            assert_eq!(call.headers["x-goog-api-key"], "TEST_GEMINI_456");
        }
    }
}

#[test]
fn gemini_failure_falls_back_to_token_search_with_a_warning() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "dialog")]);
    let body = cli_env(
        &["search", "dialog", "--source", "shadcn", "--json"],
        &hanging(),
        &cache,
        vec![("GEMINI_API_KEY", "x")],
    )
    .json();
    assert_eq!(body["results"][0]["id"], "shadcn/dialog");
    assert!(body["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("semantic")));
}

#[test]
fn the_key_is_read_from_the_env_file_only_as_google_api_key() {
    let file = temp().join(".env");
    fs::write(&file, "OTHER=1\nGOOGLE_API_KEY=\"abc\"\n").unwrap();
    assert_eq!(
        read_key_from(None, Some(file.to_string_lossy().into_owned())).as_deref(),
        Some("abc")
    );
    assert_eq!(read_key_from(None, Some("/nonexistent".into())), None);
}

fn similarity(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
    pairs
        .iter()
        .map(|(id, value)| (id.to_string(), *value))
        .collect()
}

fn ranking(gap_penalty: f64, curated_bonus: f64, min_similarity: f64) -> Ranking {
    Ranking {
        gap_penalty,
        curated_bonus,
        min_similarity,
    }
}

#[test]
fn semantic_order_follows_cosine_small_nudges_only_separate_near_ties() {
    let candidates = [
        item_with("a", "toast", json!({ "gaps": ["needs-key"] })),
        item("b", "toast"),
        item("c", "menu"),
    ];
    let scores = similarity(&[("a/toast", 0.8), ("b/toast", 0.79), ("c/menu", 0.2)]);
    let limits = no_gaps(20);
    let bonus = RANKING.curated_bonus;
    assert_eq!(
        rank_semantic(
            &candidates,
            &scores,
            limits,
            ranking(0.0, 0.0, RANKING.min_similarity)
        )[0]["id"],
        "a/toast"
    );
    assert_eq!(
        rank_semantic(
            &candidates,
            &scores,
            limits,
            ranking(0.02, bonus, RANKING.min_similarity)
        )[0]["id"],
        "b/toast"
    );
    assert_eq!(
        rank_semantic(&candidates, &scores, limits, ranking(0.02, bonus, 0.0))
            .last()
            .unwrap()["id"],
        "c/menu"
    );
    assert_eq!(
        rank_semantic(&candidates, &scores, limits, ranking(0.02, bonus, 0.6)).len(),
        2
    );
}

#[test]
fn bm25_finds_a_multi_word_description_by_any_matching_word() {
    let candidates = [
        item_with("a", "command", json!({ "description": "palette" })),
        item("b", "input"),
    ];
    assert_eq!(
        rank(&candidates, "palette of commands", no_gaps(20))[0]["id"],
        "a/command"
    );
}

#[test]
fn a_missing_demo_page_or_maintenance_note_never_changes_the_order() {
    let candidates = [
        item_with(
            "a",
            "cmdk",
            json!({ "gaps": ["no-preview", "unmaintained"] }),
        ),
        item("b", "other"),
    ];
    let scores = similarity(&[("a/cmdk", 0.8), ("b/other", 0.79)]);
    assert_eq!(
        rank_semantic(
            &candidates,
            &scores,
            no_gaps(20),
            ranking(0.5, 0.0, RANKING.min_similarity)
        )[0]["id"],
        "a/cmdk"
    );
}

#[test]
#[cfg(unix)]
fn r01_get_force_refuses_to_write_through_symlinks() {
    let (cache, out, outside) = (temp(), temp(), temp());
    seed_index(&cache, "magicui", vec![magic_item()]);
    fs::create_dir_all(out.join("magicui/button")).unwrap();
    symlink(&outside, out.join("magicui/button/nested")).unwrap();
    let server = registry_mock(
        json!({ "files": [{ "path": "nested/button.tsx", "content": "export const a = 1" }] }),
    );
    let result = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--force",
        ],
        &server,
        &cache,
    );
    assert_eq!(result.code, 1);
    assert!(entries(&outside).is_empty());
}

#[test]
#[cfg(unix)]
fn r01_a_symlinked_final_file_or_clayui_json_is_not_followed() {
    let (cache, out) = (temp(), temp());
    let victim = temp().join("victim.txt");
    fs::write(&victim, "original").unwrap();
    seed_index(&cache, "magicui", vec![magic_item()]);
    fs::create_dir_all(out.join("magicui/button")).unwrap();
    symlink(&victim, out.join("magicui/button/clayui.json")).unwrap();
    let server =
        registry_mock(json!({ "files": [{ "path": "a.tsx", "content": "export const a = 1" }] }));
    cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--force",
        ],
        &server,
        &cache,
    );
    assert_eq!(fs::read_to_string(&victim).unwrap(), "original");
}

#[test]
fn r02_a_provider_response_echoing_a_secret_never_reaches_the_output() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|call| {
        ok(call
            .headers
            .get("x-goog-api-key")
            .cloned()
            .unwrap_or_else(|| "{}".into()))
    });
    let result = cli_env(
        &["search", "button", "--source", "magicui", "--json"],
        &server,
        &cache,
        vec![("GEMINI_API_KEY", "TEST_GEMINI_456")],
    );
    assert_eq!(result.code, 0);
    assert!(!result.out.contains("TEST_GEMINI_456") && !result.err.contains("TEST_GEMINI_456"));
    assert!(!all_files(&cache).contains("TEST_GEMINI_456"));
}

#[test]
fn r04_a_damaged_source_entry_does_not_stop_healthy_ones() {
    let cache = temp();
    let body = json!({ "sources": { "magicui": { "items": null }, "shadcn": { "items": [null, item("shadcn", "dialog")], "syncedAt": iso_now(), "error": null } } });
    fs::write(cache.join("index.json"), body.to_string()).unwrap();
    let result = cli(
        &["search", "dialog", "--source", "shadcn", "--json"],
        &hanging(),
        &cache,
    );
    assert_eq!(result.code, 0);
    assert_eq!(result.json()["results"][0]["id"], "shadcn/dialog");
}

#[test]
fn a_damaged_vector_cache_is_rebuilt_instead_of_failing() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    fs::write(cache.join("embeddings.json"), "{ not json").unwrap();
    let vector: Vec<f64> = (0..768).map(|index| f64::from(index + 1)).collect();
    let server = mock(move |_| ok(json!({ "embeddings": [{ "values": vector }] }).to_string()));
    let result = cli_env(
        &["search", "button", "--source", "magicui", "--json"],
        &server,
        &cache,
        vec![("GEMINI_API_KEY", "x")],
    );
    assert_eq!(result.code, 0);
    assert_eq!(result.json()["results"][0]["id"], "magicui/button");
    assert!(
        read_json(cache.join("embeddings.json"))
            .as_object()
            .unwrap()
            .len()
            == 1
    );
}

#[test]
fn r07_invalid_numeric_flags_exit_2() {
    let server = hanging();
    let cases: [&[&str]; 6] = [
        &["--limit", "abc"],
        &["--limit=-1"],
        &["--limit=1.5"],
        &["--limit=Infinity"],
        &["--details", "abc"],
        &["--details=-1"],
    ];
    for flags in cases {
        let mut arguments = vec!["search", "x"];
        arguments.extend_from_slice(flags);
        assert_eq!(cli(&arguments, &server, &temp()).code, 2, "{flags:?}");
    }
}

#[test]
fn r08_when_every_path_is_unsafe_get_exits_3_and_creates_nothing() {
    let (cache, out) = (temp(), temp());
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server =
        registry_mock(json!({ "files": [{ "path": "../../outside.tsx", "content": "x" }] }));
    let result = cli(
        &["get", "magicui/button", "--out", out.to_str().unwrap()],
        &server,
        &cache,
    );
    assert_eq!(result.code, 3);
    assert!(!out.join("magicui").exists());
}

#[test]
fn r05_a_detail_with_no_code_is_no_longer_reported_as_free_of_gaps() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let shown = cli(
        &["show", "magicui/button", "--json"],
        &registry_mock(json!({ "files": [] })),
        &cache,
    )
    .json();
    assert!(shown["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap == "no-source-listing"));
}

// --- init -------------------------------------------------------------------

const AGENTS_SKILL: &str = ".agents/skills/clay-ui/SKILL.md";
const CLAUDE_SKILL: &str = ".claude/skills/clay-ui/SKILL.md";

fn init(args: &[&str], cwd: &Path, home: &Path) -> Run {
    let mut arguments = vec!["init"];
    arguments.extend_from_slice(args);
    cli_with(
        &arguments,
        &hanging(),
        Options {
            cache: &temp(),
            env: vec![],
            cwd: Some(cwd),
            home: Some(home),
        },
    )
}

#[test]
fn init_writes_both_skills_into_the_project_only() {
    let (cwd, home) = (temp(), temp());
    let result = init(&["--no-sync"], &cwd, &home);
    assert_eq!(result.code, 0, "{}", result.err);
    let expected = include_str!("../skills/clay-ui/SKILL.md");
    assert_eq!(
        fs::read_to_string(cwd.join(AGENTS_SKILL)).unwrap(),
        expected
    );
    assert_eq!(
        fs::read_to_string(cwd.join(CLAUDE_SKILL)).unwrap(),
        expected
    );
    assert!(entries(&home).is_empty());
    assert!(result.out.contains("Done.") && result.out.contains("Skill installed"));
    assert!(result.out.contains("Tip: set GEMINI_API_KEY"));
}

#[test]
fn init_global_writes_into_home_only() {
    let (cwd, home) = (temp(), temp());
    assert_eq!(init(&["--global", "--no-sync"], &cwd, &home).code, 0);
    assert!(home.join(AGENTS_SKILL).exists() && home.join(CLAUDE_SKILL).exists());
    assert!(entries(&cwd).is_empty());
}

#[test]
fn init_target_selects_one_folder() {
    let (agents, claude, home) = (temp(), temp(), temp());
    assert_eq!(
        init(&["--target", "agents", "--no-sync"], &agents, &home).code,
        0
    );
    assert!(agents.join(AGENTS_SKILL).exists() && !agents.join(".claude").exists());
    assert_eq!(
        init(&["--target=claude", "--no-sync"], &claude, &home).code,
        0
    );
    assert!(claude.join(CLAUDE_SKILL).exists() && !claude.join(".agents").exists());
}

#[test]
fn init_rejects_an_unknown_target() {
    let (cwd, home) = (temp(), temp());
    assert_eq!(
        init(&["--target", "cursor", "--no-sync"], &cwd, &home).code,
        2
    );
    assert!(entries(&cwd).is_empty());
}

#[test]
#[cfg(unix)]
fn init_refuses_a_symlinked_folder_and_leaves_its_target_alone() {
    let (cwd, home, outside) = (temp(), temp(), temp());
    symlink(&outside, cwd.join(".claude")).unwrap();
    assert_eq!(
        init(&["--target", "claude", "--no-sync"], &cwd, &home).code,
        1
    );
    assert!(entries(&outside).is_empty());
}

#[test]
#[cfg(unix)]
fn init_does_not_follow_a_symlinked_skill_file() {
    let (cwd, home) = (temp(), temp());
    let victim = temp().join("victim.md");
    fs::write(&victim, "original").unwrap();
    fs::create_dir_all(cwd.join(".agents/skills/clay-ui")).unwrap();
    symlink(&victim, cwd.join(AGENTS_SKILL)).unwrap();
    assert_eq!(
        init(&["--target", "agents", "--no-sync"], &cwd, &home).code,
        1
    );
    assert_eq!(fs::read_to_string(&victim).unwrap(), "original");
}

#[test]
fn init_without_no_sync_preserves_the_index_and_reports_failing_sources() {
    let (cwd, home, cache) = (temp(), temp(), temp());
    let result = cli_with(
        &["init"],
        &mock(|_| Reply::Respond(500, "x".into())),
        Options {
            cache: &cache,
            env: vec![],
            cwd: Some(&cwd),
            home: Some(&home),
        },
    );
    assert_eq!(result.code, 1, "{}", result.err);
    assert!(result.out.contains("shadcn: ERROR") && result.out.contains("embeddings: no key"));
    assert_eq!(
        read_json(cache.join("index.json"))["sources"]
            .as_object()
            .unwrap()
            .len(),
        clayui_lib::sources::SOURCES
            .iter()
            .filter(|s| !s.live)
            .count()
    );
}

// --- CLI contract details ---------------------------------------------------

#[test]
fn output_is_english_only() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = registry_mock(
        json!({ "files": [{ "path": "a.tsx", "content": "x" }], "dependencies": ["a"], "registryDependencies": ["b"] }),
    );
    let shown = cli(&["show", "magicui/button"], &server, &cache);
    assert!(shown.out.contains("registry:  b") && shown.out.contains("file:      a.tsx"));
    let searched = cli(
        &["search", "button", "--source", "magicui"],
        &server,
        &cache,
    );
    assert!(searched.err.contains("Warning: No Gemini key"));
    for word in [
        "Uwaga",
        "UWAGA",
        "rejestr",
        "plik",
        "projekt",
        "niepoprawny",
        "Niepoprawny",
    ] {
        for text in [&shown.out, &shown.err, &searched.out, &searched.err] {
            assert!(!text.contains(word), "{word}");
        }
    }
}

#[test]
fn get_writes_files_usage_and_metadata() {
    let (cache, out) = (temp(), temp());
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = registry_mock(
        json!({ "files": [{ "path": "src/a.tsx", "content": "export const a = 1" }, { "path": "../bad.tsx", "content": "x" }] }),
    );
    let result = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(result.code, 0, "{}", result.err);
    let destination = out.join("magicui/button");
    assert_eq!(
        fs::read_to_string(destination.join("src/a.tsx")).unwrap(),
        "export const a = 1"
    );
    let meta = read_json(destination.join("clayui.json"));
    assert_eq!(meta["files"], json!(["src/a.tsx"]));
    assert_eq!(meta["skipped"], json!(["../bad.tsx"]));
    assert!(result.err.contains("Skipped unsafe path: ../bad.tsx"));
    assert_eq!(result.json()["destination"], destination.to_str().unwrap());
    let again = cli(
        &["get", "magicui/button", "--out", out.to_str().unwrap()],
        &server,
        &cache,
    );
    assert_eq!(again.code, 2, "an existing folder needs --force");
}

#[test]
fn a_21st_response_echoing_a_key_is_redacted_from_output_and_cache() {
    let cache = temp();
    let server = mock(|call| {
        let echoed = call
            .headers
            .get("authorization")
            .cloned()
            .unwrap_or_else(|| "secret-token-value".into());
        if call.path.contains("/install/") {
            ok(json!({ "files": [{ "path": "a.tsx", "content": echoed }] }).to_string())
        } else {
            ok(format!("# {echoed}\n\n> d\n\n- License: mit\n"))
        }
    });
    let result = cli_env(
        &["show", "21st/a/b", "--json", "--code"],
        &server,
        &cache,
        vec![("API_KEY_21ST", "secret-token-value")],
    );
    assert_eq!(result.code, 0, "{}", result.err);
    assert!(
        !result.out.contains("secret-token-value") && !result.err.contains("secret-token-value")
    );
}

#[test]
fn usage_only_after_github_403_does_not_succeed_or_create_output() {
    let cache = temp();
    seed_index(
        &cache,
        "sonner",
        vec![item_with(
            "sonner",
            "sonner",
            json!({
                "usageUrl": "https://example.com/usage",
                "sourceCode": { "repo": "owner/repo", "dir": "src" }
            }),
        )],
    );
    let server = mock(|call| {
        if call.path.contains("/usage") {
            ok("# Usage\nimport { toast } from 'sonner'")
        } else {
            Reply::Respond(403, "forbidden".into())
        }
    });
    let out = temp().join("examples");
    let result = cli(
        &[
            "get",
            "sonner/sonner",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(result.code, 3);
    assert!(result.out.is_empty());
    assert!(result.err.contains("403") && result.err.contains("no source files"));
    assert!(!out.exists());
    let shown = cli(&["show", "sonner/sonner", "--json"], &server, &cache);
    assert!(shown.json()["gaps"]
        .as_array()
        .unwrap()
        .contains(&json!("no-source-listing")));
}

#[test]
fn sync_json_failure_is_machine_readable_and_keeps_healthy_data() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "table")]);
    let server = mock(|_| Reply::Respond(403, "forbidden".into()));
    let result = cli(&["sync", "--source", "shadcn", "--json"], &server, &cache);
    assert_eq!(result.code, 1);
    assert_eq!(result.json()["ok"], false);
    assert_eq!(result.json()["failed"][0]["source"], "shadcn");
    assert!(result.json()["failed"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("403"));
    assert_eq!(
        read_json(cache.join("index.json"))["sources"]["shadcn"]["items"][0]["name"],
        "table"
    );
}

#[test]
fn cn_helper_is_not_a_missing_package_but_real_packages_are() {
    let project = clayui_lib::project::Project {
        react: json!("19"),
        tailwind: Value::Null,
        dependencies: json!({"react":"19"}).as_object().unwrap().clone(),
    };
    let detail = json!({"dependencies":["cn","react","clsx","@radix-ui/react-dialog@1.0"]})
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        clayui_lib::project::missing_dependencies(&detail, &project),
        vec![json!("clsx"), json!("@radix-ui/react-dialog@1.0")]
    );
}

#[test]
fn json_usage_errors_keep_stdout_empty_and_exit_two() {
    let result = cli(&["get", "invalid", "--json"], &hanging(), &temp());
    assert_eq!(result.code, 2);
    assert!(result.out.is_empty());
    assert!(result.err.contains("Invalid id"));
}

fn static_item() -> Value {
    json!({"id":"magicui/button","source":"magicui","name":"button","title":"Button","description":"A source-backed example","keywords":[],"gaps":[],"dependencies":[],"registryDependencies":[],"license":"MIT","files":[{"path":"button.tsx","content":"export function Button() {}"}]})
}

#[test]
fn static_catalog_fetch_has_no_credentials_and_get_needs_no_origin_request() {
    let cache = temp();
    let server = mock(|call| {
        if call.path.starts_with("/unit.invalid/") {
            ok(json!({"schema":1,"items":[static_item()]}).to_string())
        } else if call.path.contains("generativelanguage") {
            ok(json!({"embeddings":[{"values":vec![1.0;768]}]}).to_string())
        } else {
            Reply::Respond(500, "unexpected origin request".into())
        }
    });
    let result = cli_env(
        &["sync", "--source", "magicui", "--json"],
        &server,
        &cache,
        vec![
            ("CLAYUI_CDN_BASE_URL", "https://unit.invalid/v1"),
            ("GITHUB_TOKEN", "fake-github-secret"),
            ("GEMINI_API_KEY", "fake-gemini-secret"),
        ],
    );
    assert_eq!(result.code, 0, "{}", result.err);
    let calls = server.calls.lock().unwrap();
    let cdn: Vec<_> = calls
        .iter()
        .filter(|call| call.path.starts_with("/unit.invalid/"))
        .collect();
    assert_eq!(cdn.len(), 1);
    assert!(!cdn[0].headers.contains_key("authorization"));
    assert!(!cdn[0].headers.contains_key("x-goog-api-key"));
    drop(calls);
    let out = temp();
    let result = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &hanging(),
        &cache,
    );
    assert_eq!(result.code, 0, "{}", result.err);
    assert_eq!(
        fs::read_to_string(out.join("magicui/button/button.tsx")).unwrap(),
        "export function Button() {}"
    );
    let result = cli(
        &["search", "button", "--source", "magicui", "--json"],
        &hanging(),
        &cache,
    );
    assert!(result.json()["results"][0].get("files").is_none());
    let result = cli(
        &[
            "search",
            "button",
            "--source",
            "magicui",
            "--details",
            "1",
            "--code",
            "--json",
        ],
        &hanging(),
        &cache,
    );
    assert_eq!(
        result.json()["results"][0]["files"][0]["content"],
        "export function Button() {}"
    );
}

#[test]
fn invalid_static_catalog_falls_back_to_current_source() {
    let cache = temp();
    let server = mock(|call| {
        if call.path.starts_with("/unit.invalid/") {
            ok("{\"schema\":999,\"items\":[]}")
        } else {
            ok(json!({"items":[{"name":"button","type":"registry:ui"}]}).to_string())
        }
    });
    let result = cli_env(
        &["sync", "--source", "magicui", "--json"],
        &server,
        &cache,
        vec![("CLAYUI_CDN_BASE_URL", "https://unit.invalid")],
    );
    assert_eq!(result.code, 0, "{}", result.err);
    assert!(result.err.contains("using current sources"));
    assert_eq!(
        read_json(cache.join("index.json"))["sources"]["magicui"]["items"][0]["id"],
        "magicui/button"
    );
}

#[test]
fn download_writes_scrub_source_usage_metadata_and_cache() {
    let (cache, out) = (temp(), temp());
    seed_index(&cache, "magicui", vec![magic_item()]);
    let secret = "TEST_DOWNLOAD_CREDENTIAL";
    let server = registry_mock(
        json!({"description":secret,"files":[{"path":"button.tsx","content":format!("export const x = '{secret}'")}],"usage":secret}),
    );
    let result = cli_env(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
        vec![("GITHUB_TOKEN", secret)],
    );
    assert_eq!(result.code, 0, "{}", result.err);
    assert!(!all_files(&out).contains(secret));
    assert!(!all_files(&cache).contains(secret));
    assert!(all_files(&out).contains("[redacted]"));
}

#[test]
fn cache_benchmark_cold_warm_gemini_calls_and_offline_component() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|call| {
        std::thread::sleep(Duration::from_millis(25));
        if call.path.contains("batchEmbedContents") {
            ok(json!({"embeddings":[{"values":vec![1.0;768]}]}).to_string())
        } else {
            ok(json!({"files":[{"path":"button.tsx","content":"export const x=1"}]}).to_string())
        }
    });
    let args = ["search", "button", "--source", "magicui", "--json"];
    let start = std::time::Instant::now();
    let first = cli_env(
        &args,
        &server,
        &cache,
        vec![("GEMINI_API_KEY", "TEST_BENCHMARK_KEY")],
    );
    let cold = start.elapsed();
    assert_eq!(first.code, 0);
    let calls = || {
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.path.contains("batchEmbedContents"))
            .count()
    };
    let before = calls();
    let start = std::time::Instant::now();
    let second = cli_env(
        &args,
        &server,
        &cache,
        vec![("GEMINI_API_KEY", "TEST_BENCHMARK_KEY")],
    );
    let warm = start.elapsed();
    assert_eq!(second.code, 0);
    assert_eq!(before, 2);
    assert_eq!(calls() - before, 0);
    println!(
        "CACHE_BENCH query cold_ms={:.3} warm_ms={:.3} Gemini_calls_cold={} Gemini_calls_warm={}",
        cold.as_secs_f64() * 1000.,
        warm.as_secs_f64() * 1000.,
        before,
        calls() - before
    );
    let out1 = temp();
    let out2 = temp();
    let start = std::time::Instant::now();
    assert_eq!(
        cli(
            &[
                "get",
                "magicui/button",
                "--out",
                out1.to_str().unwrap(),
                "--json"
            ],
            &server,
            &cache
        )
        .code,
        0
    );
    let cold = start.elapsed();
    let before = server.calls.lock().unwrap().len();
    let start = std::time::Instant::now();
    let result = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out2.to_str().unwrap(),
            "--json",
        ],
        &hanging(),
        &cache,
    );
    let warm = start.elapsed();
    assert_eq!(result.code, 0);
    assert!(all_files(&out2).contains("export const x=1"));
    assert_eq!(before, server.calls.lock().unwrap().len());
    println!(
        "CACHE_BENCH component cold_ms={:.3} offline_warm_ms={:.3}",
        cold.as_secs_f64() * 1000.,
        warm.as_secs_f64() * 1000.
    );
}

#[cfg(unix)]
#[test]
fn init_preflight_prevents_partial_skill_install_and_errno40_is_clear() {
    let (cwd, home, outside) = (temp(), temp(), temp());
    symlink(&outside, cwd.join(".claude")).unwrap();
    let result = init(&["--no-sync"], &cwd, &home);
    assert_eq!(result.code, 1);
    assert!(!cwd.join(".agents").exists());
    assert!(result.err.contains("Refusing to write"));
    fs::remove_file(cwd.join(".claude")).unwrap();
    fs::create_dir_all(cwd.join(".claude/skills/clay-ui/SKILL.md")).unwrap();
    assert_eq!(init(&["--no-sync"], &cwd, &home).code, 1);
    assert!(!cwd.join(".agents").exists());
    let target = outside.join("file");
    fs::write(&target, "safe").unwrap();
    let link = cwd.join("link");
    symlink(&target, &link).unwrap();
    assert!(clayui_lib::fsafe::write_no_follow(&link, b"bad")
        .unwrap_err()
        .contains("Refusing to write"));
}

#[cfg(unix)]
#[test]
fn refuses_non_utf8_arguments_and_empty_home() {
    use std::os::unix::ffi::OsStringExt;
    let cwd = temp();
    let p = Command::new(env!("CARGO_BIN_EXE_clayui"))
        .env_clear()
        .env("HOME", &cwd)
        .current_dir(&cwd)
        .arg(std::ffi::OsString::from_vec(vec![0xe9]))
        .output()
        .unwrap();
    assert_eq!(p.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&p.stderr).contains("valid UTF-8"));
    let p = Command::new(env!("CARGO_BIN_EXE_clayui"))
        .env_clear()
        .env("HOME", "")
        .current_dir(&cwd)
        .args(["init", "--global", "--target", "agents", "--no-sync"])
        .output()
        .unwrap();
    assert_eq!(p.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&p.stderr).contains("Home directory is empty"));
    assert!(entries(&cwd).is_empty());
}

#[test]
fn implicit_sync_survives_unwritable_cache() {
    let (cache, cwd, home) = (temp(), temp(), temp());
    let blocked = cache.join("not-a-folder");
    fs::write(&blocked, "file").unwrap();
    let server =
        mock(|_| ok(json!([{"name":"button","type":"registry:ui","title":"Button"}]).to_string()));
    let p = cli_with(
        &["search", "button", "--source", "magicui", "--json"],
        &server,
        Options {
            cache: &blocked,
            env: vec![],
            cwd: Some(&cwd),
            home: Some(&home),
        },
    );
    assert_eq!(p.code, 0, "{}", p.err);
    assert!(p.err.contains("in-memory index"));
    assert!(!p.json()["results"].as_array().unwrap().is_empty());
}

#[test]
fn query_cache_preserves_exact_text_and_model_and_recovers_damage() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|_| ok(json!({"embeddings":[{"values":vec![1.0;768]}]}).to_string()));
    for (query, model, expected) in [
        ("button", "model-a", 2),
        ("button", "model-a", 2),
        ("Button", "model-a", 3),
        ("button", "model-b", 5),
    ] {
        let r = cli_env(
            &["search", query, "--source", "magicui", "--json"],
            &server,
            &cache,
            vec![
                ("GEMINI_API_KEY", "TEST_QUERY_KEY"),
                ("CLAYUI_EMBED_MODEL", model),
            ],
        );
        assert_eq!(r.code, 0);
        assert_eq!(server.calls.lock().unwrap().len(), expected);
    }
    for entry in fs::read_dir(cache.join("queries")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "json") {
            fs::write(entry.path(), "damaged").unwrap();
        }
    }
    let r = cli_env(
        &["search", "button", "--source", "magicui", "--json"],
        &server,
        &cache,
        vec![
            ("GEMINI_API_KEY", "TEST_QUERY_KEY"),
            ("CLAYUI_EMBED_MODEL", "model-b"),
        ],
    );
    assert_eq!(r.code, 0);
    assert_eq!(server.calls.lock().unwrap().len(), 6);
    assert!(!all_files(&cache).contains("TEST_QUERY_KEY"));
}

#[test]
fn stale_download_warning_is_in_json_and_stderr() {
    let (cache, out) = (temp(), temp());
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server =
        registry_mock(json!({"files":[{"path":"button.tsx","content":"export const cached=1"}]}));
    assert_eq!(
        cli(
            &[
                "get",
                "magicui/button",
                "--out",
                out.to_str().unwrap(),
                "--json"
            ],
            &server,
            &cache
        )
        .code,
        0
    );
    for entry in fs::read_dir(cache.join("components")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "json") {
            let mut v: Value = read_json(entry.path());
            let created = v["created"].as_u64().unwrap() - 8 * 86400;
            v["created"] = json!(created);
            v["sha256"] = json!(clayui_lib::cache::hash(
                json!([v["key"], created, v["data"]]).to_string().as_bytes()
            ));
            fs::write(entry.path(), v.to_string()).unwrap();
        }
    }
    let r = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--force",
            "--json",
        ],
        &mock(|_| Reply::Respond(403, "blocked".into())),
        &cache,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(r.json()["cacheAgeDays"], 8);
    assert!(r.json()["cacheWarning"]
        .as_str()
        .unwrap()
        .contains("8 days"));
    assert!(r.err.contains("8 days"));
}

// Release fidelity regressions: all endpoints are mocked; downloaded code is never executed.
#[test]
fn kokonut_listing_authenticates_the_tree_like_radix() {
    let server = mock(|call| {
        if call.path.contains("api.github.com") {
            if call.headers.get("authorization").map(String::as_str) != Some("Bearer fixture-token")
            {
                return Reply::Respond(403, "anonymous quota exhausted".into());
            }
            ok(json!({"tree":[{"path":"content/docs/components/button.mdx"}]}).to_string())
        } else {
            ok(json!({"items":[{"name":"button","type":"registry:ui"}]}).to_string())
        }
    });
    let cache = temp();
    let failed = cli(&["sync", "--source", "kokonut", "--json"], &server, &cache);
    assert_eq!(failed.code, 1);
    let result = cli_env(
        &["sync", "--source", "kokonut", "--json"],
        &server,
        &cache,
        vec![("GITHUB_TOKEN", "fixture-token")],
    );
    assert_eq!(result.code, 0, "{}", result.err);
    assert_eq!(
        read_json(cache.join("index.json"))["sources"]["kokonut"]["items"][0]["name"],
        "button"
    );
    assert!(!result.out.contains("fixture-token") && !result.err.contains("fixture-token"));
}

#[test]
fn missing_local_imports_are_metadata_and_do_not_block_get_or_change_ranking() {
    for (source, name, path, content, missing) in [
        (
            "base-ui",
            "separator",
            "source/Separator.tsx",
            "import { x } from '../../utils/x'; export * from './sibling';",
            "../../utils/x",
        ),
        (
            "rareui",
            "LiquidWaveLazy",
            "registry/ui/LiquidWaveLazy/index.tsx",
            "const Lazy = lazy(() => import('../LiquidWave'));",
            "../LiquidWave",
        ),
    ] {
        let cache = temp();
        seed_index(
            &cache,
            source,
            vec![item_with(
                source,
                name,
                json!({
                    "sourceCode":{"repo":"owner/repo", "dir":"src"}, "usageUrl":"https://example.com/usage"
                }),
            )],
        );
        let server = mock(move |call| {
            if call.path.contains("/commits/") {
                return ok(json!({"sha":"abc123"}).to_string());
            }
            if call.path.contains("/git/trees/") {
                return ok(json!({"tree":[{"path":"src/Separator.tsx"}]}).to_string());
            }
            if call.path.contains("raw.githubusercontent") {
                return ok(content);
            }
            if call.path.contains("shitui.in") {
                return ok(json!({"files":[{"path":path,"content":content}]}).to_string());
            }
            ok("# Usage")
        });
        let id = format!("{source}/{name}");
        let before = cli(
            &["search", name, "--source", source, "--json"],
            &server,
            &cache,
        );
        let out = temp();
        let result = cli(
            &["get", &id, "--out", out.to_str().unwrap(), "--json"],
            &server,
            &cache,
        );
        assert_eq!(result.code, 0, "{}", result.err);
        assert!(result.json()["gaps"]
            .as_array()
            .unwrap()
            .contains(&json!("local-imports-missing")));
        assert!(result.json()["missingLocalImports"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["import"] == missing));
        let meta = read_json(out.join(&id).join("clayui.json"));
        assert_eq!(
            meta["missingLocalImports"],
            result.json()["missingLocalImports"]
        );
        let after = cli(
            &["search", name, "--source", source, "--json"],
            &server,
            &cache,
        );
        assert_eq!(before.json()["results"], after.json()["results"]);
    }
}

#[test]
fn repository_provenance_pins_commit_hashes_usage_and_lists_omitted_files() {
    let cache = temp();
    seed_index(
        &cache,
        "base-ui",
        vec![item_with(
            "base-ui",
            "tabs",
            json!({
                "usageUrl":"https://example.com/usage", "sourceCode":{"repo":"owner/repo", "dir":"src"}
            }),
        )],
    );
    let server = mock(|call| {
        if call.path.contains("/commits/HEAD") {
            return ok(json!({"sha":"commit123"}).to_string());
        }
        if call.path.contains("/git/trees/commit123?") {
            return ok(json!({"sha":"tree456", "tree":[
            {"path":"src/index.ts", "type":"blob"}, {"path":"src/prehydrationScript.template.js", "type":"blob"},
            {"path":"src/index.test.ts", "type":"blob"}, {"path":"src/folder", "type":"tree"}
        ]}).to_string());
        }
        if call.path.contains("/commit123/src/index.ts") {
            return ok("export const x = 1;\n");
        }
        if call.path.contains("/usage") {
            return ok("# Usage\n");
        }
        Reply::Respond(404, "unexpected URL".into())
    });
    let out = temp();
    let result = cli(
        &[
            "get",
            "base-ui/tabs",
            "--json",
            "--out",
            out.to_str().unwrap(),
        ],
        &server,
        &cache,
    );
    assert_eq!(result.code, 0, "{}", result.err);
    let meta = read_json(out.join("base-ui/tabs/clayui.json"));
    assert_eq!(
        meta["sourceUrl"],
        "https://github.com/owner/repo/tree/commit123/src"
    );
    assert_eq!(meta["sourceRef"], "commit123");
    assert_eq!(
        meta["skipped"],
        json!([
            "source/prehydrationScript.template.js",
            "source/index.test.ts"
        ])
    );
    for file in meta["files"].as_array().unwrap() {
        let file = file.as_str().unwrap();
        assert_eq!(
            meta["fileSha256"][file],
            clayui_lib::cache::hash(&fs::read(out.join("base-ui/tabs").join(file)).unwrap())
        );
    }
    assert_eq!(meta["fileSha256"].as_object().unwrap().len(), 2);
}

#[test]
fn registry_provenance_version_skipped_and_preview_match_show_get_and_manifest() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|call| {
        if call.path.contains("x.dev") {
            return Reply::Respond(404, "gone".into());
        }
        ok(json!({"version":"1.2.3", "ref":"release-tag", "files":[
            {"path":"src/a.tsx", "content":"export const a = 1;\n"}, {"path":"empty.ts", "content":""}, {"path":"missing.ts"}
        ]}).to_string())
    });
    let shown = cli(&["show", "magicui/button", "--json"], &server, &cache);
    let out = temp();
    let got = cli(
        &[
            "get",
            "magicui/button",
            "--json",
            "--out",
            out.to_str().unwrap(),
        ],
        &server,
        &cache,
    );
    assert_eq!(got.code, 0, "{}", got.err);
    let meta = read_json(out.join("magicui/button/clayui.json"));
    for value in [shown.json(), got.json(), meta.clone()] {
        assert_eq!(value["previewStatus"], "broken");
        assert!(value["gaps"]
            .as_array()
            .unwrap()
            .contains(&json!("preview-broken")));
    }
    assert_eq!(meta["sourceUrl"], "https://magicui.design/r/button.json");
    assert_eq!(meta["version"], "1.2.3");
    assert_eq!(meta["sourceRef"], "release-tag");
    assert_eq!(meta["skipped"], json!(["empty.ts", "missing.ts"]));
    assert_eq!(
        meta["fileSha256"]["src/a.tsx"],
        "037ecd1db38c230c248787e60fd7bfc0cb0101b187b59535b6e7483be762d350"
    );
}

#[test]
fn beautiful_foundation_is_indexed_and_downloadable() {
    let cache = temp();
    let server = mock(|call| {
        if call.path.ends_with("/registry.json") {
            return ok(json!({"items":[
            {"name":"foundation","type":"registry:style"}, {"name":"other","type":"registry:style"}
        ]}).to_string());
        }
        ok(json!({"description":"Foundation", "files":[{"path":"foundation.css","content":":root {}"}]}).to_string())
    });
    let synced = cli(
        &["sync", "--source", "beautifului", "--json"],
        &server,
        &cache,
    );
    assert_eq!(synced.code, 0, "{}", synced.err);
    let items = read_json(cache.join("index.json"))["sources"]["beautifului"]["items"].clone();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["name"], "foundation");
    let got = cli(
        &[
            "get",
            "beautifului/foundation",
            "--out",
            temp().to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(got.code, 0, "{}", got.err);
    assert_eq!(got.json()["files"], json!(["foundation.css"]));
}

#[test]
fn dead_21st_markdown_or_install_is_unavailable_with_exit_3_and_no_output() {
    for install_only in [false, true] {
        let server = mock(move |call| {
            if !install_only || call.path.contains("/install/") {
                return Reply::Respond(404, "gone".into());
            }
            ok("# Item\n\n- License: unknown\n")
        });
        let cache = temp();
        let out = temp().join("out");
        let result = cli_env(
            &["get", "21st/a/b", "--json", "--out", out.to_str().unwrap()],
            &server,
            &cache,
            if install_only {
                vec![("API_KEY_21ST", "fixture-key")]
            } else {
                vec![]
            },
        );
        assert_eq!(result.code, 3, "{}", result.err);
        assert!(result.out.is_empty());
        assert!(result.err.contains("unavailable") && result.err.contains("404"));
        assert!(!result.err.contains("requires API_KEY_21ST"));
        assert!(!out.exists());
    }
}

#[test]
fn skill_explains_local_imports_and_retains_owner_licence_rule() {
    let skill = include_str!("../skills/clay-ui/SKILL.md");
    assert!(
        skill.contains("local-imports-missing")
            && skill.contains("import the package")
            && skill.contains("write or find the missing piece")
    );
    assert!(skill.contains("Never skip or demote the item for this."));
}

#[test]
fn local_import_gap_never_demotes_keyword_or_semantic_results() {
    let candidates = [item("a", "button"), item("b", "button")];
    let mut flagged = candidates.clone();
    flagged[0].insert("gaps".into(), json!(["local-imports-missing"]));
    let scores = similarity(&[("a/button", 0.8), ("b/button", 0.79)]);
    let summarize = |items: Vec<Object>| {
        items
            .into_iter()
            .map(|i| (i["id"].clone(), i["score"].clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        summarize(rank(&candidates, "button", no_gaps(20))),
        summarize(rank(&flagged, "button", no_gaps(20)))
    );
    assert_eq!(
        summarize(rank_semantic(&candidates, &scores, no_gaps(20), RANKING)),
        summarize(rank_semantic(&flagged, &scores, no_gaps(20), RANKING))
    );
}

#[test]
fn default_search_order_is_byte_identical_to_previous_binary_on_fixed_index() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/default-search.json")).unwrap();
    let cache = temp();
    fs::write(cache.join("index.json"), fixture["index"].to_string()).unwrap();
    let server = mock(|_| Reply::Respond(404, "no live results".into()));
    for case in fixture["cases"].as_array().unwrap() {
        let args: Vec<_> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        let result = cli(&args, &server, &cache);
        assert_eq!(result.code, 0, "{}", result.err);
        let ids: Vec<_> = result.json()["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].clone())
            .collect();
        assert_eq!(
            serde_json::to_vec(&ids).unwrap(),
            case["order"].as_str().unwrap().as_bytes(),
            "{args:?}"
        );
    }
}

#[test]
fn alias_imports_are_checked_only_against_safe_delivered_files() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|_| {
        ok(json!({"files":[
        {"path":"main.tsx","content":"import '@/components/primitives/GlideMenu'; import '@/components/ui/button'; import '@/present'; import '@/unsafe'; import '@scope/pkg';"},
        {"path":"src/present/index.tsx","content":"export const ok = 1"},
        {"path":"../unsafe.tsx","target":"../unsafe.tsx","content":"x"}
    ]}).to_string())
    });
    let shown = cli(&["show", "magicui/button", "--json"], &server, &cache);
    let got = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            temp().to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(got.code, 0, "{}", got.err);
    let expected = json!([
        {"file":"main.tsx","import":"@/components/primitives/GlideMenu"},
        {"file":"main.tsx","import":"@/components/ui/button"},
        {"file":"main.tsx","import":"@/unsafe"}
    ]);
    assert_eq!(shown.json()["missingLocalImports"], expected);
    assert_eq!(got.json()["missingLocalImports"], expected);
    assert!(got.json()["gaps"]
        .as_array()
        .unwrap()
        .contains(&json!("local-imports-missing")));
}

#[test]
fn license_has_one_markdown_source_for_search_and_show_without_demoting_unknown() {
    for (stated, license, assumed) in [
        ("mit", "MIT", false),
        ("unknown", "MIT", true),
        ("", "MIT", true),
        ("Apache-2.0", "Apache-2.0", false),
    ] {
        let cache = temp();
        fs::write(cache.join("index.json"), "{\"sources\":{}}").unwrap();
        let server = mock(move |call| {
            if call.path.contains("/s/") {
                return ok("- [Table](https://21st.dev/@originui/components/table.md)");
            }
            ok(format!("# Table\n\n> Data table\n\n- License: {stated}\n"))
        });
        let search = cli(
            &["search", "table", "--source", "21st", "--json"],
            &server,
            &cache,
        );
        let show = cli(&["show", "21st/originui/table", "--json"], &server, &cache);
        assert_eq!(search.code, 0, "{}", search.err);
        assert_eq!(show.code, 0, "{}", show.err);
        for result in [&search.json()["results"][0], &show.json()] {
            assert_eq!(result["license"], license);
            assert_eq!(result["licenseAssumed"], assumed);
        }
    }
}

#[test]
fn missing_dependencies_include_imported_package_roots_but_not_aliases_or_helpers() {
    let cache = temp();
    seed_index(&cache, "shadcn", vec![item("shadcn", "breadcrumb")]);
    let server = mock(|_| {
        ok(json!({"dependencies":["react@19","@radix-ui/react-slot@1"],"files":[
        {"path":"breadcrumb.tsx","content":"import 'lucide-react'; export { X } from '@radix-ui/react-slot'; import 'react/jsx-runtime'; import '@/components/ui/button'; const helper = require('cn'); const lazy = import('@scope/extra/subpath'); // import 'fake'"},
        {"path":"../excluded.tsx","content":"import 'unsafe-file-dep'"}
    ]}).to_string())
    });
    let project = temp();
    fs::write(
        project.join("package.json"),
        "{\"dependencies\":{\"react\":\"19\"}}",
    )
    .unwrap();
    let result = cli(
        &[
            "show",
            "shadcn/breadcrumb",
            "--project",
            project.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(result.code, 0, "{}", result.err);
    assert_eq!(
        result.json()["project"]["missingDependencies"],
        json!(["@radix-ui/react-slot@1", "lucide-react", "@scope/extra"])
    );
}

#[test]
fn available_only_excludes_known_unfetchable_items_and_refills_results() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/default-search.json")).unwrap();
    let cache = temp();
    fs::write(cache.join("index.json"), fixture["index"].to_string()).unwrap();
    let server = mock(|_| Reply::Respond(404, "none".into()));
    let result = cli(
        &[
            "search",
            "table",
            "--source",
            "shadcn",
            "--available-only",
            "--limit",
            "3",
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(result.code, 0, "{}", result.err);
    let value = result.json();
    assert_eq!(value["results"].as_array().unwrap().len(), 3);
    assert!(value["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["gaps"] == json!([])));
    // The skill's main query loses no available candidate.
    let args = [
        "search",
        "date range picker booking stays",
        "--source",
        "shadcn",
        "--limit",
        "20",
        "--json",
    ];
    let plain = cli(&args, &server, &cache).json();
    let mut available = args.to_vec();
    available.push("--available-only");
    let ids = |v: &Value| {
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&plain), ids(&cli(&available, &server, &cache).json()));
    assert_eq!(
        plain["results"],
        cli(&available, &server, &cache).json()["results"]
    );
    for detail_server in [
        mock(|_| ok("{}")),
        mock(|_| Reply::Respond(404, "gone".into())),
    ] {
        let result = cli(
            &[
                "search",
                "table",
                "--source",
                "shadcn",
                "--available-only",
                "--limit",
                "3",
                "--details",
                "3",
                "--json",
            ],
            &detail_server,
            &cache,
        );
        assert_eq!(result.code, 0, "{}", result.err);
        assert_eq!(result.json()["results"], json!([]));
    }
}

#[test]
fn cached_get_keeps_original_fetched_at_and_reports_unknown_offline_preview() {
    let cache = temp();
    seed_index(&cache, "magicui", vec![magic_item()]);
    let server = mock(|call| {
        if call.path.contains("x.dev") {
            ok("<html>working</html>")
        } else {
            ok(json!({"files":[{"path":"button.tsx","content":"export const x = 1"}]}).to_string())
        }
    });
    let out = temp();
    let args = [
        "get",
        "magicui/button",
        "--out",
        out.to_str().unwrap(),
        "--json",
        "--force",
    ];
    let cold = cli(&args, &server, &cache).json();
    let warm = cli(&args, &server, &cache).json();
    assert_eq!(cold["fromCache"], false);
    assert_eq!(warm["fromCache"], true);
    assert_eq!(cold["fetchedAt"], warm["fetchedAt"]);
    assert_eq!(cold["previewStatus"], "ok");
    let offline = cli(
        &args,
        &mock(|_| Reply::Respond(503, "offline".into())),
        &cache,
    );
    assert_eq!(offline.code, 0, "{}", offline.err);
    assert_eq!(offline.json()["previewStatus"], "unknown");
    assert_eq!(offline.json()["fetchedAt"], cold["fetchedAt"]);
    let manifest = read_json(out.join("magicui/button/clayui.json"));
    assert_eq!(manifest["fromCache"], true);
    assert_eq!(manifest["fetchedAt"], cold["fetchedAt"]);
}

#[test]
fn install_commands_are_cleaned_only_for_show_and_get_descriptions() {
    let cache = temp();
    let description = "Nice button. npx shadcn@latest add button";
    seed_index(
        &cache,
        "magicui",
        vec![item_with(
            "magicui",
            "button",
            json!({"description":description}),
        )],
    );
    let server = mock(move |_| {
        ok(json!({"description":description,"files":[{"path":"button.tsx","content":"// npm install is code evidence, do not clean"}]}).to_string())
    });
    let before = cli(
        &["search", "button", "--source", "magicui", "--json"],
        &server,
        &cache,
    );
    assert_eq!(before.json()["results"][0]["description"], description);
    let shown = cli(
        &["show", "magicui/button", "--json", "--code"],
        &server,
        &cache,
    )
    .json();
    assert_eq!(shown["description"], "Nice button.");
    assert!(shown["files"][0]["content"]
        .as_str()
        .unwrap()
        .contains("npm install"));
    let out = temp();
    let got = cli(
        &[
            "get",
            "magicui/button",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    )
    .json();
    assert_eq!(got["description"], "Nice button.");
    assert_eq!(
        read_json(out.join("magicui/button/clayui.json"))["description"],
        "Nice button."
    );
    assert_eq!(
        before.out,
        cli(
            &["search", "button", "--source", "magicui", "--json"],
            &server,
            &cache
        )
        .out
    );
}

#[test]
fn exit_2_prints_only_the_error_line() {
    let cache = temp();
    let server = mock(|_| ok(""));
    for args in [
        vec!["show"],
        vec!["search"],
        vec!["search", "x", "--bogus"],
        vec!["search", "x", "--limit", "-1"],
    ] {
        let result = cli(&args, &server, &cache);
        assert_eq!(result.code, 2);
        assert!(result.out.is_empty());
        assert_eq!(result.err.lines().count(), 1, "{}", result.err);
        assert!(!result.err.contains("Clay UI —"));
    }
}

#[test]
fn skill_and_init_cover_acceptance_guidance() {
    let skill = include_str!("../skills/clay-ui/SKILL.md");
    for text in [
        "--available-only --limit 20 --json",
        "local-imports-missing` appears only after `show` or `get`",
        "mention the limited coverage only if coverage matters",
        "no free single data table with both filtering and sorting",
        "build on TanStack",
        "write the missing part and tell the client",
    ] {
        assert!(skill.contains(text), "{text}");
    }
    let result = cli(
        &["init", "--target", "agents", "--no-sync"],
        &mock(|_| ok("")),
        &temp(),
    );
    assert_eq!(result.code, 0);
    assert!(result.out.contains("CLAYUI_GEMINI_ENV_FILE"));
}

#[test]
fn new_registry_specs_sync_show_and_get_without_installing() {
    let specs: Value =
        serde_json::from_str(include_str!("../experiments/new-sources/sources.json")).unwrap();
    for row in specs.as_array().unwrap() {
        let (id, index, item) = (
            row[0].as_str().unwrap(),
            row[2].as_str().unwrap(),
            row[3].as_str().unwrap(),
        );
        let cache = temp();
        let server = mock(|call| {
            if call.path.ends_with("/registry.json") {
                return ok(json!({"items":[
                    {"name":"sample","type":"registry:ui","title":"Sample chart", "meta":{"isPro":false,"tier":"free"}},
                    {"name":"paid","type":"registry:ui","meta":{"isPro":true,"tier":"pro"}},
                    {"name":"helper","type":"registry:hook"},
                    {"name":"skill","type":"registry:item"},
                    {"name":"sample-demo","type":"registry:component"},
                    {"name":"../escape","type":"registry:ui"}
                ]}).to_string());
            }
            ok(json!({"files":[{"path":"chart.tsx","content":"export const Chart = () => null;"}],"registryDependencies":["https://asciicn.dev/r/ascii-core.json"],"cssVars":{"theme":{"color":"red"}},"meta":{"isPro":false}}).to_string())
        });
        let synced = cli(&["sync", "--source", id, "--json"], &server, &cache);
        assert_eq!(synced.code, 0, "{id}: {}", synced.err);
        let catalog = read_json(cache.join("index.json"));
        let items = catalog["sources"][id]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{id}: {items:?}");
        assert_eq!(items[0]["license"], row[4]);
        let component = format!("{id}/sample");
        let shown = cli(&["show", &component, "--json"], &server, &cache);
        assert_eq!(shown.code, 0, "{id}: {}", shown.err);
        assert_eq!(shown.json()["cssVars"]["theme"]["color"], "red");
        assert_eq!(
            shown.json()["registryDependencies"],
            json!(["https://asciicn.dev/r/ascii-core.json"])
        );
        let out = temp();
        let got = cli(
            &["get", &component, "--out", out.to_str().unwrap(), "--json"],
            &server,
            &cache,
        );
        assert_eq!(got.code, 0, "{id}: {}", got.err);
        let manifest = read_json(out.join(id).join("sample/clayui.json"));
        assert_eq!(manifest["cssVars"]["theme"]["color"], "red");
        assert!(manifest["licenseUrl"]
            .as_str()
            .unwrap()
            .starts_with("https://raw.githubusercontent.com/"));
        assert!(out.join(id).join("sample/chart.tsx").is_file());
        let paths: Vec<_> = server
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.path.clone())
            .collect();
        assert!(
            paths.contains(&format!("/{}", index.strip_prefix("https://").unwrap())),
            "{id}: {paths:?}"
        );
        assert!(
            paths.contains(&format!(
                "/{}",
                item.replace("{name}", "sample")
                    .strip_prefix("https://")
                    .unwrap()
            )),
            "{id}: {paths:?}"
        );
    }
}

#[test]
fn reui_skips_denied_modules_and_excludes_premium_blocks() {
    let cache = temp();
    let server = mock(|call| {
        if call.path.ends_with("registry.json") {
            return ok(json!({"items":[
                {"name":"data-grid","type":"registry:ui"},
                {"name":"locked","type":"registry:ui"},
                {"name":"app-shell-35","type":"registry:block"}
            ]})
            .to_string());
        }
        if call.path.ends_with("locked.json") {
            return Reply::Respond(401, "license required".into());
        }
        ok(json!({"files":[{"path":"grid.tsx","content":"export const grid = 1;"}]}).to_string())
    });
    assert_eq!(
        cli(&["sync", "--source", "reui", "--json"], &server, &cache).code,
        0
    );
    let index = read_json(cache.join("index.json"));
    assert_eq!(
        index["sources"]["reui"]["items"].as_array().unwrap().len(),
        1
    );
    assert!(!server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|c| c.path.ends_with("app-shell-35.json")));
}

#[test]
fn shadcn_space_requires_repository_membership_and_explicit_free_flag() {
    let cache = temp();
    let server = mock(|call| {
        let names = if call.path.contains("raw.githubusercontent.com") {
            vec![json!({"name":"hero-01"}), json!({"name":"hero-02"})]
        } else {
            vec![
                json!({"name":"hero-01","type":"registry:block","meta":{"isPro":false}}),
                json!({"name":"hero-02","type":"registry:block","meta":{"isPro":true}}),
                json!({"name":"hero-03","type":"registry:block","meta":{"isPro":false}}),
                json!({"name":"unknown","type":"registry:block"}),
            ]
        };
        ok(json!({"items":names}).to_string())
    });
    let result = cli(
        &["sync", "--source", "shadcn-space", "--json"],
        &server,
        &cache,
    );
    assert_eq!(result.code, 0, "{}", result.err);
    let index = read_json(cache.join("index.json"));
    let items = index["sources"]["shadcn-space"]["items"]
        .as_array()
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "hero-01");
}

#[test]
fn new_registry_rechecks_pro_metadata_before_delivering_code() {
    let cache = temp();
    seed_index(
        &cache,
        "shadcn-space",
        vec![item_with(
            "shadcn-space",
            "hero-01",
            json!({"licenseUrl":"https://example.com/LICENSE"}),
        )],
    );
    let server = registry_mock(
        json!({"meta":{"isPro":true},"files":[{"path":"paid.tsx","content":"paid"}]}),
    );
    let out = temp();
    let got = cli(
        &[
            "get",
            "shadcn-space/hero-01",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(got.code, 3, "{}", got.err);
    assert!(entries(&out).is_empty());
}

#[test]
fn asciicn_downloads_own_engine_and_css_without_following_other_hosts() {
    let cache = temp();
    seed_index(
        &cache,
        "asciicn",
        vec![item_with(
            "asciicn",
            "miami",
            json!({"licenseUrl":"https://example.com/LICENSE"}),
        )],
    );
    let server = mock(|call| {
        let body = if call.path.ends_with("miami.json") {
            json!({"files":[{"path":"miami.tsx","content":"import { engine } from './core';"}],"dependencies":[],"registryDependencies":["https://asciicn.dev/r/core.json","https://other.invalid/steal.json"]})
        } else if call.path.ends_with("core.json") {
            json!({"files":[{"path":"core.ts","content":"export const engine = 1;"}],"dependencies":["engine-package"],"registryDependencies":["https://asciicn.dev/r/core.json","https://asciicn.dev/r/tokens.json"]})
        } else if call.path.ends_with("tokens.json") {
            json!({"cssVars":{"theme":{"accent":"red"}}})
        } else {
            return Reply::Respond(404, "unexpected".into());
        };
        ok(body.to_string())
    });
    let out = temp();
    let got = cli(
        &[
            "get",
            "asciicn/miami",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ],
        &server,
        &cache,
    );
    assert_eq!(got.code, 0, "{}", got.err);
    let manifest = got.json();
    assert_eq!(manifest["files"], json!(["miami.tsx", "core.ts"]));
    assert_eq!(manifest["dependencies"], json!(["engine-package"]));
    let configs = manifest["registryItems"].as_array().unwrap();
    assert!(configs
        .iter()
        .any(|i| i["name"] == "tokens" && i["cssVars"]["theme"]["accent"] == "red"));
    let calls = server.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.path.ends_with("core.json"))
            .count(),
        1
    );
    assert!(!calls.iter().any(|c| c.path.contains("other.invalid")));
}

#[test]
fn origin_numeric_compositions_use_upstream_tags_and_keep_their_ids() {
    let cache = temp();
    let server = registry_mock(
        json!({"items":[{"name":"comp-371","type":"registry:component","meta":{"tags":["dropdown","checkbox"]}}]}),
    );
    assert_eq!(
        cli(&["sync", "--source", "origin", "--json"], &server, &cache).code,
        0
    );
    let index = read_json(cache.join("index.json"));
    let entry = &index["sources"]["origin"]["items"][0];
    assert_eq!(entry["id"], "origin/comp-371");
    assert_eq!(entry["keywords"], json!(["dropdown", "checkbox"]));
    assert_eq!(entry["title"], "Dropdown, Checkbox (comp-371)");
    assert_eq!(entry["example"], true);
}

#[test]
fn ui_sdk_indexes_react_docs_and_downloads_only_the_component_examples() {
    let tree = json!({"tree":[
        {"path":"apps/docs/content/docs/react/components/index.mdx", "type":"blob"},
        {"path":"apps/docs/content/docs/react/components/combobox.mdx", "type":"blob"},
        {"path":"apps/docs/content/docs/react/components/time-input.mdx", "type":"blob"},
        {"path":"apps/docs/content/docs/vue/components/combobox.mdx", "type":"blob"},
        {"path":"examples/components/combobox/combobox.css", "type":"blob"},
        {"path":"examples/components/combobox/react/demo.tsx", "type":"blob"},
        {"path":"examples/components/combobox/solid/demo.tsx", "type":"blob"},
        {"path":"examples/components/listbox/react/demo.tsx", "type":"blob"},
        {"path":"examples/shared/react/log.ts", "type":"blob"},
        {"path":"examples/shared/solid/log.ts", "type":"blob"},
        {"path":"examples/styles/tokens.css", "type":"blob"}
    ]});
    let server = mock(move |call| {
        if call.path.contains("registry.npmjs.org") {
            return ok(json!({"license":"Apache-2.0"}).to_string());
        }
        if call.path.contains("/commits/HEAD") {
            return ok(json!({"sha":"c0ffee"}).to_string());
        }
        if call.path.contains("/git/trees/") {
            return ok(tree.to_string());
        }
        if call.path.contains("/combobox.mdx") {
            return ok("---\ntitle: Combobox\ndescription: Search a list of options.\n---\n");
        }
        if call.path.contains(".mdx") {
            return ok("---\ntitle: Time Input\n---\n");
        }
        if call.path.contains("ui-sdk.dev") {
            return ok("# Combobox usage\n");
        }
        if call.path.contains("raw.githubusercontent") {
            return ok("export const x = 1;\n");
        }
        Reply::Respond(404, "unexpected URL".into())
    });
    let cache = temp();
    let synced = cli(&["sync", "--source", "ui-sdk", "--json"], &server, &cache);
    assert_eq!(synced.code, 0, "{}", synced.err);
    let items = read_json(cache.join("index.json"))["sources"]["ui-sdk"]["items"].clone();
    let names: Vec<&str> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["combobox", "time-input"]);
    assert_eq!(items[0]["description"], "Search a list of options.");
    assert_eq!(items[0]["license"], "Apache-2.0");
    assert_eq!(items[0]["gaps"], json!(["beta"]));
    assert!(items[1].get("sourceCode").is_none());

    let out = temp();
    let got = cli(
        &[
            "get",
            "ui-sdk/combobox",
            "--json",
            "--out",
            out.to_str().unwrap(),
        ],
        &server,
        &cache,
    );
    assert_eq!(got.code, 0, "{}", got.err);
    let meta = read_json(out.join("ui-sdk/combobox/clayui.json"));
    let mut files: Vec<&str> = meta["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file.as_str().unwrap())
        .collect();
    files.sort_unstable();
    assert_eq!(
        files,
        vec![
            "USAGE.md",
            "source/components/combobox/combobox.css",
            "source/components/combobox/react/demo.tsx",
            "source/shared/react/log.ts",
            "source/styles/tokens.css",
        ]
    );
    assert_eq!(meta["skipped"], json!([]));

    let usage_only = cli(
        &[
            "get",
            "ui-sdk/time-input",
            "--json",
            "--out",
            out.to_str().unwrap(),
        ],
        &server,
        &cache,
    );
    assert_eq!(usage_only.code, 3, "{}", usage_only.out);
}
