//! Best-effort local module inventory. Reads delivered text only; never follows the host filesystem.
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug)]
enum Token {
    Word(String),
    String(String),
    Punct(char),
}

fn tokens(text: &str) -> Vec<Token> {
    let mut chars = text.chars().peekable();
    let mut out = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
            }
            '\'' | '"' | '`' => {
                let mut value = String::new();
                while let Some(next) = chars.next() {
                    if next == c {
                        break;
                    }
                    if next == '\\' {
                        chars.next();
                    } else {
                        value.push(next);
                    }
                }
                // Interpolated templates are not statically resolvable imports.
                if c != '`' || !value.contains("${") {
                    out.push(Token::String(value));
                }
            }
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let mut word = c.to_string();
                while chars
                    .peek()
                    .is_some_and(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                {
                    word.push(chars.next().unwrap());
                }
                out.push(Token::Word(word));
            }
            c if !c.is_whitespace() => out.push(Token::Punct(c)),
            _ => {}
        }
    }
    out
}

fn resolves(file: &str, import: &str, delivered: &HashSet<&str>) -> bool {
    let mut parts: Vec<&str> = file.split('/').collect();
    parts.pop();
    for part in import.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                if parts.pop().is_none() {
                    return false;
                }
            }
            part => parts.push(part),
        }
    }
    let path = parts.join("/");
    if delivered.contains(path.as_str()) {
        return true;
    }
    let stem = path
        .strip_suffix(".js")
        .or_else(|| path.strip_suffix(".jsx"))
        .or_else(|| path.strip_suffix(".mjs"))
        .or_else(|| path.strip_suffix(".cjs"));
    if stem.is_some_and(|stem| {
        [".ts", ".tsx", ".mts", ".cts"]
            .iter()
            .any(|ext| delivered.contains(format!("{stem}{ext}").as_str()))
    }) {
        return true;
    }
    [
        ".ts",
        ".tsx",
        ".js",
        ".jsx",
        ".mjs",
        ".cjs",
        ".mts",
        ".cts",
        ".json",
        ".css",
        "/index.ts",
        "/index.tsx",
        "/index.js",
        "/index.jsx",
        "/index.mjs",
        "/index.cjs",
    ]
    .iter()
    .any(|ext| delivered.contains(format!("{path}{ext}").as_str()))
}

pub fn specifiers(files: &[(String, String)]) -> Vec<(String, String)> {
    let mut imports = Vec::new();
    for (file, text) in files {
        if !["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"]
            .iter()
            .any(|ext| file.ends_with(&format!(".{ext}")))
        {
            continue;
        }
        let tokens = tokens(text);
        for (i, token) in tokens.iter().enumerate() {
            let Token::Word(word) = token else { continue };
            let next = match word.as_str() {
                "from" | "import" if matches!(tokens.get(i + 1), Some(Token::String(_))) => i + 1,
                "import" | "require" if matches!(tokens.get(i + 1), Some(Token::Punct('('))) => {
                    i + 2
                }
                _ => continue,
            };
            if let Some(Token::String(import)) = tokens.get(next) {
                let entry = (file.clone(), import.clone());
                if !imports.contains(&entry) {
                    imports.push(entry);
                }
            }
        }
    }
    imports
}

pub fn missing(files: &[(String, String)]) -> Vec<Value> {
    let delivered: HashSet<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    specifiers(files)
        .into_iter()
        .filter(|(file, import)| {
            if let Some(alias) = import.strip_prefix("@/") {
                // Check only delivered root/src paths; never consult the host project.
                !resolves("root.ts", alias, &delivered)
                    && !resolves("src/root.ts", alias, &delivered)
            } else {
                (import.starts_with("./") || import.starts_with("../"))
                    && !resolves(file, import, &delivered)
            }
        })
        .map(|(file, import)| json!({"file":file,"import":import}))
        .collect()
}

pub fn packages(files: &[(String, String)]) -> Vec<String> {
    let mut packages = Vec::new();
    for (_, import) in specifiers(files) {
        if import.starts_with(['.', '/', '#']) || import.starts_with("@/") || import.contains(':') {
            continue;
        }
        let parts: Vec<_> = import.split('/').collect();
        let name = if import.starts_with('@') {
            if parts.len() < 2 || parts[1].is_empty() {
                continue;
            }
            parts[..2].join("/")
        } else {
            parts[0].to_string()
        };
        if name != "cn" && !packages.contains(&name) {
            packages.push(name);
        }
    }
    packages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_delivered_modules_and_reports_static_missing_imports() {
        let files = vec![
            ("src/main.tsx".into(), "import type { A } from './a.js'; export { B } from './folder'; import './style.css'; const lazy = import('../LiquidWave'); const c = require('./absent'); import './side'; // import './comment'\n/* from './comment2' */ const str = \"from './string'\"; import('pkg'); import('@/alias');".into()),
            ("src/a.ts".into(), "".into()),
            ("src/folder/index.tsx".into(), "".into()),
            ("src/style.css".into(), "".into()),
        ];
        assert_eq!(
            missing(&files),
            vec![
                json!({"file":"src/main.tsx", "import":"../LiquidWave"}),
                json!({"file":"src/main.tsx", "import":"./absent"}),
                json!({"file":"src/main.tsx", "import":"./side"}),
                json!({"file":"src/main.tsx", "import":"@/alias"}),
            ]
        );
    }
}
