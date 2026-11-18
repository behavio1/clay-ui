//! Reads the target project and reports what an example would need there. Never writes to the project.

use crate::js::Object;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub struct Project {
    pub react: Value,
    pub tailwind: Value,
    pub dependencies: Object,
}

/// `path.resolve`: absolute and lexically normalized, without touching the disk.
pub fn resolve(path: &str) -> PathBuf {
    let joined = std::env::current_dir().unwrap_or_default().join(path);
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub enum ProjectError {
    Usage(String),
    Other(String),
}

pub fn inspect_project(directory: &str) -> Result<Project, ProjectError> {
    let root = resolve(directory);
    let file = root.join("package.json");
    if !file.exists() {
        return Err(ProjectError::Usage(format!(
            "No package.json in {}",
            root.display()
        )));
    }
    let text = std::fs::read(&file).map_err(|error| ProjectError::Other(error.to_string()))?;
    let manifest: Value = serde_json::from_slice(&text)
        .map_err(|_| ProjectError::Usage(format!("Invalid package.json in {}", root.display())))?;
    if manifest.is_null() {
        return Err(ProjectError::Other("package.json is empty".into()));
    }
    let mut dependencies = Object::new();
    for field in ["dependencies", "devDependencies"] {
        if let Some(Value::Object(entries)) = manifest.get(field) {
            dependencies.extend(entries.clone());
        }
    }
    let lookup = |name: &str| {
        dependencies
            .get(name)
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or(Value::Null)
    };
    Ok(Project {
        react: lookup("react"),
        tailwind: lookup("tailwindcss"),
        dependencies,
    })
}

fn bare_name(spec: &str) -> &str {
    spec.char_indices()
        .find(|(i, c)| *i > 0 && *c == '@')
        .map_or(spec, |(i, _)| &spec[..i])
}

fn npm_name_part(part: &str) -> bool {
    (1..=214).contains(&part.len())
        && !part.starts_with(['.', '_', '-'])
        && part
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-._~".contains(&b))
}

fn semver_partial(text: &str) -> bool {
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    let (text, build) = text
        .split_once('+')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let (core, pre) = text
        .split_once('-')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let tag = |t: Option<&str>| {
        t.is_none_or(|t| {
            !t.is_empty()
                && t.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-.".contains(&b))
        })
    };
    let parts: Vec<&str> = core.split('.').collect();
    (1..=3).contains(&parts.len())
        && parts.iter().all(|part| {
            matches!(*part, "x" | "X" | "*")
                || (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        })
        && tag(pre)
        && tag(build)
}

fn semver_comparator(text: &str) -> bool {
    let rest = [">=", "<=", ">", "<", "=", "^", "~"]
        .iter()
        .find_map(|op| text.strip_prefix(op))
        .unwrap_or(text);
    semver_partial(rest)
}

fn npm_version(version: &str) -> bool {
    let bytes = version.as_bytes();
    let tag = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && !(matches!(bytes[0], b'v' | b'V') && bytes.get(1).is_some_and(u8::is_ascii_digit))
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(b));
    tag || version.split("||").all(|set| {
        let set = set.trim();
        match set.split_once(" - ") {
            Some((low, high)) => semver_partial(low.trim()) && semver_partial(high.trim()),
            None => !set.is_empty() && set.split_whitespace().all(semver_comparator),
        }
    })
}

/// A registry dependency npm resolves by name: `name`, `@scope/name`, optionally `@<range or tag>`.
/// Aliases, URLs, git, file paths and archives are not plain.
pub fn plain_dependency(spec: &str) -> bool {
    let name = bare_name(spec);
    let version = spec
        .get(name.len()..)
        .and_then(|rest| rest.strip_prefix('@'));
    if spec.len() > name.len() && version.is_none_or(|v| !npm_version(v)) {
        return false;
    }
    let valid_name = match name.strip_prefix('@') {
        Some(scoped) => scoped
            .split_once('/')
            .is_some_and(|(scope, part)| npm_name_part(scope) && npm_name_part(part)),
        None => npm_name_part(name),
    };
    valid_name
        && ![".tgz", ".tar", ".tar.gz"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

/// Upstream and imported dependencies split into plain npm specifiers and everything else.
fn dependency_lists(detail: &Object) -> (Vec<Value>, Vec<Value>) {
    let (mut plain, mut unverified) = (Vec::new(), Vec::new());
    let (mut listed, mut seen) = (HashSet::new(), HashSet::new());
    let upstream = detail
        .get("dependencies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let imported = crate::imports::packages(&crate::cli::delivered_files(detail));
    let upstream_count = upstream.len();
    for (index, value) in upstream
        .into_iter()
        .chain(imported.into_iter().map(Value::String))
        .enumerate()
    {
        match value.as_str() {
            Some(spec) if plain_dependency(spec) => {
                // Upstream entries stay as listed; an import is added only for a name not yet listed.
                if listed.insert(bare_name(spec).to_string()) || index < upstream_count {
                    plain.push(value);
                }
            }
            _ => {
                if seen.insert(value.to_string()) {
                    unverified.push(value);
                }
            }
        }
    }
    (plain, unverified)
}

/// Plain npm dependencies the project does not list (a version suffix is ignored), and the upstream or
/// imported dependencies that are not plain npm names, which are never presented as installable.
pub fn dependencies_for(detail: &Object, project: &Project) -> (Vec<Value>, Vec<Value>) {
    let (plain, unverified) = dependency_lists(detail);
    let missing = plain
        .into_iter()
        .filter(|dependency| {
            let bare = bare_name(dependency.as_str().unwrap_or(""));
            bare != "cn" && !project.dependencies.contains_key(bare)
        })
        .collect();
    (missing, unverified)
}

/// True for a path that coding agents load as instructions, rules or skills (CLAUDE.md, .claude/, .cursor/rules …).
pub fn agent_instruction_path(path: &str) -> bool {
    const FOLDERS: &[&str] = &[
        ".claude",
        ".agents",
        ".cursor",
        ".codex",
        ".windsurf",
        ".gemini",
        ".clinerules",
        ".roo",
        ".kiro",
    ];
    const FILES: &[&str] = &[
        "claude.md",
        "claude.local.md",
        "agents.md",
        "gemini.md",
        ".cursorrules",
        ".windsurfrules",
        ".clinerules",
        "copilot-instructions.md",
    ];
    const SUFFIXES: &[&str] = &[".mdc", ".instructions.md", ".prompt.md", ".chatmode.md"];
    let parts: Vec<String> = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let Some((file, folders)) = parts.split_last() else {
        return false;
    };
    folders.iter().any(|part| FOLDERS.contains(&part.as_str()))
        || FILES.contains(&file.as_str())
        || FOLDERS.contains(&file.as_str())
        || SUFFIXES.iter().any(|suffix| file.ends_with(suffix))
}

/// First agent instruction file or folder inside `folder`, without following links. A folder that
/// does not exist has none; a folder that cannot be fully read is an error, never "none found".
pub fn find_agent_instruction(folder: &Path) -> Result<Option<PathBuf>, String> {
    match std::fs::symlink_metadata(folder) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        // A linked destination is never scanned; the no-follow writer refuses it afterwards.
        Ok(meta) if meta.file_type().is_symlink() => return Ok(None),
        _ => {}
    }
    let fail =
        |path: &Path, error: std::io::Error| format!("Cannot check {}: {error}", path.display());
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|e| fail(&dir, e))? {
            let path = entry.map_err(|e| fail(&dir, e))?.path();
            let relative = path
                .strip_prefix(folder)
                .map_err(|_| format!("Cannot check {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            if agent_instruction_path(&relative) {
                return Ok(Some(path));
            }
            if std::fs::symlink_metadata(&path)
                .map_err(|e| fail(&path, e))?
                .is_dir()
            {
                stack.push(path);
            }
        }
    }
    Ok(None)
}

/// Relative output path for a downloaded file, or None when the registry path is unsafe.
pub fn safe_relative(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if value.is_empty()
        || value.starts_with('/')
        || (cfg!(windows) && value.starts_with('\\'))
        || Path::new(value).is_absolute()
        || drive
        || value.starts_with('~')
    {
        return None;
    }
    let parts: Vec<&str> = value
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    // A ':' in any segment is a Windows drive or stream prefix that would leave the folder.
    (!parts.is_empty()
        && !parts
            .iter()
            .any(|part| *part == ".." || *part == "." || part.contains(':')))
    .then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_dependencies_are_registry_names_with_optional_ranges() {
        for plain in [
            "react",
            "react@^19",
            "react@>=18 <20",
            "react@^18 || ^19",
            "react@1.2 - 2",
            "@radix-ui/react-dialog@1.1.2",
            "motion@latest",
            "x@1.0.0-beta.1+b",
            "x@*",
            "x@~1.2.x",
        ] {
            assert!(plain_dependency(plain), "{plain}");
        }
        for other in [
            "react@^",
            "react@",
            "payload.tgz",
            "package.tar.gz",
            "a.tar",
            "lodash@npm:evil",
            "clsx@https://x/c.tgz",
            "git+https://x/y.git",
            "github:a/b",
            "file:../x",
            "bad name",
            "",
            "@scope",
            "@scope/",
            ".hidden",
            "_x",
            "React",
            "x@v1 two three@",
            "x@1.2.3.4",
            "--global",
            "-g",
        ] {
            assert!(!plain_dependency(other), "{other}");
        }
    }

    #[test]
    fn agent_instruction_paths_cover_known_instruction_files_only() {
        for path in [
            "CLAUDE.md",
            "x/claude.local.md",
            "AGENTS.md",
            "GEMINI.md",
            ".claude/skills/a/SKILL.md",
            ".cursor/rules/a.mdc",
            "rules/a.mdc",
            ".github/copilot-instructions.md",
            ".github/instructions/a.instructions.md",
            "a.prompt.md",
            ".windsurfrules",
            "x/.agents/y.ts",
        ] {
            assert!(agent_instruction_path(path), "{path}");
        }
        for path in [
            "components/a.tsx",
            ".github/workflows/ci.yml",
            "README.md",
            "docs/claude.mdx",
        ] {
            assert!(!agent_instruction_path(path), "{path}");
        }
    }
}
