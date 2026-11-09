//! Reads the target project and reports what an example would need there. Never writes to the project.

use crate::js::Object;
use serde_json::Value;
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

/// Dependencies from the example that the project does not list; a version suffix is ignored.
pub fn missing_dependencies(detail: &Object, project: &Project) -> Vec<Value> {
    let mut list = detail
        .get("dependencies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for package in crate::imports::packages(&crate::cli::delivered_files(detail)) {
        if !list.iter().any(|value| {
            value.as_str().is_some_and(|v| {
                v.char_indices()
                    .find(|(i, c)| *i > 0 && *c == '@')
                    .map_or(v, |(i, _)| &v[..i])
                    == package
            })
        }) {
            list.push(Value::String(package));
        }
    }
    list.into_iter()
        .filter(|dependency| {
            let name = dependency.as_str().unwrap_or("");
            let bare = name
                .char_indices()
                .find(|(index, c)| *index > 0 && *c == '@')
                .map_or(name, |(index, _)| &name[..index]);
            bare != "cn" && !project.dependencies.contains_key(bare)
        })
        .collect()
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
    (!parts.is_empty() && !parts.iter().any(|part| *part == ".." || *part == "."))
        .then(|| parts.join("/"))
}
