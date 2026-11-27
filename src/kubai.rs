//! Kubai Icons: one entry per icon object, with its styles and badged versions folded in.
//! The catalog is the published package's metadata; `get` reads only the MIT paths at the
//! release tag of that package version.
use super::*;
use std::collections::HashSet;

pub struct Spec {
    pub repo: &'static str,
    pub package: &'static str,
    pub react: &'static str,
    pub site: &'static str,
}

pub const fn source(id: &'static str, title: &'static str, spec: Spec) -> Source {
    Source {
        id,
        title,
        access: "npm catalog + GitHub",
        license: Some("MIT"),
        live: false,
        kind: Kind::Kubai(spec),
    }
}

/// Style, its metadata in the package, the repository folder and extension of one icon,
/// and the React import path suffix.
const STYLES: [(&str, &str, &str, &str, &str); 3] = [
    ("layered", "dist/icons.json", "icons", ".js", ""),
    (
        "two-tone",
        "dist/duotone/icons.json",
        "icons-duotone",
        ".json",
        "/duotone",
    ),
    (
        "line",
        "dist/line/icons.json",
        "icons-line",
        ".json",
        "/line",
    ),
];

/// The paths LICENSING.md puts under the MIT LICENSE-ICONS. The rest of the repository is
/// source-available (the skill) or all rights reserved (site, tools, tests, docs): never read.
const MIT_DIRS: [&str; 5] = [
    "icons/",
    "badges/",
    "icons-duotone/",
    "icons-line/",
    "packages/",
];
const LICENSE_FILE: &str = "LICENSE-ICONS";
/// The React component every icon renders through.
const BINDING: [&str; 2] = [
    "packages/react/src/create.js",
    "packages/react/src/create.d.ts",
];
const REACT_README: &str = "packages/react/README.md";

fn is_mit(path: &str) -> bool {
    path == LICENSE_FILE || MIT_DIRS.iter().any(|dir| path.starts_with(dir))
}

/// The package's own export name: `link-2-plus` -> `Link2Plus`.
fn export_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper = true;
    for c in name.chars() {
        if c == '-' {
            upper = true;
            continue;
        }
        out.push(if upper { c.to_ascii_uppercase() } else { c });
        upper = false;
    }
    out
}

fn safe_version(version: &str) -> bool {
    !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

#[derive(Default)]
struct Family {
    name: String,
    category: String,
    tags: Vec<String>,
    /// Styles that draw the object itself.
    styles: Vec<&'static str>,
    /// Exact icon names per style, the object first where it exists.
    variants: Vec<(&'static str, Vec<String>)>,
    badges: Vec<String>,
}

fn text<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key).and_then(Value::as_str)
}

/// Folds every style's rows into one family per object, sorted by name.
fn families(styles: &[(&'static str, Vec<Value>)]) -> Vec<Family> {
    let mut out: Vec<Family> = Vec::new();
    for (style, rows) in styles {
        for row in rows {
            let Some(name) = text(row, "name").filter(|name| safe_name(name)) else {
                continue;
            };
            let base = text(row, "base").unwrap_or(name);
            if !safe_name(base) {
                continue;
            }
            let position = match out.iter().position(|family| family.name == base) {
                Some(position) => position,
                None => {
                    out.push(Family {
                        name: base.to_string(),
                        ..Family::default()
                    });
                    out.len() - 1
                }
            };
            let family = &mut out[position];
            if family.category.is_empty() {
                family.category = text(row, "category").unwrap_or("").to_string();
            }
            match family.variants.iter_mut().find(|(s, _)| s == style) {
                Some((_, names)) => names.push(name.to_string()),
                None => family.variants.push((*style, vec![name.to_string()])),
            }
            match text(row, "badge").filter(|_| name != base) {
                Some(badge) => {
                    if !family.badges.iter().any(|known| known == badge) {
                        family.badges.push(badge.to_string());
                    }
                }
                None => {
                    if !family.styles.contains(style) {
                        family.styles.push(*style);
                    }
                    // Tags of the plain icon only; badged rows repeat their badge's words.
                    for tag in row
                        .get("tags")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(tag) = tag.as_str().filter(|tag| tag.is_ascii()) {
                            let tag = tag.to_lowercase();
                            if !family.tags.contains(&tag) {
                                family.tags.push(tag);
                            }
                        }
                    }
                }
            }
        }
    }
    for family in &mut out {
        for (_, names) in &mut family.variants {
            names.sort_by_key(|name| (name != &family.name, name.clone()));
        }
    }
    out.retain(|family| !family.styles.is_empty());
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn entry(source: &Source, spec: &Spec, version: &str, family: &Family) -> Object {
    let words = family.name.replace('-', " ");
    // Styles and badges stay out of the searched text: shared by hundreds of objects, they
    // would lengthen the most complete entries and match unrelated words such as "line".
    let description = format!("Animated {words} icon that moves on hover.");
    let title = format!("{} icon", title_from_slug(&family.name));
    let mut words: Vec<&str> = vec!["icon", "kubai", family.category.as_str()];
    words.extend(family.tags.iter().map(String::as_str));
    let keywords = tremor::extra_words(words, &title, &description);
    let variants: Object = family
        .variants
        .iter()
        .map(|(style, names)| (style.to_string(), json!(names)))
        .collect();
    let preview = if family.styles.contains(&"layered") {
        json!(format!("https://{}/icons/{}/", spec.site, family.name))
    } else {
        // The site has a page only for objects drawn in the layered style.
        Value::Null
    };
    candidate(
        source,
        vec![
            ("name", json!(family.name)),
            ("title", json!(title)),
            ("description", json!(description)),
            ("keywords", json!(keywords)),
            ("category", json!(family.category)),
            ("variants", Value::Object(variants)),
            ("previewUrl", preview),
            ("dependencies", json!([spec.react])),
            ("version", json!(version)),
            (
                "sourceCode",
                json!({
                    "repo": spec.repo,
                    "tag": format!("{}@{version}", spec.package),
                    "styles": family.styles,
                }),
            ),
        ],
    )
}

pub fn load(source: &Source, spec: &Spec) -> Res<Vec<Object>> {
    let latest = not_null(fetch_json(&format!(
        "https://registry.npmjs.org/{}/latest",
        spec.package
    ))?)?;
    // The catalog is admitted only while the package itself stays MIT.
    if text(&latest, "license") != Some("MIT") {
        return Err(Fail::source(format!(
            "{}: the {} package is no longer MIT licensed",
            source.id, spec.package
        )));
    }
    let version = text(&latest, "version")
        .filter(|version| safe_version(version))
        .ok_or_else(|| Fail::source(format!("{}: package version missing", source.id)))?
        .to_string();
    let lists = map_limit(&STYLES, STYLES.len(), |(style, file, ..)| {
        let body = fetch_json(&format!(
            "https://cdn.jsdelivr.net/npm/{}@{version}/{file}",
            spec.package
        ))?;
        match body {
            Value::Array(rows) => Ok((*style, rows)),
            _ => Err(Fail::source(format!(
                "{}: unexpected icon metadata shape",
                source.id
            ))),
        }
    })?;
    Ok(families(&lists)
        .iter()
        .map(|family| entry(source, spec, &version, family))
        .collect())
}

/// A relative import of `file`, as a repository path, or None for packages and escapes.
fn resolve(file: &str, import: &str, tree: &HashSet<String>) -> Option<String> {
    if !(import.starts_with("./") || import.starts_with("../")) {
        return None;
    }
    let mut parts: Vec<&str> = file.split('/').collect();
    parts.pop();
    for part in import.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    let base = parts.join("/");
    ["", ".js"]
        .iter()
        .map(|ext| format!("{base}{ext}"))
        .find(|path| tree.contains(path))
}

fn commit_of(repo: &str, reference: &str) -> Res<String> {
    let commit = fetch_json_with(
        &format!(
            "https://api.github.com/repos/{repo}/commits/{}",
            enc(reference)
        ),
        &github_headers(),
    )?;
    commit
        .get("sha")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Fail::source("GitHub commit has no sha"))
}

fn usage(spec: &Spec, item: &Object, readme: Option<String>) -> String {
    let title = item.get("title").map_or(String::new(), js::string_of);
    let mut out = format!(
        "# {title} (Kubai Icons)\n\nAdd the `{}` package (MIT) and import the icon from it. The files beside this note show how the icon is drawn and how it moves; they are not meant to be copied into the project.\n\n| Style | Import |\n| --- | --- |\n",
        spec.react
    );
    for (style, .., suffix) in STYLES {
        let Some(names) = item
            .get("variants")
            .and_then(|variants| variants.get(style))
            .and_then(Value::as_array)
        else {
            continue;
        };
        let exports: Vec<String> = names
            .iter()
            .filter_map(Value::as_str)
            .map(export_name)
            .collect();
        out.push_str(&format!(
            "| {style} | `import {{ {} }} from '{}{suffix}'` |\n",
            exports.join(", "),
            spec.react
        ));
    }
    out.push_str("\nWithout `title` the icon is `aria-hidden`; pass `title` when the icon alone carries meaning, and give an icon-only button its own accessible name.\n");
    if let Some(readme) = readme.filter(|text| !text.is_empty()) {
        out.push_str("\n---\n\n");
        out.push_str(&readme);
    }
    out
}

/// The icon's drawing in each style plus the local modules it reaches, and the React binding,
/// all from MIT paths at the release tag.
pub fn detail(spec: &Spec, item: &Object) -> Res<Object> {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| safe_name(name))
        .ok_or_else(|| Fail::other("source location missing"))?;
    let code = item.get("sourceCode").cloned().unwrap_or(Value::Null);
    let repo = text(&code, "repo").unwrap_or(spec.repo).to_string();
    let tag = text(&code, "tag").unwrap_or("HEAD").to_string();
    let mut warnings: Vec<Value> = Vec::new();
    let sha = match commit_of(&repo, &tag) {
        Ok(sha) => sha,
        Err(_) => {
            warnings.push(json!(format!(
                "Release tag {tag} not found in {repo}; read the default branch instead"
            )));
            commit_of(&repo, "HEAD")?
        }
    };
    let body = fetch_json_with(
        &format!("https://api.github.com/repos/{repo}/git/trees/{sha}?recursive=1"),
        &github_headers(),
    )?;
    if body.get("truncated").and_then(Value::as_bool) == Some(true) {
        return Err(Fail::source("GitHub source tree is truncated"));
    }
    let tree: HashSet<String> = tree_paths(&body).into_iter().collect();
    let styles: Vec<&str> = code
        .get("styles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut queue: Vec<String> = Vec::new();
    for (style, _, folder, ext, _) in STYLES {
        if !styles.contains(&style) {
            continue;
        }
        let path = format!("{folder}/{name}{ext}");
        if tree.contains(&path) {
            queue.push(path);
        } else {
            warnings.push(json!(format!("{path} is not in {repo} at {sha}")));
        }
    }
    let drawings = queue.len();
    queue.extend(
        BINDING
            .iter()
            .chain([&LICENSE_FILE])
            .filter(|path| tree.contains(**path))
            .map(|path| path.to_string()),
    );
    let mut detail = item.clone();
    if drawings == 0 {
        detail.insert("files".into(), json!([]));
        detail.insert("skipped".into(), json!([]));
        detail.insert("sourceWarnings".into(), Value::Array(warnings));
        return Ok(with_gaps(detail, &["no-source-listing"]));
    }
    let mut seen: HashSet<String> = queue.iter().cloned().collect();
    let mut files: Vec<(String, String)> = Vec::new();
    let mut skipped = Vec::new();
    let raw = |path: &str| {
        fetch_text(&format!(
            "https://raw.githubusercontent.com/{repo}/{sha}/{}",
            encode_path(path)
        ))
    };
    while !queue.is_empty() {
        let fetched = map_limit(&queue, CONCURRENCY, |path| Ok((path.clone(), raw(path)?)))?;
        queue = Vec::new();
        for (path, content) in fetched {
            for (_, import) in crate::imports::specifiers(&[(path.clone(), content.clone())]) {
                let Some(next) = resolve(&path, &import, &tree) else {
                    continue;
                };
                if seen.insert(next.clone()) {
                    // A module outside the MIT paths is listed, never downloaded.
                    if !is_mit(&next) || seen.len() > MAX_SOURCE_FILES {
                        skipped.push(json!(next));
                    } else {
                        queue.push(next);
                    }
                }
            }
            files.push((path, content));
        }
    }
    files.sort();
    let mut dependencies = vec![spec.react.to_string()];
    for package in crate::imports::packages(&files) {
        if !dependencies.contains(&package) {
            dependencies.push(package);
        }
    }
    let readme = raw(REACT_README).ok();
    detail.insert("usage".into(), json!(usage(spec, item, readme)));
    detail.insert("dependencies".into(), json!(dependencies));
    detail.insert(
        "files".into(),
        Value::Array(
            files
                .into_iter()
                .map(|(path, content)| json!({ "path": path, "content": content }))
                .collect(),
        ),
    );
    detail.insert("skipped".into(), Value::Array(skipped));
    detail.insert("sourceWarnings".into(), Value::Array(warnings));
    detail.insert(
        "sourceUrl".into(),
        json!(format!("https://github.com/{repo}/tree/{sha}")),
    );
    detail.insert("sourceRef".into(), json!(sha));
    Ok(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(value: Value) -> Vec<Value> {
        value.as_array().unwrap().clone()
    }

    #[test]
    fn styles_and_badges_fold_into_one_family_per_object() {
        let layered = rows(json!([
            {"name": "folder", "category": "files", "tags": ["directory", "files", "文件"]},
            {"name": "folder-plus", "category": "files", "tags": ["directory", "add"], "base": "folder", "badge": "plus"},
            {"name": "folder-off", "category": "files", "tags": [], "base": "folder", "badge": "off"},
            {"name": "../bad", "category": "files", "tags": []}
        ]));
        let line = rows(json!([
            {"name": "folder", "category": "files", "tags": ["directory"]},
            {"name": "a-arrow-down", "category": "text", "tags": ["textformat.size.smaller"], "only": true},
            {"name": "ghost-check", "category": "x", "tags": [], "base": "ghost", "badge": "check"}
        ]));
        let found = families(&[("layered", layered), ("two-tone", vec![]), ("line", line)]);
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        // An object with only a badged version and no drawing of its own is left out.
        assert_eq!(names, vec!["a-arrow-down", "folder"]);
        let folder = &found[1];
        assert_eq!(folder.styles, vec!["layered", "line"]);
        assert_eq!(folder.badges, vec!["plus", "off"]);
        assert_eq!(folder.tags, vec!["directory", "files"]);
        assert_eq!(
            folder.variants[0],
            (
                "layered",
                vec![
                    "folder".to_string(),
                    "folder-off".into(),
                    "folder-plus".into()
                ]
            )
        );
    }

    #[test]
    fn export_names_follow_the_package() {
        assert_eq!(export_name("folder-plus"), "FolderPlus");
        assert_eq!(export_name("link-2-plus"), "Link2Plus");
        assert_eq!(export_name("printer-3d"), "Printer3d");
        assert_eq!(export_name("a-arrow-down"), "AArrowDown");
    }

    #[test]
    fn only_relative_imports_resolve_and_only_mit_paths_are_read() {
        let tree: HashSet<String> = [
            "icons/folder.js",
            "icons/_interface.js",
            "packages/core/src/kit.js",
            "src/engine.js",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            resolve("icons/folder.js", "../packages/core/src/kit.js", &tree).as_deref(),
            Some("packages/core/src/kit.js")
        );
        assert_eq!(
            resolve("icons/folder.js", "./_interface", &tree).as_deref(),
            Some("icons/_interface.js")
        );
        assert_eq!(resolve("icons/folder.js", "react", &tree), None);
        assert_eq!(resolve("icons/folder.js", "../../../etc", &tree), None);
        let engine = resolve("icons/folder.js", "../src/engine.js", &tree).unwrap();
        assert!(!is_mit(&engine));
        for path in [
            "SKILL.md",
            "PROMPT.md",
            "index.html",
            "scripts/check.mjs",
            "styles/line/motion.mjs",
            "site/src/app/page.tsx",
            "tools/build.mjs",
            "tests/core.test.js",
            "docs/authoring.md",
            "history/01-board.html",
            "assets/og.png",
            "LICENSE",
        ] {
            assert!(!is_mit(path), "{path}");
        }
        for path in [
            "icons/folder.js",
            "badges/plus.js",
            "icons-line/folder.json",
            "icons-duotone/folder.json",
            "packages/react/src/create.js",
            "LICENSE-ICONS",
        ] {
            assert!(is_mit(path), "{path}");
        }
    }
}
