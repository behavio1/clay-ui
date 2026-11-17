//! The UI libraries as data, plus reading each one into the shared candidate shape.
//! The hand-written matchers below reproduce the JS regular expressions of the Node version.

use crate::js::{self, coalesce, encode_uri_component as enc, truthy, Object};
use crate::net::{
    fetch_json, fetch_json_with, fetch_text, host_of, join, map_limit, request, Fail, Headers, Res,
};
use crate::search::Group;
use serde_json::{json, Value};
use std::collections::HashMap;

#[path = "community.rs"]
mod community;

const KOKONUT_RAW: &str = "https://raw.githubusercontent.com/kokonut-labs/kokonutui/main/public/r";
const COMPONENT_TYPES: [&str; 3] = ["registry:ui", "registry:component", "registry:block"];
const MAX_SOURCE_FILES: usize = 120;
const CONCURRENCY: usize = 8;
const ACETERNITY_OWNER: &str = "manuarora700";

fn safe_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn word_like(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn run_of_word_dots(text: &str) -> &str {
    let end = text
        .find(|c: char| !(word_like(c) || c == '.' || c == '-'))
        .unwrap_or(text.len());
    &text[..end]
}

/// `/^key:\s*(.+)$/m`, trimmed and unquoted; empty when absent. `\s*` may cross lines, as in JS.
fn frontmatter(text: &str, key: &str) -> String {
    let prefix = format!("{key}:");
    for start in js::line_starts(text) {
        let Some(after) = text[start..].strip_prefix(prefix.as_str()) else {
            continue;
        };
        let rest = after.trim_start_matches(js::is_space);
        let line = rest.split(js::is_line_end).next().unwrap_or("");
        return crate::embeddings::strip_quotes(js::trim(line)).to_string();
    }
    String::new()
}

fn github_headers() -> Headers {
    match std::env::var("GITHUB_TOKEN") {
        Ok(token) if !token.is_empty() => vec![("Authorization", format!("Bearer {token}"))],
        _ => Vec::new(),
    }
}

fn tree_paths(body: &Value) -> Vec<String> {
    body.get("tree")
        .and_then(Value::as_array)
        .map_or(Vec::new(), |nodes| {
            nodes
                .iter()
                .filter_map(|node| node.get("path")?.as_str().map(str::to_string))
                .collect()
        })
}

fn repo_tree(repo: &str) -> Res<Vec<String>> {
    Ok(tree_paths(&fetch_json_with(
        &format!("https://api.github.com/repos/{repo}/git/trees/HEAD?recursive=1"),
        &github_headers(),
    )?))
}

fn encode_path(path: &str) -> String {
    path.split('/').map(enc).collect::<Vec<_>>().join("/")
}

pub fn title_from_slug(slug: &str) -> String {
    let mut out = String::with_capacity(slug.len());
    let mut previous_word = false;
    for c in slug.chars().map(|c| if c == '-' { ' ' } else { c }) {
        out.push(if !previous_word && word_like(c) {
            c.to_ascii_uppercase()
        } else {
            c
        });
        previous_word = word_like(c);
    }
    out
}

fn strings(items: &[&str]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|text| Value::String(text.to_string()))
            .collect(),
    )
}

fn candidate(source: &Source, fields: Vec<(&str, Value)>) -> Object {
    let name = fields
        .iter()
        .find(|(key, _)| *key == "name")
        .map_or(String::new(), |(_, value)| js::string_of(value));
    let mut item = Object::new();
    item.insert("id".into(), Value::String(format!("{}/{name}", source.id)));
    item.insert("source".into(), json!(source.id));
    item.insert("title".into(), Value::String(title_from_slug(&name)));
    item.insert("description".into(), json!(""));
    item.insert("previewUrl".into(), Value::Null);
    item.insert(
        "license".into(),
        source.license.map_or(Value::Null, |license| json!(license)),
    );
    item.insert("dependencies".into(), json!([]));
    item.insert("registryDependencies".into(), json!([]));
    item.insert("gaps".into(), json!([]));
    for (key, value) in fields {
        item.insert(key.to_string(), value);
    }
    item
}

fn gaps_of(item: &Object) -> Vec<Value> {
    item.get("gaps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn add_unique(list: &mut Vec<Value>, gap: &str) {
    if !list.iter().any(|existing| existing.as_str() == Some(gap)) {
        list.push(json!(gap));
    }
}

fn with_gaps(mut item: Object, extra: &[&str]) -> Object {
    let mut gaps: Vec<Value> = Vec::new();
    for gap in gaps_of(&item) {
        if !gaps.contains(&gap) {
            gaps.push(gap);
        }
    }
    for gap in extra {
        add_unique(&mut gaps, gap);
    }
    if !truthy(item.get("license")) {
        add_unique(&mut gaps, "no-license");
    }
    if !truthy(item.get("previewUrl")) {
        add_unique(&mut gaps, "no-preview");
    }
    if item
        .get("previewUrl")
        .and_then(Value::as_str)
        .is_some_and(|url| url.contains('#'))
    {
        add_unique(&mut gaps, "preview-unverified");
    }
    item.insert("gaps".into(), Value::Array(gaps));
    item
}

fn not_null(value: Value) -> Res<Value> {
    if value.is_null() {
        Err(Fail::other("Cannot read properties of null"))
    } else {
        Ok(value)
    }
}

fn pick_files(entry: &Value) -> Res<Value> {
    let files = match entry.get("files") {
        None | Some(Value::Null) => return Ok(json!([])),
        Some(Value::Array(files)) => files,
        Some(_) => return Err(Fail::other("files is not a list")),
    };
    let mut picked = Vec::with_capacity(files.len());
    for file in files {
        if file.is_null() {
            return Err(Fail::other("Cannot read properties of null"));
        }
        let mut out = Object::new();
        for key in ["path", "target", "content"] {
            if let Some(value) = file.get(key) {
                out.insert(key.into(), value.clone());
            }
        }
        picked.push(Value::Object(out));
    }
    Ok(Value::Array(picked))
}

// --- shadcn-compatible registries -----------------------------------------

fn gap_for_status(status: u16) -> &'static str {
    if status == 404 {
        "no-source-listing"
    } else {
        "pro-only"
    }
}

fn code_gap(url: &str) -> Vec<&'static str> {
    match request(url, &Vec::new()) {
        Ok(reply) if matches!(reply.status, 401 | 403 | 404) => vec![gap_for_status(reply.status)],
        _ => Vec::new(),
    }
}

pub struct RegistrySpec {
    index_url: &'static str,
    item_url: fn(&str) -> String,
    previews: Previews,
    enrich: Enrich,
    probe: bool,
}

#[derive(Clone, Copy)]
enum Previews {
    Shadcn,
    Magic,
    Beautiful,
    Kokonut,
    Aceternity,
}

#[derive(Clone, Copy, PartialEq)]
enum Enrich {
    None,
    Shadcn,
    Beautiful,
    Aceternity,
}

#[derive(Default, Clone)]
struct Extra {
    title: Option<Value>,
    description: Option<Value>,
    keywords: Option<Value>,
}

#[derive(Clone, Default)]
struct CatalogEntry {
    extra: Extra,
    documentation_url: Option<Value>,
}

type Catalog = HashMap<String, CatalogEntry>;
type PreviewMap = HashMap<String, Value>;

/// Matches `content/docs/<a>/<b>.mdx`.
fn docs_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("content/docs/")?;
    let (first, file) = rest.split_once('/')?;
    let second = file.strip_suffix(".mdx")?;
    (!first.is_empty() && !second.is_empty() && !second.contains('/')).then_some((first, second))
}

fn kokonut_previews() -> Res<PreviewMap> {
    let tree = fetch_json_with(
        "https://api.github.com/repos/kokonut-labs/kokonutui/git/trees/main?recursive=1",
        &github_headers(),
    )?;
    let mut map = PreviewMap::new();
    for path in tree_paths(&tree) {
        if let Some((category, slug)) = docs_path(&path) {
            map.insert(
                slug.to_string(),
                json!(format!("https://kokonutui.com/docs/{category}/{slug}")),
            );
        }
    }
    Ok(map)
}

fn load_aceternity_catalog() -> Res<Catalog> {
    let body = fetch_json("https://ui.aceternity.com/api/components")?;
    let mut categories: HashMap<String, Vec<Value>> = HashMap::new();
    if let Some(Value::Object(groups)) = body.get("categories") {
        for (category, names) in groups {
            let names = names
                .as_array()
                .ok_or_else(|| Fail::other("names is not a list"))?;
            for name in names {
                categories
                    .entry(js::string_of(name))
                    .or_default()
                    .push(json!(category));
            }
        }
    }
    let mut catalog = Catalog::new();
    for field in ["components", "blocks"] {
        let Some(Value::Array(entries)) = body.get(field) else {
            continue;
        };
        for entry in entries {
            let name = entry.get("name").map_or(String::new(), js::string_of);
            let keywords = Value::Array(categories.get(&name).cloned().unwrap_or_default());
            let extra = Extra {
                title: entry.get("title").filter(|v| !v.is_null()).cloned(),
                description: Some(coalesce(entry.get("description"), json!(""))),
                keywords: Some(keywords),
            };
            catalog.insert(
                name,
                CatalogEntry {
                    extra,
                    documentation_url: entry.get("documentationUrl").cloned(),
                },
            );
        }
    }
    Ok(catalog)
}

fn nested_str<'a>(item: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = item;
    for key in path {
        current = current.get(key)?;
    }
    (!current.is_null()).then_some(current)
}

fn preview_for(spec: &RegistrySpec, item: &Value, name: &str, map: &PreviewMap) -> Value {
    match spec.previews {
        Previews::Kokonut | Previews::Aceternity => map.get(name).cloned().unwrap_or(Value::Null),
        Previews::Shadcn => nested_str(item, &["meta", "links", "radix", "docs"])
            .or(nested_str(item, &["meta", "links", "base", "docs"]))
            .cloned()
            .unwrap_or(Value::Null),
        Previews::Magic => json!(format!("https://magicui.design/docs/components/{name}")),
        Previews::Beautiful => {
            if item.get("type").and_then(Value::as_str) == Some("registry:component") {
                json!(format!("https://www.beautifului.dev/#{name}"))
            } else {
                Value::Null
            }
        }
    }
}

fn enrich(spec: &RegistrySpec, name: &str, preview: &Value, catalog: &Catalog) -> Extra {
    let result: Res<Extra> = (|| {
        Ok(match spec.enrich {
            Enrich::None => Extra::default(),
            Enrich::Aceternity => catalog
                .get(name)
                .map(|entry| entry.extra.clone())
                .unwrap_or_default(),
            Enrich::Shadcn if truthy(Some(preview)) => {
                let text = fetch_text(&format!("{}.md", js::string_of(preview)))?;
                Extra {
                    description: Some(json!(frontmatter(&text, "description"))),
                    ..Extra::default()
                }
            }
            Enrich::Shadcn => Extra::default(),
            Enrich::Beautiful => {
                let body = not_null(fetch_json(&format!(
                    "https://www.beautifului.dev/r/{}.json",
                    enc(name)
                ))?)?;
                Extra {
                    description: body.get("description").filter(|v| !v.is_null()).cloned(),
                    ..Extra::default()
                }
            }
        })
    })();
    result.unwrap_or_default()
}

fn load_registry(source: &Source, spec: &RegistrySpec) -> Res<Vec<Object>> {
    let (body, previews) = join(
        || fetch_json(spec.index_url),
        || -> Res<(PreviewMap, Catalog)> {
            match spec.previews {
                Previews::Kokonut => Ok((kokonut_previews()?, Catalog::new())),
                Previews::Aceternity => {
                    let catalog = load_aceternity_catalog().unwrap_or_default();
                    let map = catalog
                        .iter()
                        .filter(|(_, entry)| truthy(entry.documentation_url.as_ref()))
                        .filter_map(|(name, entry)| {
                            Some((name.clone(), entry.documentation_url.clone()?))
                        })
                        .collect();
                    Ok((map, catalog))
                }
                _ => Ok((PreviewMap::new(), Catalog::new())),
            }
        },
    );
    let body = not_null(body?)?;
    let (map, catalog) = previews?;
    let items = if body.is_array() {
        Some(&body)
    } else {
        body.get("items")
    };
    let Some(Value::Array(items)) = items else {
        return Err(Fail::source(format!(
            "{}: unexpected index shape",
            source.id
        )));
    };
    if items.iter().any(Value::is_null) {
        return Err(Fail::other(
            "Cannot read properties of null (reading 'type')",
        ));
    }
    let components: Vec<&Value> = items
        .iter()
        .filter(|item| {
            item.get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| {
                    COMPONENT_TYPES.contains(&kind)
                        || (source.id == "beautifului"
                            && kind == "registry:style"
                            && item.get("name").and_then(Value::as_str) == Some("foundation"))
                })
                && item
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(safe_name)
        })
        .collect();
    map_limit(&components, CONCURRENCY, |item| {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        let preview = preview_for(spec, item, name, &map);
        let extra = enrich(spec, name, &preview, &catalog);
        let gaps = if spec.probe {
            code_gap(&(spec.item_url)(name))
        } else {
            Vec::new()
        };
        Ok(with_gaps(
            candidate(
                source,
                vec![
                    ("name", json!(name)),
                    (
                        "title",
                        extra
                            .title
                            .or(item.get("title").filter(|v| !v.is_null()).cloned())
                            .unwrap_or_else(|| json!(title_from_slug(name))),
                    ),
                    (
                        "description",
                        extra
                            .description
                            .or(item.get("description").filter(|v| !v.is_null()).cloned())
                            .unwrap_or_else(|| json!("")),
                    ),
                    ("keywords", extra.keywords.unwrap_or_else(|| json!([]))),
                    ("previewUrl", preview),
                    (
                        "dependencies",
                        coalesce(item.get("dependencies"), json!([])),
                    ),
                    (
                        "registryDependencies",
                        coalesce(item.get("registryDependencies"), json!([])),
                    ),
                ],
            ),
            &gaps,
        ))
    })
}

fn registry_detail(spec: &RegistrySpec, item: &Object) -> Res<Object> {
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
    let source_url = (spec.item_url)(name);
    registry_detail_url(&source_url, item)
}

fn registry_detail_url(source_url: &str, item: &Object) -> Res<Object> {
    let entry = match fetch_json(source_url) {
        Ok(entry) => not_null(entry)?,
        Err(Fail {
            status: Some(status),
            ..
        }) if matches!(status, 401 | 403 | 404) => {
            let mut detail = item.clone();
            detail.insert("files".into(), json!([]));
            return Ok(with_gaps(detail, &[gap_for_status(status)]));
        }
        Err(error) => return Err(error),
    };
    let mut detail = item.clone();
    if item.contains_key("licenseUrl") && !community::public(&entry) {
        detail.insert("files".into(), json!([]));
        return Ok(with_gaps(detail, &["pro-only"]));
    }
    detail.insert("sourceUrl".into(), json!(source_url));
    copy_revision(&entry, &mut detail);
    if source_url.starts_with(KOKONUT_RAW) {
        detail.insert("sourceRef".into(), json!("main"));
    }
    let description = entry
        .get("description")
        .filter(|v| !v.is_null())
        .or(item.get("description"))
        .cloned();
    if let Some(description) = description {
        detail.insert("description".into(), description);
    }
    detail.insert(
        "dependencies".into(),
        coalesce(entry.get("dependencies"), json!([])),
    );
    detail.insert(
        "registryDependencies".into(),
        coalesce(entry.get("registryDependencies"), json!([])),
    );
    let env_vars: Vec<Value> = entry
        .get("envVars")
        .and_then(Value::as_object)
        .map_or(Vec::new(), |vars| {
            vars.keys().map(|key| json!(key)).collect()
        });
    detail.insert("envVars".into(), Value::Array(env_vars));
    for key in ["css", "cssVars", "tailwind"] {
        if let Some(value) = entry.get(key) {
            detail.insert(key.into(), value.clone());
        }
    }
    detail.insert("files".into(), pick_files(&entry)?);
    Ok(detail)
}

// --- npm packages ---------------------------------------------------------

fn npm_license(package: &str) -> Res<Value> {
    let meta = not_null(fetch_json(&format!(
        "https://registry.npmjs.org/{}/latest",
        package.replacen('/', "%2F", 1)
    ))?)?;
    Ok(meta
        .get("license")
        .filter(|value| value.is_string())
        .cloned()
        .unwrap_or(Value::Null))
}

fn is_source_file(file: &str) -> bool {
    file.ends_with(".ts") || file.ends_with(".tsx") || file.ends_with(".css")
}

fn is_excluded_file(file: &str) -> bool {
    [
        ".test.",
        ".spec.",
        ".stories.",
        "__tests__",
        "/test/",
        "/tests/",
        "/stories/",
    ]
    .iter()
    .any(|part| file.contains(part))
}

// Keep the tree revision tied to the downloaded bytes; never claim an npm version for repo HEAD.
fn copy_revision(entry: &Value, detail: &mut Object) {
    for (upstream, field) in [("version", "version"), ("ref", "sourceRef")] {
        if let Some(value) = entry.get(upstream).filter(|v| v.is_string()) {
            detail.insert(field.into(), value.clone());
        }
    }
}

#[derive(Default)]
struct RepoFiles {
    files: Vec<Value>,
    skipped: Vec<Value>,
    source_url: String,
    source_ref: String,
}

fn repo_source_files(source_code: &Value) -> Res<RepoFiles> {
    let text_of = |key: &str| {
        source_code
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| Fail::other("source location missing"))
    };
    let (repo, dir) = (text_of("repo")?, text_of("dir")?);
    // Optional sub-paths of `dir`; files outside them are neither fetched nor listed as skipped.
    let include: Vec<&str> = source_code
        .get("include")
        .and_then(Value::as_array)
        .map_or(Vec::new(), |parts| {
            parts.iter().filter_map(Value::as_str).collect()
        });
    let commit = fetch_json_with(
        &format!("https://api.github.com/repos/{repo}/commits/HEAD"),
        &github_headers(),
    )?;
    let source_ref = commit
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or("HEAD")
        .to_string();
    let tree = fetch_json_with(
        &format!("https://api.github.com/repos/{repo}/git/trees/{source_ref}?recursive=1"),
        &github_headers(),
    )?;
    if tree.get("truncated").and_then(Value::as_bool) == Some(true) {
        return Err(Fail::source("GitHub source tree is truncated"));
    }
    let prefix = format!("{dir}/");
    let mut paths = Vec::new();
    let mut skipped = Vec::new();
    for node in tree
        .get("tree")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(file) = node.get("path").and_then(Value::as_str) else {
            continue;
        };
        if !file.starts_with(&prefix) || node.get("type").and_then(Value::as_str) == Some("tree") {
            continue;
        }
        let relative = &file[prefix.len()..];
        if !include.is_empty() && !include.iter().any(|part| relative.starts_with(part)) {
            continue;
        }
        if is_source_file(file) && !is_excluded_file(file) && paths.len() < MAX_SOURCE_FILES {
            paths.push(file);
        } else {
            skipped.push(json!(format!("source/{}", &file[prefix.len()..])));
        }
    }
    let files = map_limit(&paths, CONCURRENCY, |file| {
        let content = fetch_text(&format!(
            "https://raw.githubusercontent.com/{repo}/{source_ref}/{}",
            encode_path(file)
        ))?;
        Ok(json!({ "path": format!("source/{}", &file[prefix.len()..]), "content": content }))
    })?;
    Ok(RepoFiles {
        files,
        skipped,
        source_url: format!("https://github.com/{repo}/tree/{source_ref}/{dir}"),
        source_ref,
    })
}

fn package_detail(item: &Object) -> Res<Object> {
    let usage_url = item.get("usageUrl").map_or(String::new(), js::string_of);
    let source_code = item.get("sourceCode").cloned().unwrap_or(Value::Null);
    let (usage, files) = join(
        || fetch_text(&usage_url),
        || repo_source_files(&source_code),
    );
    let warnings: Vec<Value> = [usage.as_ref().err(), files.as_ref().err()]
        .into_iter()
        .flatten()
        .map(|error| json!(error.message))
        .collect();
    let usage = usage.ok();
    let files = files.unwrap_or_default();
    let mut detail = item.clone();
    let has_usage = usage.as_deref().is_some_and(|text| !text.is_empty());
    if let Some(usage) = usage {
        detail.insert("usage".into(), Value::String(usage));
    }
    let has_files = !files.files.is_empty();
    if !files.source_url.is_empty() {
        detail.insert("sourceUrl".into(), json!(files.source_url));
        detail.insert("sourceRef".into(), json!(files.source_ref));
    }
    detail.insert("skipped".into(), Value::Array(files.skipped));
    detail.insert("files".into(), Value::Array(files.files));
    detail.insert("sourceWarnings".into(), Value::Array(warnings));
    if has_files {
        return Ok(detail);
    }
    Ok(with_gaps(
        detail,
        &[if has_usage {
            "usage-only"
        } else {
            "no-source-listing"
        }],
    ))
}

pub struct SinglePackage {
    package: &'static str,
    docs: Option<&'static str>,
    description: &'static str,
    keywords: &'static [&'static str],
    curated: bool,
    unmaintained: bool,
    repo: &'static str,
    dir: &'static str,
}

fn load_single(source: &Source, spec: &SinglePackage) -> Res<Vec<Object>> {
    let license = npm_license(spec.package)?;
    let mut fields = vec![
        ("name", json!(source.id)),
        ("title", json!(source.title)),
        ("description", json!(spec.description)),
        ("keywords", strings(spec.keywords)),
        ("license", license),
    ];
    if spec.curated {
        fields.push(("curated", json!("pick-ui-library")));
    }
    fields.push(("sourceCode", json!({ "repo": spec.repo, "dir": spec.dir })));
    fields.push((
        "previewUrl",
        spec.docs.map_or(Value::Null, |docs| json!(docs)),
    ));
    fields.push(("dependencies", json!([spec.package])));
    fields.push((
        "usageUrl",
        json!(format!("https://unpkg.com/{}/README.md", spec.package)),
    ));
    let extra: &[&str] = if spec.unmaintained {
        &["unmaintained"]
    } else {
        &[]
    };
    Ok(vec![with_gaps(candidate(source, fields), extra)])
}

// --- Radix and Base UI ----------------------------------------------------

fn radix_name(path: &str) -> Option<&str> {
    let name = path
        .strip_prefix("data/primitives/docs/components/")?
        .strip_suffix(".mdx")?;
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
    .then_some(name)
}

fn load_radix(source: &Source) -> Res<Vec<Object>> {
    let (tree, license) = join(|| repo_tree("radix-ui/website"), || npm_license("radix-ui"));
    let (tree, license) = (tree?, license?);
    let files: Vec<&String> = tree
        .iter()
        .filter(|file| radix_name(file).is_some())
        .collect();
    if files.is_empty() {
        return Err(Fail::source("radix: no component docs in repo tree"));
    }
    map_limit(&files, CONCURRENCY, |file| {
        let name = radix_name(file).unwrap_or("");
        let usage_url = format!("https://raw.githubusercontent.com/radix-ui/website/main/{file}");
        let description = frontmatter(&fetch_text(&usage_url)?, "metaDescription");
        Ok(with_gaps(
            candidate(
                source,
                vec![
                    ("name", json!(name)),
                    ("license", license.clone()),
                    ("usageUrl", json!(usage_url)),
                    (
                        "sourceCode",
                        json!({ "repo": "radix-ui/primitives", "dir": format!("packages/react/{name}/src") }),
                    ),
                    ("description", json!(description)),
                    ("keywords", json!(["primitive", "unstyled"])),
                    (
                        "previewUrl",
                        json!(format!(
                            "https://www.radix-ui.com/primitives/docs/components/{name}"
                        )),
                    ),
                    ("dependencies", json!(["radix-ui"])),
                ],
            ),
            &[],
        ))
    })
}

const BASE_UI_CURATED: [&str; 4] = ["dialog", "menu", "popover", "select"];

/// The text between the first two `^## Components$` lines, cut at the next `^## ` line.
fn base_ui_section(text: &str) -> Option<&str> {
    let starts = js::line_starts(text);
    let line_at = |start: usize| text[start..].split(js::is_line_end).next().unwrap_or("");
    let heading = starts
        .iter()
        .copied()
        .find(|start| line_at(*start) == "## Components")?;
    let begin = heading + "## Components".len();
    let end = starts
        .iter()
        .copied()
        .find(|start| *start >= begin && line_at(*start).starts_with("## "))
        .unwrap_or(text.len());
    Some(&text[begin..end])
}

struct BaseUiRow<'a> {
    title: &'a str,
    name: &'a str,
    url: String,
    description: &'a str,
    end: usize,
}

fn base_ui_row(section: &str, start: usize) -> Option<BaseUiRow<'_>> {
    const LINK: &str = "](https://base-ui.com/react/components/";
    let rest = section[start..].strip_prefix("- [")?;
    let close = rest.find(']')?;
    if close == 0 {
        return None;
    }
    let title = &rest[..close];
    let after = rest[close..].strip_prefix(LINK)?;
    let name_len = after
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
        .unwrap_or(after.len());
    if name_len == 0 {
        return None;
    }
    let name = &after[..name_len];
    let tail = after[name_len..].strip_prefix(".md):")?;
    let description = tail.trim_start_matches(js::is_space);
    let line = description.split(js::is_line_end).next().unwrap_or("");
    let end = section.len() - description.len() + line.len();
    Some(BaseUiRow {
        title,
        name,
        url: format!("https://base-ui.com/react/components/{name}"),
        description: line,
        end,
    })
}

fn load_base_ui(source: &Source) -> Res<Vec<Object>> {
    let (text, license) = join(
        || fetch_text("https://base-ui.com/llms.txt"),
        || npm_license("@base-ui/react"),
    );
    let (text, license) = (text?, license?);
    let section = base_ui_section(&text)
        .ok_or_else(|| Fail::source("base-ui: components section missing"))?;
    let mut items = Vec::new();
    let mut position = 0;
    for start in js::line_starts(section) {
        if start < position {
            continue;
        }
        let Some(row) = base_ui_row(section, start) else {
            continue;
        };
        position = row.end;
        let mut fields = vec![
            ("name", json!(row.name)),
            ("title", json!(row.title)),
            ("description", json!(row.description)),
            ("license", license.clone()),
            ("previewUrl", json!(row.url)),
            ("usageUrl", json!(format!("{}.md", row.url))),
            (
                "sourceCode",
                json!({ "repo": "mui/base-ui", "dir": format!("packages/react/src/{}", row.name) }),
            ),
            ("keywords", json!(["primitive", "unstyled"])),
        ];
        if BASE_UI_CURATED.contains(&row.name) {
            fields.push(("curated", json!("pick-ui-library")));
        }
        fields.push(("dependencies", json!(["@base-ui/react"])));
        items.push(with_gaps(candidate(source, fields), &[]));
    }
    Ok(items)
}

// --- UI SDK (Vercel Labs) -------------------------------------------------

const UI_SDK_REPO: &str = "vercel-labs/ui";

fn ui_sdk_name(path: &str) -> Option<&str> {
    let name = path
        .strip_prefix("apps/docs/content/docs/react/components/")?
        .strip_suffix(".mdx")?;
    (!name.is_empty()
        && name != "index"
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
    .then_some(name)
}

/// The React examples of one component, with their CSS and the shared example styles.
fn ui_sdk_source(name: &str) -> Value {
    json!({
        "repo": UI_SDK_REPO,
        "dir": "examples",
        "include": [
            format!("components/{name}/react/"),
            format!("components/{name}/{name}.css"),
            "shared/react/",
            "styles/",
        ],
    })
}

fn load_ui_sdk(source: &Source) -> Res<Vec<Object>> {
    let (tree, license) = join(|| repo_tree(UI_SDK_REPO), || npm_license("@ui-sdk/react"));
    let (tree, license) = (tree?, license?);
    let files: Vec<&String> = tree
        .iter()
        .filter(|file| ui_sdk_name(file).is_some())
        .collect();
    if files.is_empty() {
        return Err(Fail::source("ui-sdk: no component docs in repo tree"));
    }
    map_limit(&files, CONCURRENCY, |file| {
        let name = ui_sdk_name(file).unwrap_or("");
        let text = fetch_text(&format!(
            "https://raw.githubusercontent.com/{UI_SDK_REPO}/main/{}",
            encode_path(file)
        ))?;
        let title = Some(frontmatter(&text, "title"))
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| title_from_slug(name));
        let page = format!("https://ui-sdk.dev/docs/react/components/{name}");
        let mut fields = vec![
            ("name", json!(name)),
            ("title", json!(title)),
            ("description", json!(frontmatter(&text, "description"))),
            ("license", license.clone()),
            ("usageUrl", json!(format!("{page}.md"))),
        ];
        // Without React examples there is nothing to copy beyond the docs page.
        let examples = format!("examples/components/{name}/react/");
        if tree.iter().any(|path| path.starts_with(&examples)) {
            fields.push(("sourceCode", ui_sdk_source(name)));
        }
        fields.extend([
            ("keywords", json!(["primitive", "unstyled", "headless"])),
            ("previewUrl", json!(page)),
            ("dependencies", json!(["@ui-sdk/react", "@ui-sdk/core"])),
        ]);
        Ok(with_gaps(candidate(source, fields), &["beta"]))
    })
}

// --- RareUI ---------------------------------------------------------------

fn squash(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .collect()
}

fn split_camel(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    let mut previous = '\0';
    for c in text.chars() {
        if previous.is_ascii_lowercase() && c.is_ascii_uppercase() {
            out.push('-');
        }
        out.push(c);
        previous = c;
    }
    out
}

struct RareDoc {
    category: String,
    slug: String,
    file: String,
}

fn load_rareui(source: &Source) -> Res<Vec<Object>> {
    let (index, tree) = join(
        || fetch_json("https://shitui.in/r/index.json"),
        || repo_tree("Codewithswappy/ShitUI"),
    );
    let index = not_null(index?)?;
    let tree = tree?;
    let Some(Value::Array(components)) = index.get("components") else {
        return Err(Fail::source("rareui: unexpected index shape"));
    };
    let mut docs = HashMap::new();
    for file in tree {
        if let Some((category, slug)) = docs_path(&file) {
            docs.insert(
                squash(slug),
                RareDoc {
                    category: category.to_string(),
                    slug: slug.to_string(),
                    file: file.clone(),
                },
            );
        }
    }
    let names: Vec<&str> = components
        .iter()
        .filter_map(Value::as_str)
        .filter(|name| safe_name(name))
        .collect();
    map_limit(&names, CONCURRENCY, |name| {
        let doc = docs.get(&squash(name));
        let text = doc.map_or(String::new(), |doc| {
            fetch_text(&format!(
                "https://raw.githubusercontent.com/Codewithswappy/ShitUI/main/{}",
                encode_path(&doc.file)
            ))
            .unwrap_or_default()
        });
        let title = Some(frontmatter(&text, "title"))
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| title_from_slug(&split_camel(name)));
        Ok(with_gaps(
            candidate(
                source,
                vec![
                    ("name", json!(name)),
                    ("title", json!(title)),
                    ("description", json!(frontmatter(&text, "description"))),
                    (
                        "keywords",
                        doc.map_or(json!([]), |doc| json!([doc.category])),
                    ),
                    (
                        "previewUrl",
                        doc.map_or(Value::Null, |doc| {
                            json!(format!(
                                "https://shitui.in/docs/{}/{}",
                                doc.category, doc.slug
                            ))
                        }),
                    ),
                ],
            ),
            &[],
        ))
    })
}

fn rareui_detail(item: &Object) -> Res<Object> {
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
    let source_url = format!("https://shitui.in/r/{}.json", enc(name));
    let entry = not_null(fetch_json(&source_url)?)?;
    let mut detail = item.clone();
    detail.insert("sourceUrl".into(), json!(source_url));
    copy_revision(&entry, &mut detail);
    detail.insert(
        "dependencies".into(),
        coalesce(entry.get("dependencies"), json!([])),
    );
    detail.insert(
        "registryDependencies".into(),
        coalesce(entry.get("registryDependencies"), json!([])),
    );
    detail.insert("files".into(), pick_files(&entry)?);
    Ok(detail)
}

// --- 21st.dev (live; the catalog is too large to index) -------------------

fn key_21st() -> Option<String> {
    std::env::var("API_KEY_21ST")
        .ok()
        .filter(|key| !key.is_empty())
}

fn bearer(key: &str) -> Headers {
    vec![("Authorization", format!("Bearer {key}"))]
}

fn files_21st(user: &str, slug: &str) -> Res<Value> {
    let Some(key) = key_21st() else {
        return Ok(json!([]));
    };
    let body = not_null(fetch_json_with(
        &format!(
            "https://21st.dev/api/v1/components/install/{}/{}",
            enc(user),
            enc(slug)
        ),
        &bearer(&key),
    )?)?;
    pick_files(&body)
}

/// `/^<prefix>(.+)$/m`: the capture of the first line that starts with the prefix and has text after it.
fn line_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    js::lines(text).find_map(|line| line.strip_prefix(prefix).filter(|rest| !rest.is_empty()))
}

/// Splits like `text.split(/,\s*/)`.
fn split_commas(text: &str) -> Vec<Value> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find(',') {
        parts.push(json!(&rest[..index]));
        rest = rest[index + 1..].trim_start_matches(js::is_space);
    }
    parts.push(json!(rest));
    parts
}

fn unavailable_21st(item: &Object, source_url: &str) -> Object {
    let mut detail = item.clone();
    detail.insert("files".into(), json!([]));
    detail.insert("sourceUrl".into(), json!(source_url));
    detail.insert(
        "gaps".into(),
        Value::Array(
            gaps_of(item)
                .into_iter()
                .filter(|g| g.as_str() != Some("needs-key"))
                .collect(),
        ),
    );
    detail.insert(
        "sourceWarnings".into(),
        json!(["Upstream component is unavailable (404)"]),
    );
    with_gaps(detail, &["no-source-listing"])
}

fn license_label(stated: &str) -> &str {
    if stated.eq_ignore_ascii_case("mit") {
        "MIT"
    } else {
        stated
    }
}

pub fn normalize_license(mut item: Object) -> Object {
    if let Some(license) = item.get("license").and_then(Value::as_str) {
        item.insert("license".into(), json!(license_label(license)));
    }
    item
}

fn license_21st(text: &str) -> (String, bool) {
    let stated = line_after(text, "- License: ")
        .map(js::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("unknown"));
    (
        stated.map_or_else(|| "MIT".into(), |s| license_label(s).into()),
        stated.is_none(),
    )
}

pub fn search_license(item: &Object) -> Object {
    let mut out = item.clone();
    let is_21st = item.get("source").and_then(Value::as_str) == Some("21st")
        || item.get("mirror").and_then(Value::as_bool) == Some(true);
    if is_21st {
        if let Some(url) = item.get("previewUrl").and_then(Value::as_str) {
            if let Ok(text) = fetch_text(&format!("{}.md", url.trim_end_matches('/'))) {
                let (license, assumed) = license_21st(&text);
                out.insert("license".into(), json!(license));
                out.insert("licenseAssumed".into(), json!(assumed));
            }
        }
    }
    out
}

fn detail_21st(item: &Object, user: &str, slug: &str) -> Res<Object> {
    let source_url = format!(
        "https://21st.dev/@{}/components/{}.md",
        enc(user),
        enc(slug)
    );
    let text = match fetch_text(&source_url) {
        Ok(text) => text,
        Err(error) if error.status == Some(404) => return Ok(unavailable_21st(item, &source_url)),
        Err(error) => return Err(error),
    };
    let field = |label: &str| line_after(&text, &format!("- {label}: ")).map(js::trim);
    let (license, assumed) = license_21st(&text);
    let mut detail = item.clone();
    if let Some(title) = line_after(&text, "# ") {
        detail.insert("title".into(), json!(title));
    }
    if let Some(description) = line_after(&text, "> ") {
        detail.insert("description".into(), json!(description));
    }
    detail.insert("license".into(), json!(license));
    detail.insert("licenseAssumed".into(), json!(assumed));
    detail.insert(
        "dependencies".into(),
        Value::Array(field("npm dependencies").map_or(Vec::new(), split_commas)),
    );
    let files = match files_21st(user, slug) {
        Ok(files) => files,
        Err(error) if error.status == Some(404) => return Ok(unavailable_21st(item, &source_url)),
        Err(error) => return Err(error),
    };
    detail.insert("sourceUrl".into(), json!(source_url));
    detail.insert("files".into(), files);
    let gaps: Vec<Value> = gaps_of(item)
        .into_iter()
        .filter(|gap| gap.as_str() != Some("no-license"))
        .collect();
    detail.insert("gaps".into(), Value::Array(gaps));
    Ok(with_gaps(detail, &[]))
}

fn to_candidate_21st(
    source: &Source,
    user: &str,
    slug: &str,
    title: Value,
    description: Value,
    keywords: Value,
    has_key: bool,
) -> Object {
    let extra: &[&str] = if has_key { &[] } else { &["needs-key"] };
    with_gaps(
        candidate(
            source,
            vec![
                ("license", json!("MIT")),
                ("licenseAssumed", json!(true)),
                ("name", json!(format!("{user}/{slug}"))),
                ("title", title),
                ("description", description),
                ("keywords", keywords),
                (
                    "previewUrl",
                    json!(format!("https://21st.dev/@{user}/components/{slug}")),
                ),
            ],
        ),
        extra,
    )
}

/// `/@([\w.-]+)\/components\/([\w.-]+)/`: the first such reference in a URL.
fn component_ref(url: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    while let Some(found) = url[offset..].find('@') {
        let start = offset + found + 1;
        let user = run_of_word_dots(&url[start..]);
        if !user.is_empty() {
            if let Some(rest) = url[start + user.len()..].strip_prefix("/components/") {
                let slug = run_of_word_dots(rest);
                if !slug.is_empty() {
                    return Some((user, slug));
                }
            }
        }
        offset = start;
    }
    None
}

struct TagRow<'a> {
    title: &'a str,
    user: &'a str,
    slug: &'a str,
    end: usize,
}

/// `/^- \[([^\]]+)\]\(https:\/\/21st\.dev\/@USER\/components\/SLUG(?:\/[\w.-]+)?\.md\)/`.
fn link_row<'a>(
    text: &'a str,
    start: usize,
    fixed_user: Option<&str>,
    allow_variant: bool,
) -> Option<TagRow<'a>> {
    let rest = text[start..].strip_prefix("- [")?;
    let close = rest.find(']')?;
    if close == 0 {
        return None;
    }
    let title = &rest[..close];
    let after = rest[close..].strip_prefix("](https://21st.dev/@")?;
    let user = match fixed_user {
        Some(expected) => {
            after.strip_prefix(expected)?;
            &after[..expected.len()]
        }
        None => run_of_word_dots(after),
    };
    if user.is_empty() {
        return None;
    }
    let tail = after[user.len()..].strip_prefix("/components/")?;
    let run = run_of_word_dots(tail);
    let after_run = &tail[run.len()..];
    let (slug, closing) =
        if let Some(variant) = after_run.strip_prefix('/').filter(|_| allow_variant) {
            let second = run_of_word_dots(variant);
            second.strip_suffix(".md").filter(|stem| !stem.is_empty())?;
            (run, variant[second.len()..].strip_prefix(')')?)
        } else {
            (run.strip_suffix(".md")?, after_run.strip_prefix(')')?)
        };
    if slug.is_empty() {
        return None;
    }
    Some(TagRow {
        title,
        user,
        slug,
        end: text.len() - closing.len(),
    })
}

fn scan_rows<'a>(text: &'a str, fixed_user: Option<&str>, allow_variant: bool) -> Vec<TagRow<'a>> {
    let mut rows = Vec::new();
    let mut position = 0;
    for start in js::line_starts(text) {
        if start < position {
            continue;
        }
        if let Some(row) = link_row(text, start, fixed_user, allow_variant) {
            position = row.end;
            rows.push(row);
        }
    }
    rows
}

fn search_21st_with_key(
    source: &Source,
    groups: &[Group],
    key: &str,
    limit: usize,
) -> Res<(Vec<Object>, Vec<String>)> {
    let query = groups
        .iter()
        .map(|group| group.synonyms.first().unwrap_or(&group.token).clone())
        .collect::<Vec<_>>()
        .join(" ");
    let url = format!(
        "https://21st.dev/api/v1/components/search?q={}&scope=public&limit={}",
        enc(&query),
        limit.min(50)
    );
    let body = not_null(fetch_json_with(&url, &bearer(key))?)?;
    let mut results = Object::new();
    if let Some(Value::Array(rows)) = body.get("results") {
        for row in rows {
            if row.is_null() {
                return Err(Fail::other(
                    "Cannot read properties of null (reading 'url')",
                ));
            }
            let reference = row
                .get("url")
                .filter(|url| !url.is_null())
                .map_or(String::new(), js::string_of);
            if let Some((user, slug)) = component_ref(&reference) {
                let item = to_candidate_21st(
                    source,
                    user,
                    slug,
                    coalesce(row.get("name"), json!(slug)),
                    coalesce(row.get("description"), json!("")),
                    coalesce(row.get("tags"), json!([])),
                    true,
                );
                results.insert(format!("{user}/{slug}"), Value::Object(item));
            }
        }
    }
    Ok((objects(results), Vec::new()))
}

fn objects(map: Object) -> Vec<Object> {
    map.into_iter()
        .filter_map(|(_, value)| value.as_object().cloned())
        .collect()
}

fn search_21st_by_tags(source: &Source, groups: &[Group]) -> Res<(Vec<Object>, Vec<String>)> {
    let mut tags: Vec<String> = Vec::new();
    for term in groups
        .iter()
        .flat_map(|group| std::iter::once(&group.token).chain(&group.synonyms))
    {
        if !tags.contains(term) {
            tags.push(term.clone());
        }
    }
    tags.truncate(5);
    let pages = map_limit(&tags, tags.len().max(1), |tag| {
        let reply = request(
            &format!("https://21st.dev/community/components/s/{}.md", enc(tag)),
            &Vec::new(),
        )?;
        if reply.ok() {
            Ok(Some(reply.text()?))
        } else {
            Ok(None)
        }
    })?;
    let found = pages.iter().flatten().count();
    let mut results = Object::new();
    for (tag, page) in tags.iter().zip(&pages) {
        let Some(text) = page else { continue };
        for row in scan_rows(text, None, true) {
            let id = format!("{}/{}", row.user, row.slug);
            match results
                .get_mut(&id)
                .and_then(|existing| existing.get_mut("keywords"))
                .and_then(Value::as_array_mut)
            {
                Some(keywords) => keywords.push(json!(tag)),
                None => {
                    let item = to_candidate_21st(
                        source,
                        row.user,
                        row.slug,
                        json!(row.title),
                        json!(""),
                        json!([tag]),
                        false,
                    );
                    results.insert(id, Value::Object(item));
                }
            }
        }
    }
    if found == 0 {
        return Err(Fail::source(format!(
            "21st: no tag page for {}",
            tags.join(", ")
        )));
    }
    Ok((
        objects(results),
        vec!["21st.dev without a key: tag search only, up to 60 results per tag".to_string()],
    ))
}

// --- The catalog ----------------------------------------------------------

pub enum Kind {
    Registry(RegistrySpec),
    Community(community::Spec),
    Single(SinglePackage),
    Radix,
    BaseUi,
    UiSdk,
    RareUi,
    Live21st,
}

pub struct Source {
    pub id: &'static str,
    pub title: &'static str,
    pub access: &'static str,
    pub license: Option<&'static str>,
    pub live: bool,
    pub kind: Kind,
}

fn load_aceternity(source: &Source, spec: &RegistrySpec) -> Res<Vec<Object>> {
    match load_registry(source, spec) {
        Err(error) if !error.source_error => Err(error),
        Err(_) => {
            let text = fetch_text(&format!(
                "https://21st.dev/@{ACETERNITY_OWNER}/library/aceternity-ui.md"
            ))?;
            Ok(scan_rows(&text, Some(ACETERNITY_OWNER), false)
                .into_iter()
                .map(|row| {
                    with_gaps(
                        candidate(
                            source,
                            vec![
                                ("license", json!("MIT")),
                                ("licenseAssumed", json!(true)),
                                ("name", json!(row.slug)),
                                ("title", json!(row.title)),
                                ("mirror", json!(true)),
                                (
                                    "previewUrl",
                                    json!(format!(
                                        "https://21st.dev/@{ACETERNITY_OWNER}/components/{}",
                                        row.slug
                                    )),
                                ),
                            ],
                        ),
                        &["partial-mirror", "needs-key"],
                    )
                })
                .collect())
        }
        ok => ok,
    }
}

impl Source {
    pub fn load(&self) -> Res<Vec<Object>> {
        match &self.kind {
            Kind::Registry(spec) if self.id == "aceternity" => load_aceternity(self, spec),
            Kind::Registry(spec) => load_registry(self, spec),
            Kind::Community(spec) => community::load(self, spec),
            Kind::Single(spec) => load_single(self, spec),
            Kind::Radix => load_radix(self),
            Kind::BaseUi => load_base_ui(self),
            Kind::UiSdk => load_ui_sdk(self),
            Kind::RareUi => load_rareui(self),
            Kind::Live21st => Ok(Vec::new()),
        }
    }

    pub fn detail(&self, item: &Object) -> Res<Object> {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        match &self.kind {
            Kind::Registry(_) if self.id == "aceternity" && truthy(item.get("mirror")) => {
                detail_21st(item, ACETERNITY_OWNER, name)
            }
            Kind::Registry(spec) => registry_detail(spec, item),
            Kind::Community(spec) => community::detail(spec, item),
            Kind::Single(_) | Kind::Radix | Kind::BaseUi | Kind::UiSdk => package_detail(item),
            Kind::RareUi => rareui_detail(item),
            Kind::Live21st => {
                let mut parts = name.split('/');
                detail_21st(item, parts.next().unwrap_or(""), parts.next().unwrap_or(""))
            }
        }
    }

    pub fn search_live(&self, groups: &[Group]) -> Res<(Vec<Object>, Vec<String>)> {
        match key_21st() {
            Some(key) => search_21st_with_key(self, groups, &key, 25),
            None => search_21st_by_tags(self, groups),
        }
    }

    /// A 21st component addressed directly by `user/slug`.
    pub fn from_id(&self, name: &str) -> Option<Object> {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return None;
        }
        Some(to_candidate_21st(
            self,
            parts[0],
            parts[1],
            json!(title_from_slug(parts[1])),
            json!(""),
            json!([]),
            key_21st().is_some(),
        ))
    }
}

const fn registry(
    id: &'static str,
    title: &'static str,
    access: &'static str,
    spec: RegistrySpec,
    license: &'static str,
) -> Source {
    Source {
        id,
        title,
        access,
        license: Some(license),
        live: false,
        kind: Kind::Registry(spec),
    }
}

const fn single(id: &'static str, title: &'static str, spec: SinglePackage) -> Source {
    Source {
        id,
        title,
        access: "npm package",
        license: None,
        live: false,
        kind: Kind::Single(spec),
    }
}

fn shadcn_item(name: &str) -> String {
    format!(
        "https://ui.shadcn.com/r/styles/new-york-v4/{}.json",
        enc(name)
    )
}
fn magic_item(name: &str) -> String {
    format!("https://magicui.design/r/{}.json", enc(name))
}
fn aceternity_item(name: &str) -> String {
    format!("https://ui.aceternity.com/registry/{}.json", enc(name))
}
fn kokonut_item(name: &str) -> String {
    format!("{KOKONUT_RAW}/{}.json", enc(name))
}
fn beautiful_item(name: &str) -> String {
    format!("https://www.beautifului.dev/r/{}.json", enc(name))
}

pub static SOURCES: &[Source] = &[
    registry("shadcn", "shadcn/ui", "registry", RegistrySpec { index_url: "https://ui.shadcn.com/r/index.json", item_url: shadcn_item, previews: Previews::Shadcn, enrich: Enrich::Shadcn, probe: true }, "MIT"),
    registry("magicui", "Magic UI", "registry", RegistrySpec { index_url: "https://magicui.design/r/registry.json", item_url: magic_item, previews: Previews::Magic, enrich: Enrich::None, probe: false }, "MIT"),
    registry("aceternity", "Aceternity UI", "registry", RegistrySpec { index_url: "https://ui.aceternity.com/registry/index.json", item_url: aceternity_item, previews: Previews::Aceternity, enrich: Enrich::Aceternity, probe: true }, "Aceternity License"),
    registry("kokonut", "Kokonut UI", "registry via GitHub", RegistrySpec { index_url: "https://raw.githubusercontent.com/kokonut-labs/kokonutui/main/public/r/registry.json", item_url: kokonut_item, previews: Previews::Kokonut, enrich: Enrich::None, probe: false }, "MIT"),
    registry("beautifului", "Beautiful UI", "registry", RegistrySpec { index_url: "https://www.beautifului.dev/r/registry.json", item_url: beautiful_item, previews: Previews::Beautiful, enrich: Enrich::Beautiful, probe: false }, "MIT"),
    Source { id: "21st", title: "21st.dev", access: "live API / Markdown", license: None, live: true, kind: Kind::Live21st },
    Source { id: "rareui", title: "ShitUI (formerly RareUI)", access: "registry + repo docs", license: Some("MIT"), live: false, kind: Kind::RareUi },
    Source { id: "base-ui", title: "Base UI", access: "llms.txt + npm package", license: None, live: false, kind: Kind::BaseUi },
    Source { id: "radix", title: "Radix Primitives", access: "repo docs + npm package", license: None, live: false, kind: Kind::Radix },
    Source { id: "ui-sdk", title: "UI SDK (Vercel Labs)", access: "repo docs + examples", license: None, live: false, kind: Kind::UiSdk },
    single("sonner", "Sonner", SinglePackage { package: "sonner", docs: Some("https://sonner.emilkowal.ski"), description: "An opinionated toast notification component for React.", keywords: &["toast", "notification", "alert", "snackbar"], curated: true, unmaintained: false, repo: "emilkowalski/sonner", dir: "src" }),
    single("vaul", "Vaul", SinglePackage { package: "vaul", docs: Some("https://vaul.emilkowal.ski"), description: "Unstyled drawer and bottom sheet component for React.", keywords: &["drawer", "bottom", "sheet", "mobile"], curated: false, unmaintained: true, repo: "emilkowalski/vaul", dir: "src" }),
    single("cmdk", "cmdk", SinglePackage { package: "cmdk", docs: None, description: "Fast, composable command menu for React (Cmd+K palette). No live demo page: the old site redirects to the GitHub repository.", keywords: &["command", "menu", "palette", "search", "combobox", "cmdk"], curated: true, unmaintained: false, repo: "dip/cmdk", dir: "cmdk/src" }),
    single("number-flow", "NumberFlow", SinglePackage { package: "@number-flow/react", docs: Some("https://number-flow.barvian.me"), description: "Animated number component with digit transitions.", keywords: &["number", "counter", "animated", "price", "stat"], curated: true, unmaintained: false, repo: "barvian/number-flow", dir: "packages/react/src" }),
    community::source("evilcharts", "Evil Charts", "MIT", community::Spec { index: "https://evilcharts.com/r/registry.json", item: "https://evilcharts.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/legions-developer/evilcharts/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("origin", "Origin UI", "MIT", community::Spec { index: "https://raw.githubusercontent.com/cosscom/ui/main/apps/origin/registry.json", item: "https://raw.githubusercontent.com/cosscom/ui/main/apps/origin/public/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/cosscom/ui/HEAD/apps/origin/LICENSE.md", policy: community::Policy::Public }),
    community::source("kibo", "Kibo UI", "MIT", community::Spec { index: "https://www.kibo-ui.com/r/registry.json", item: "https://www.kibo-ui.com/r/{name}.json", preview: Some("https://www.kibo-ui.com/components/{name}"), license_url: "https://raw.githubusercontent.com/shadcnblocks/kibo/HEAD/license.md", policy: community::Policy::Public }),
    community::source("reui", "ReUI", "MIT", community::Spec { index: "https://reui.io/r/styles/base-nova/registry.json", item: "https://reui.io/r/styles/base-nova/{name}.json", preview: Some("https://reui.io/docs/{name}"), license_url: "https://raw.githubusercontent.com/keenthemes/reui/HEAD/LICENSE.md", policy: community::Policy::UiOnly }),
    community::source("blocks-so", "blocks.so", "MIT", community::Spec { index: "https://blocks.so/r/registry.json", item: "https://blocks.so/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/ephraimduncan/blocks/HEAD/LICENSE.md", policy: community::Policy::Public }),
    community::source("motion-primitives", "Motion Primitives", "MIT", community::Spec { index: "https://motion-primitives.com/c/registry.json", item: "https://motion-primitives.com/c/{name}.json", preview: Some("https://motion-primitives.com/docs/{name}"), license_url: "https://raw.githubusercontent.com/ibelick/motion-primitives/HEAD/LICENCE.md", policy: community::Policy::Public }),
    community::source("smoothui", "SmoothUI", "MIT", community::Spec { index: "https://smoothui.dev/r/registry.json", item: "https://smoothui.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/educlopez/smoothui/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("fancy", "Fancy Components", "MIT", community::Spec { index: "https://fancycomponents.dev/r/registry.json", item: "https://fancycomponents.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/danielpetho/fancy/HEAD/LICENSE", policy: community::Policy::UiOnly }),
    community::source("beui", "BeUI", "MIT", community::Spec { index: "https://beui.dev/r/registry.json", item: "https://beui.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/starc007/ui-components/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("uiarc", "UIArc", "MIT", community::Spec { index: "https://uiarc.dev/r/registry.json", item: "https://uiarc.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/kuratlielia/arc-library/HEAD/LICENSE", policy: community::Policy::FreeTier }),
    community::source("devl", "devl.dev", "MIT", community::Spec { index: "https://devl.dev/r/registry.json", item: "https://devl.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/sean-brydon/devl.dev/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("cult", "Cult UI", "MIT", community::Spec { index: "https://www.cult-ui.com/r/registry.json", item: "https://www.cult-ui.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/nolly-studio/cult-ui/HEAD/LICENSE.md", policy: community::Policy::Public }),
    community::source("8bitcn", "8bitcn", "MIT", community::Spec { index: "https://www.8bitcn.com/r/registry.json", item: "https://www.8bitcn.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/TheOrcDev/8bitcn-ui/HEAD/license.md", policy: community::Policy::Public }),
    community::source("shadcn-space", "Shadcn Space", "MIT", community::Spec { index: "https://shadcnspace.com/r/registry.json", item: "https://shadcnspace.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/shadcnspace/shadcnspace/HEAD/LICENSE", policy: community::Policy::RepoFree }),
    community::source("ui-layouts", "UI Layouts", "MIT", community::Spec { index: "https://ui-layouts.com/r/registry.json", item: "https://ui-layouts.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/ui-layouts/uilayouts/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("spectrum", "Spectrum UI", "Apache-2.0", community::Spec { index: "https://ui.spectrumhq.in/r/registry.json", item: "https://ui.spectrumhq.in/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/arihantcodes/spectrum-ui/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("ruixen", "Ruixen UI", "MIT", community::Spec { index: "https://ruixen.com/registry.json", item: "https://ruixen.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/ruixenui/ruixen.com/HEAD/LICENSE.md", policy: community::Policy::Public }),
    community::source("watermelon", "Watermelon UI", "MIT", community::Spec { index: "https://registry.watermelon.sh/r/registry.json", item: "https://registry.watermelon.sh/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/WatermelonCorp/watermelon-platform/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("avara", "Avara UI", "MIT", community::Spec { index: "https://avaraui.com/r/registry.json", item: "https://avaraui.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/AlgorizeTech/Avara-UI/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("hextaui", "HextaUI", "MIT", community::Spec { index: "https://hextaui.com/r/registry.json", item: "https://hextaui.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/preetsuthar17/hextaui/HEAD/LICENSE", policy: community::Policy::UiOnly }),
    community::source("vengeance", "Vengeance UI", "MIT", community::Spec { index: "https://vengenceui.com/r/registry.json", item: "https://vengenceui.com/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/Ashutoshx7/VengeanceUI/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("componentry", "Componentry", "MIT", community::Spec { index: "https://componentry.dev/r/registry.json", item: "https://componentry.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/harshjdhv/componentry/HEAD/LICENSE", policy: community::Policy::Public }),
    community::source("asciicn", "asciicn", "MIT", community::Spec { index: "https://asciicn.dev/r/registry.json", item: "https://asciicn.dev/r/{name}.json", preview: None, license_url: "https://raw.githubusercontent.com/opencoredev/asciicn/HEAD/LICENSE", policy: community::Policy::Public }),
];

pub fn source_by_id(id: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|source| source.id == id)
}

/// ok = page exists, broken = page is gone, unverified = could not tell, none = no page known.
pub fn preview_status(url: Option<&Value>) -> &'static str {
    if !truthy(url) {
        return "none";
    }
    let Some(url) = url.and_then(Value::as_str) else {
        return "unverified";
    };
    if url.contains('#') {
        return "unverified";
    }
    let Ok(reply) = request(url, &Vec::new()) else {
        return "unverified";
    };
    if host_of(&reply.final_url).ok() != host_of(&reply.requested_url).ok() {
        return "unverified";
    }
    if reply.ok() {
        "ok"
    } else if matches!(reply.status, 404 | 410) {
        "broken"
    } else {
        "unverified"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_follows_the_js_expression() {
        assert_eq!(
            frontmatter("title: \"Hello\"\ndescription:  x y \n", "description"),
            "x y"
        );
        assert_eq!(
            frontmatter("description:\n  next line\n", "description"),
            "next line"
        );
        assert_eq!(frontmatter("nothing", "description"), "");
    }

    #[test]
    fn title_case_matches_word_boundaries() {
        assert_eq!(title_from_slug("data-table_x 2nd"), "Data Table_x 2nd");
        assert_eq!(split_camel("LiquidButtonX"), "Liquid-Button-X");
    }

    #[test]
    fn tag_rows_parse_slug_and_variant() {
        let text = "- [A](https://21st.dev/@u.x/components/slug-1.md)\n- [B](https://21st.dev/@u/components/s2/default.md) tail\n- bad\n";
        let rows = scan_rows(text, None, true);
        assert_eq!(
            rows.iter()
                .map(|row| (row.title, row.user, row.slug))
                .collect::<Vec<_>>(),
            vec![("A", "u.x", "slug-1"), ("B", "u", "s2")]
        );
    }

    #[test]
    fn base_ui_section_is_cut_at_the_next_heading() {
        let text = "## Intro\n## Components\n- [D](https://base-ui.com/react/components/dialog.md): Modal\n## Other\n- [E](https://base-ui.com/react/components/e.md): x\n";
        let section = base_ui_section(text).unwrap();
        assert!(section.contains("dialog") && !section.contains("/e.md"));
    }

    #[test]
    fn component_reference_is_found_in_urls() {
        assert_eq!(
            component_ref("https://x/@a.b/components/c-d?x=1"),
            Some(("a.b", "c-d"))
        );
        assert_eq!(component_ref("https://x/@/components/c"), None);
    }
}
