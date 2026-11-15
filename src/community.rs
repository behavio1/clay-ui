//! Public, permissively licensed registries. Keep admission separate from ranking.
use super::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq)]
pub enum Policy {
    Public,
    UiOnly,
    FreeTier,
    RepoFree,
}

pub struct Spec {
    pub index: &'static str,
    pub item: &'static str,
    pub preview: Option<&'static str>,
    pub license_url: &'static str,
    pub policy: Policy,
}

pub const fn source(
    id: &'static str,
    title: &'static str,
    license: &'static str,
    spec: Spec,
) -> Source {
    Source {
        id,
        title,
        access: "public registry",
        license: Some(license),
        live: false,
        kind: Kind::Community(spec),
    }
}

fn is_example(item: &Value) -> bool {
    let name = item["name"].as_str().unwrap_or("");
    name.ends_with("-demo")
        || name.contains("-demo-")
        || name.ends_with("-example")
        || item["files"].as_array().is_some_and(|files| {
            files.iter().any(|f| {
                f["path"]
                    .as_str()
                    .is_some_and(|p| p.contains("/examples/") || p.contains("/demos/"))
            })
        })
}

pub(super) fn public(item: &Value) -> bool {
    let meta = &item["meta"];
    meta["isPro"] != true
        && meta["pro"] != true
        && meta["premium"] != true
        && !matches!(meta["tier"].as_str(), Some("pro" | "premium" | "paid"))
        && item["isPro"] != true
        && item["pro"] != true
}

fn admitted(spec: &Spec, item: &Value, repo: &HashSet<String>) -> bool {
    let Some(name) = item["name"].as_str().filter(|name| safe_name(name)) else {
        return false;
    };
    if name == "cli" || !public(item) {
        return false;
    }
    let kind = item["type"].as_str().unwrap_or("");
    if !COMPONENT_TYPES.contains(&kind) {
        return false;
    }
    match spec.policy {
        Policy::UiOnly => kind == "registry:ui",
        Policy::FreeTier => item["meta"]["tier"] == "free",
        Policy::RepoFree => item["meta"]["isPro"] == false && repo.contains(name),
        Policy::Public => !is_example(item),
    }
}

fn items(body: &Value) -> Res<&Vec<Value>> {
    (if body.is_array() {
        Some(body)
    } else {
        body.get("items")
    })
    .and_then(Value::as_array)
    .ok_or_else(|| Fail::source("unexpected public registry index shape"))
}

pub fn load(source: &Source, spec: &Spec) -> Res<Vec<Object>> {
    let body = fetch_json(spec.index)?;
    // A live free flag alone does not establish that the code is in the MIT repository.
    let repo = if spec.policy == Policy::RepoFree {
        let repo_body = fetch_json(
            "https://raw.githubusercontent.com/shadcnspace/shadcnspace/main/registry.json",
        )?;
        items(&repo_body)?
            .iter()
            .filter_map(|i| i["name"].as_str().map(str::to_owned))
            .collect()
    } else {
        HashSet::new()
    };
    let entries: Vec<_> = items(&body)?
        .iter()
        .filter(|i| admitted(spec, i, &repo))
        .collect();
    let loaded = map_limit(&entries, 4, |entry| {
        let name = entry["name"].as_str().unwrap();
        // ReUI mixes premium blocks into its index. Admit only public UI modules,
        // and require a successful code-bearing anonymous response for every one.
        if source.id == "reui" {
            match fetch_json(&spec.item.replace("{name}", &enc(name))) {
                Ok(body)
                    if body["files"].as_array().is_some_and(|f| {
                        !f.is_empty()
                            && f.iter()
                                .all(|v| v["content"].as_str().is_some_and(|s| !s.is_empty()))
                    }) => {}
                Ok(_) => return Err(Fail::source(format!("reui/{name}: missing public code"))),
                Err(Fail {
                    status: Some(401 | 403 | 404),
                    ..
                }) => return Ok(None),
                Err(e) => return Err(e),
            }
        }
        let mut fields = vec![
            ("name", json!(name)),
            ("licenseUrl", json!(spec.license_url)),
        ];
        for key in [
            "title",
            "description",
            "dependencies",
            "registryDependencies",
            "categories",
        ] {
            if let Some(value) = entry.get(key).filter(|v| !v.is_null()) {
                fields.push((key, value.clone()));
            }
        }
        fields.push(("registryType", entry["type"].clone()));
        let keywords: Vec<Value> = [
            entry.get("categories"),
            entry.get("tags"),
            entry.get("meta").and_then(|m| m.get("tags")),
        ]
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .filter(|v| v.is_string())
        .cloned()
        .collect();
        if source.id == "origin" && name.starts_with("comp-") && !keywords.is_empty() {
            let tags = keywords
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            fields.push((
                "title",
                json!(format!("{} ({name})", title_from_slug(&tags))),
            ));
            fields.push(("description", json!(format!("Upstream tags: {tags}."))));
        }
        fields.push(("keywords", json!(keywords)));
        if let Some(url) = entry["meta"]["docs"]
            .as_str()
            .filter(|url| url.starts_with("https://"))
        {
            fields.push(("previewUrl", json!(url)));
        } else if let Some(pattern) = spec.preview {
            fields.push(("previewUrl", json!(pattern.replace("{name}", &enc(name)))));
        }
        // Blocks include compositions and variants; this is source metadata, not a ready-product claim.
        fields.push((
            "example",
            json!(
                entry["type"] == "registry:block"
                    || is_example(entry)
                    || (source.id == "origin" && name.starts_with("comp-"))
            ),
        ));
        Ok(Some(with_gaps(candidate(source, fields), &[])))
    })?;
    let mut seen = HashSet::new();
    Ok(loaded
        .into_iter()
        .flatten()
        .filter(|i| seen.insert(i["id"].clone().to_string()))
        .collect())
}

pub fn detail(spec: &Spec, item: &Object) -> Res<Object> {
    let name = item["name"]
        .as_str()
        .filter(|n| safe_name(n))
        .ok_or_else(|| Fail::source("invalid component name"))?;
    let url = spec.item.replace("{name}", &enc(name));
    let mut detail = registry_detail_url(&url, item)?;
    // Preserve the complete registry contract, including engine/tokens and CSS dependencies.
    // As with other registries, dependencies are reported for the agent; nothing is installed.
    detail.insert("licenseUrl".into(), json!(spec.license_url));
    if item.get("source").and_then(Value::as_str) == Some("asciicn") {
        bundle_ascii_engine(spec, &mut detail)?;
    }
    Ok(detail)
}

// asciicn components import a shared renderer/shader engine. Only explicit dependencies
// on its own registry can be fetched here; other libraries retain their existing contract.
fn bundle_ascii_engine(spec: &Spec, detail: &mut Object) -> Res<()> {
    let prefix = spec.item.split("{name}").next().unwrap();
    let mut queue: Vec<Value> = detail
        .get("registryDependencies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut registry_items = Vec::new();
    while let Some(dep) = queue.pop() {
        let Some(url) = dep.as_str() else {
            continue;
        };
        let Some(name) = url
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix(".json"))
            .filter(|n| safe_name(n))
        else {
            continue;
        };
        if !seen.insert(name.to_string()) {
            continue;
        }
        if seen.len() > 16 {
            return Err(Fail::source("asciicn: dependency limit exceeded"));
        }
        let body = fetch_json(url)?;
        if !public(&body) {
            return Err(Fail::source("asciicn: non-public engine dependency"));
        }
        let files = pick_files(&body)?;
        let target = detail
            .get_mut("files")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| Fail::source("asciicn: invalid dependency file list"))?;
        for f in files.as_array().unwrap() {
            if !target.contains(f) {
                target.push(f.clone());
            }
        }
        if let Some(deps) = body.get("registryDependencies").and_then(Value::as_array) {
            queue.extend(deps.iter().cloned());
        }
        if let Some(deps) = body.get("dependencies").and_then(Value::as_array) {
            let target = detail
                .get_mut("dependencies")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| Fail::source("asciicn: invalid dependency list"))?;
            for dep in deps {
                if !target.contains(dep) {
                    target.push(dep.clone());
                }
            }
        }
        let mut config = json!({"name":name,"sourceUrl":url})
            .as_object()
            .unwrap()
            .clone();
        for key in [
            "css",
            "cssVars",
            "tailwind",
            "dependencies",
            "registryDependencies",
        ] {
            if let Some(value) = body.get(key) {
                config.insert(key.into(), value.clone());
            }
        }
        registry_items.push(Value::Object(config));
    }
    detail.insert("registryItems".into(), json!(registry_items));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec(policy: Policy) -> Spec {
        Spec {
            index: "",
            item: "",
            preview: None,
            license_url: "",
            policy,
        }
    }
    #[test]
    fn types_pro_gates_and_examples_are_admission_rules() {
        let set = HashSet::new();
        for kind in [
            "theme", "style", "lib", "hook", "file", "item", "page", "example",
        ] {
            assert!(!admitted(
                &spec(Policy::Public),
                &json!({"name":"x","type":format!("registry:{kind}")}),
                &set
            ));
        }
        for kind in COMPONENT_TYPES {
            let item = json!({"name":"x","type":kind});
            assert!(admitted(&spec(Policy::Public), &item, &set));
            let mut paid = item.clone();
            paid["meta"] = json!({"isPro":true});
            assert!(!admitted(&spec(Policy::Public), &paid, &set));
        }
        assert!(!admitted(
            &spec(Policy::Public),
            &json!({"name":"x-demo","type":"registry:component"}),
            &set
        ));
        assert!(!admitted(
            &spec(Policy::Public),
            &json!({"name":"../x","type":"registry:ui"}),
            &set
        ));
        assert!(!admitted(
            &spec(Policy::UiOnly),
            &json!({"name":"x","type":"registry:block"}),
            &set
        ));
    }
    #[test]
    fn mixed_catalogs_fail_closed() {
        let repo = HashSet::from(["hero-01".to_string()]);
        let free = json!({"name":"hero-01","type":"registry:block","meta":{"isPro":false}});
        assert!(admitted(&spec(Policy::RepoFree), &free, &repo));
        assert!(!admitted(&spec(Policy::RepoFree), &free, &HashSet::new()));
        assert!(!admitted(
            &spec(Policy::RepoFree),
            &json!({"name":"hero-01","type":"registry:block"}),
            &repo
        ));
        assert!(!admitted(&spec(Policy::FreeTier), &free, &repo));
        assert!(admitted(
            &spec(Policy::FreeTier),
            &json!({"name":"x","type":"registry:ui","meta":{"tier":"free"}}),
            &repo
        ));
    }
}
