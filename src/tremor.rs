//! Tremor: the copy-and-paste components (Raw) and the dashboard blocks built on them.
//! The catalog comes from the repository tree; `get` follows each file's local imports.
use super::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq)]
pub enum Part {
    Raw,
    Blocks,
}

pub struct Spec {
    pub repo: &'static str,
    pub part: Part,
    pub docs_host: &'static str,
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
        access: "GitHub repository",
        license: Some(license),
        live: false,
        kind: Kind::Tremor(spec),
    }
}

/// Component folder, docs path on tremor.so, description and search words.
const RAW: &[(&str, &str, &str, &[&str])] = &[
    ("Accordion", "ui/accordion", "A vertical stack of headings that expand to show their content.", &["collapsible", "disclosure", "faq"]),
    ("AreaChart", "visualizations/area-chart", "Area chart: lines with shaded areas under them, with legend and tooltip, on Recharts.", &["chart", "graph", "trend", "dashboard"]),
    ("Badge", "ui/badge", "Small colored label that highlights a status or category.", &["tag", "label", "status", "pill"]),
    ("BarChart", "visualizations/bar-chart", "Bar chart to compare values across categories, with legend and tooltip, on Recharts.", &["chart", "graph", "comparison", "dashboard"]),
    ("BarList", "visualizations/barlist", "Ranked list of horizontal bars with a label inside each bar and its value.", &["ranking", "top", "list", "chart", "dashboard"]),
    ("Button", "ui/button", "Button with variants and a loading state.", &["action", "submit"]),
    ("Calendar", "inputs/calendar", "Calendar for choosing a single date or a range of dates.", &["date", "range", "day", "picker"]),
    ("Callout", "ui/callout", "Highlighted box for hints, tips, warnings and disclaimers.", &["alert", "notice", "banner", "info"]),
    ("Card", "ui/card", "Container that is the base of KPI cards, forms and sections.", &["panel", "container", "surface"]),
    ("CategoryBar", "visualizations/category-bar", "Segmented bar that shows a value against colored ranges, for progress, performance or status.", &["progress", "segmented", "threshold", "meter", "gauge"]),
    ("Checkbox", "inputs/checkbox", "Checkbox with checked, unchecked and indeterminate states.", &["check", "form", "toggle"]),
    ("ComboChart", "visualizations/combo-chart", "Bar and line chart on a single or dual axis, on Recharts.", &["chart", "dual", "axis", "graph", "dashboard"]),
    ("DatePicker", "inputs/date-picker", "Date and date range picker with presets.", &["date", "range", "calendar", "presets"]),
    ("Dialog", "ui/dialog", "Modal window over the page that makes the content beneath inactive.", &["modal", "popup", "overlay"]),
    ("Divider", "ui/divider", "Horizontal line that separates content, optionally with a label.", &["separator", "line"]),
    ("DonutChart", "visualizations/donut-chart", "Donut or pie chart for parts of a whole, on Recharts.", &["pie", "chart", "share", "dashboard"]),
    ("Drawer", "ui/drawer", "Panel that slides in from the right side of the screen.", &["sheet", "side", "panel", "overlay"]),
    ("DropdownMenu", "inputs/dropdown-menu", "Menu of options or actions opened from a button, with submenus and checkable items.", &["menu", "actions", "context"]),
    ("Input", "inputs/input", "Text input field with error state and optional search and password variants.", &["text", "field", "form", "search"]),
    ("Label", "inputs/label", "Accessible form label.", &["form", "field"]),
    ("LineChart", "visualizations/line-chart", "Line chart for one or more series over time, with legend and tooltip, on Recharts.", &["chart", "graph", "trend", "time", "dashboard"]),
    ("Popover", "ui/popover", "Floating panel with rich content, opened from a button.", &["floating", "overlay"]),
    ("ProgressBar", "visualizations/progress-bar", "Linear bar for progress, performance or status.", &["progress", "meter", "quota", "usage"]),
    ("ProgressCircle", "visualizations/progress-circle", "Circular indicator for progress, performance or status.", &["progress", "ring", "radial", "gauge"]),
    ("RadioCardGroup", "inputs/radio-card-group", "Radio buttons styled as selectable cards.", &["radio", "cards", "plan", "choice"]),
    ("RadioGroup", "inputs/radio-group", "Set of radio buttons where only one can be checked.", &["radio", "choice", "form"]),
    ("Select", "inputs/select", "Select list of options opened from a button.", &["dropdown", "options", "form"]),
    ("SelectNative", "inputs/select-native", "Native select element with custom styling.", &["dropdown", "native", "form"]),
    ("Slider", "inputs/slider", "Slider to choose one value or a range.", &["range", "slider", "input"]),
    ("SparkChart", "visualizations/spark-chart", "Small inline chart (sparkline) for a trend next to a number.", &["sparkline", "mini", "chart", "trend"]),
    ("Switch", "inputs/switch", "Toggle switch between on and off.", &["toggle", "on", "off"]),
    ("TabNavigation", "ui/tab-navigation", "Navigation links styled as tabs.", &["tabs", "nav", "links"]),
    ("Table", "ui/table", "Styled table parts for rows and columns of data.", &["data", "rows", "columns", "grid"]),
    ("Tabs", "ui/tabs", "Tabs that switch between panels of content.", &["tab", "panels", "segmented"]),
    ("Textarea", "inputs/textarea", "Multi-line text input.", &["text", "multiline", "form"]),
    ("Toast", "ui/toast", "Lightweight toast notifications with a toaster and hook.", &["notification", "snackbar", "alert"]),
    ("Toggle", "inputs/toggle", "One or more two-state buttons.", &["toggle", "button", "group"]),
    ("Tooltip", "ui/tooltip", "Popup with information about an element on hover or focus.", &["hint", "hover"]),
    ("Tracker", "visualizations/tracker", "Row of colored blocks for monitoring data such as uptime or status over time.", &["uptime", "status", "monitoring", "heatmap"]),
];

/// Block folder, its title and what the blocks in it are for.
const BLOCKS: &[(&str, &str, &str, &[&str])] = &[
    ("account-and-user-management", "Account and user management", "Settings and team pages: profile, members, roles, invitations.", &["settings", "team", "members", "profile", "roles"]),
    ("area-charts", "Area chart", "Dashboard cards with an area chart, summary numbers and period switches.", &["chart", "trend", "dashboard", "analytics"]),
    ("badges", "Badge", "Status and change badges for tables and KPI cards.", &["status", "tag", "delta", "label"]),
    ("banners", "Banner", "Announcement banners across the top of an app.", &["announcement", "notice", "promo"]),
    ("bar-charts", "Bar chart", "Dashboard cards with a bar chart, totals and category breakdowns.", &["chart", "comparison", "dashboard", "analytics"]),
    ("bar-lists", "Bar list", "Ranked lists with horizontal bars, such as top pages or top sources.", &["ranking", "top", "list", "analytics"]),
    ("billing-usage", "Billing and usage", "Plan, quota and usage overviews with progress bars and costs.", &["billing", "usage", "quota", "plan", "subscription"]),
    ("chart-compositions", "Chart composition", "Larger dashboard sections that combine charts, KPIs and lists.", &["dashboard", "chart", "overview", "analytics"]),
    ("chart-tooltips", "Chart tooltip", "Custom tooltips for Recharts charts: values, comparisons, colors.", &["tooltip", "chart", "hover", "recharts"]),
    ("dialogs", "Dialog", "Modal dialogs for confirmations, forms and invitations.", &["modal", "confirm", "popup"]),
    ("donut-charts", "Donut chart", "Cards with a donut chart and its legend or breakdown list.", &["pie", "share", "chart", "breakdown"]),
    ("empty-states", "Empty state", "Placeholders for screens with no data yet, with a call to action.", &["empty", "placeholder", "onboarding", "zero"]),
    ("feature-sections", "Feature section", "Marketing sections that present product features.", &["features", "marketing", "landing"]),
    ("file-upload", "File upload", "File upload areas with drop zone and file list.", &["upload", "dropzone", "attachment"]),
    ("filterbar", "Filter bar", "Filter bars for dashboards and tables: date range, selects, search.", &["filters", "date", "range", "toolbar", "dashboard"]),
    ("form-layouts", "Form layout", "Form layouts for settings and data entry.", &["form", "settings", "fields"]),
    ("grid-lists", "Grid list", "Grids of cards for projects, members or integrations.", &["cards", "grid", "list"]),
    ("kpi-cards", "KPI card", "KPI and metric cards: a value, its change against the previous period, often a spark chart or progress.", &["kpi", "metric", "stat", "stats", "dashboard", "trend"]),
    ("line-charts", "Line chart", "Dashboard cards with a line chart over time and summary numbers.", &["chart", "trend", "time", "dashboard"]),
    ("logins", "Login", "Sign-in and sign-up screens.", &["login", "signin", "signup", "auth"]),
    ("onboarding-feed", "Onboarding feed", "Onboarding checklists, steps and activity feeds.", &["onboarding", "checklist", "steps", "feed", "activity"]),
    ("page-shells", "Page shell", "App shells with sidebar or top navigation around the page content.", &["layout", "sidebar", "navigation", "shell"]),
    ("pricing-sections", "Pricing section", "Pricing tables and plan comparisons.", &["pricing", "plans", "tiers"]),
    ("spark-charts", "Spark chart", "Lists and cards with small inline trend charts.", &["sparkline", "trend", "mini", "chart"]),
    ("status-monitoring", "Status monitoring", "Uptime and service status overviews with trackers.", &["uptime", "status", "monitoring", "incidents"]),
    ("table-actions", "Table actions", "Tables with row selection and bulk actions.", &["table", "bulk", "selection", "actions"]),
    ("table-pagination", "Table pagination", "Tables with pagination controls.", &["table", "pagination", "pages"]),
    ("tables", "Table", "Data tables for dashboards.", &["table", "data", "rows"]),
];

/// `src/components/AreaChart/AreaChart.tsx` -> `AreaChart`.
fn raw_component(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("src/components/")?;
    let (folder, file) = rest.split_once('/')?;
    (file.strip_suffix(".tsx") == Some(folder) && safe_name(folder)).then_some(folder)
}

/// `src/content/components/kpi-cards/kpi-card-01.tsx` -> (`kpi-cards`, `kpi-card-01`).
fn block(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("src/content/components/")?;
    let (category, file) = rest.split_once('/')?;
    let name = file.strip_suffix(".tsx")?;
    (safe_name(category) && safe_name(name) && name != "index").then_some((category, name))
}

fn kebab(name: &str) -> String {
    split_camel(name).to_lowercase()
}

/// Search words that the title and description do not already say, so no variant is keyword-stuffed.
fn extra_words<'a>(words: Vec<&'a str>, title: &str, description: &str) -> Vec<&'a str> {
    let said = format!("{title} {description}").to_lowercase();
    let mut seen = HashSet::new();
    words
        .into_iter()
        .filter(|word| !said.contains(word) && seen.insert(*word))
        .collect()
}

/// Static text of the first `<h1>`..`<h4>` elements, without markup or expressions.
fn headings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (start, _) in text.match_indices("<h") {
        let rest = &text[start + 2..];
        if !rest.starts_with(['1', '2', '3', '4']) {
            continue;
        }
        let Some(open) = rest.find('>') else { continue };
        let inner = rest[open + 1..].split('<').next().unwrap_or("");
        let inner = inner.split_whitespace().collect::<Vec<_>>().join(" ");
        let inner = inner.replace("&#39;", "'").replace("&amp;", "&");
        if !inner.is_empty() && !inner.contains(['{', '}']) && !found.contains(&inner) {
            found.push(inner);
        }
        if found.len() == 2 {
            break;
        }
    }
    found
}

/// What a block is built from, read from its code, so variants of one category differ in search.
fn block_features(text: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (_, import) in crate::imports::specifiers(&[("block.tsx".into(), text.into())]) {
        let part = match import.strip_prefix("@/components/") {
            Some("Card") => continue,
            Some(name) => split_camel(name).replace('-', " ").to_lowercase(),
            None if import == "recharts" => "Recharts".to_string(),
            None => continue,
        };
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    let mut out = Vec::new();
    if !parts.is_empty() {
        out.push(format!("Built with {}.", parts.join(", ")));
    }
    let titles = headings(text);
    if !titles.is_empty() {
        let quoted: Vec<String> = titles.iter().map(|title| format!("\"{title}\"")).collect();
        out.push(format!("Headings: {}.", quoted.join(", ")));
    }
    out.join(" ")
}

pub fn load(source: &Source, spec: &Spec) -> Res<Vec<Object>> {
    let body = fetch_json_with(
        &format!(
            "https://api.github.com/repos/{}/git/trees/HEAD?recursive=1",
            spec.repo
        ),
        &github_headers(),
    )?;
    // Like a registry index: a wrong shape fails the source, an empty listing is an empty catalog.
    if !body.get("tree").is_some_and(Value::is_array) {
        return Err(Fail::source(format!(
            "{}: unexpected repository tree shape",
            source.id
        )));
    }
    let tree = tree_paths(&body);
    let items: Vec<Object> = match spec.part {
        Part::Raw => tree
            .iter()
            .filter_map(|path| {
                let folder = raw_component(path)?;
                let known = RAW.iter().find(|row| row.0 == folder);
                let mut keywords = vec!["tremor", "dashboard"];
                keywords.extend(known.map_or(&[][..], |row| row.3));
                let title = title_from_slug(&split_camel(folder)).replace('-', " ");
                let description = known.map_or("", |row| row.2);
                let keywords = extra_words(keywords, &title, description);
                Some(candidate(
                    source,
                    vec![
                        ("name", json!(kebab(folder))),
                        ("title", json!(title)),
                        ("description", json!(description)),
                        ("keywords", json!(keywords)),
                        ("sourceCode", json!({ "repo": spec.repo, "entry": path })),
                        (
                            "previewUrl",
                            known.map_or(Value::Null, |row| {
                                json!(format!("https://{}/docs/{}", spec.docs_host, row.1))
                            }),
                        ),
                    ],
                ))
            })
            .collect(),
        Part::Blocks => {
            let blocks: Vec<&String> = tree.iter().filter(|path| block(path).is_some()).collect();
            // A block that cannot be read keeps its category text; the source still loads.
            let texts = map_limit(&blocks, CONCURRENCY, |path| {
                Ok(fetch_text(&format!(
                    "https://raw.githubusercontent.com/{}/HEAD/{}",
                    spec.repo,
                    encode_path(path)
                ))
                .unwrap_or_default())
            })?;
            blocks
                .iter()
                .zip(texts)
                .filter_map(|(path, text)| {
                    let (category, name) = block(path)?;
                    let known = BLOCKS.iter().find(|row| row.0 == category);
                    let number = name.rsplit('-').next().unwrap_or("");
                    let title = match known {
                        Some(row) if number.chars().all(|c| c.is_ascii_digit()) => {
                            format!("{} {number}", row.1)
                        }
                        _ => title_from_slug(name).replace('-', " "),
                    };
                    let mut keywords = vec!["tremor", "block"];
                    keywords.extend(known.map_or(&[][..], |row| row.3));
                    let description = [known.map_or("", |row| row.2), &block_features(&text)]
                        .iter()
                        .filter(|part| !part.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" ");
                    let keywords = extra_words(keywords, &title, &description);
                    Some(candidate(
                        source,
                        vec![
                            ("name", json!(name)),
                            ("title", json!(title)),
                            ("description", json!(description)),
                            ("keywords", json!(keywords)),
                            ("category", json!(category)),
                            ("sourceCode", json!({ "repo": spec.repo, "entry": path })),
                            (
                                "previewUrl",
                                json!(format!("https://{}/blocks/{category}", spec.docs_host)),
                            ),
                            ("example", json!(true)),
                        ],
                    ))
                })
                .collect()
        }
    };
    // Neither repository has been updated since 2025.
    Ok(items
        .into_iter()
        .map(|item| with_gaps(item, &["unmaintained"]))
        .collect())
}

/// A local import of `file`, as a path in the repository tree, or None for packages.
fn resolve(file: &str, import: &str, tree: &HashSet<String>) -> Option<String> {
    let base = if let Some(alias) = import.strip_prefix("@/") {
        format!("src/{alias}")
    } else if import.starts_with("./") || import.starts_with("../") {
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
        parts.join("/")
    } else {
        return None;
    };
    ["", ".tsx", ".ts", ".css", "/index.tsx", "/index.ts"]
        .iter()
        .map(|ext| format!("{base}{ext}"))
        .find(|path| path.starts_with("src/") && tree.contains(path) && is_source_file(path))
}

/// The entry file and every local module it reaches, pinned to one commit.
pub fn detail(item: &Object) -> Res<Object> {
    let code = item.get("sourceCode").cloned().unwrap_or(Value::Null);
    let text_of = |key: &str| {
        code.get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Fail::other("source location missing"))
    };
    let (repo, entry) = (text_of("repo")?, text_of("entry")?);
    let commit = fetch_json_with(
        &format!("https://api.github.com/repos/{repo}/commits/HEAD"),
        &github_headers(),
    )?;
    let sha = commit
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or("HEAD")
        .to_string();
    let body = fetch_json_with(
        &format!("https://api.github.com/repos/{repo}/git/trees/{sha}?recursive=1"),
        &github_headers(),
    )?;
    if body.get("truncated").and_then(Value::as_bool) == Some(true) {
        return Err(Fail::source("GitHub source tree is truncated"));
    }
    let tree: HashSet<String> = tree_paths(&body).into_iter().collect();
    let mut detail = item.clone();
    if !tree.contains(&entry) {
        detail.insert("files".into(), json!([]));
        detail.insert("skipped".into(), json!([]));
        detail.insert(
            "sourceWarnings".into(),
            json!([format!("{entry} is no longer in {repo}")]),
        );
        return Ok(with_gaps(detail, &["no-source-listing"]));
    }
    let mut seen: HashSet<String> = HashSet::from([entry.clone()]);
    let mut queue = vec![entry];
    let mut files: Vec<(String, String)> = Vec::new();
    let mut skipped = Vec::new();
    while !queue.is_empty() {
        let fetched = map_limit(&queue, CONCURRENCY, |path| {
            let text = fetch_text(&format!(
                "https://raw.githubusercontent.com/{repo}/{sha}/{}",
                encode_path(path)
            ))?;
            Ok((path.clone(), text))
        })?;
        queue = Vec::new();
        for (path, text) in fetched {
            for (_, import) in crate::imports::specifiers(&[(path.clone(), text.clone())]) {
                let Some(next) = resolve(&path, &import, &tree) else {
                    continue;
                };
                if seen.insert(next.clone()) {
                    if seen.len() > MAX_SOURCE_FILES || is_excluded_file(&next) {
                        skipped.push(json!(next));
                    } else {
                        queue.push(next);
                    }
                }
            }
            files.push((path, text));
        }
    }
    files.sort();
    // Paths keep the upstream `src/` layout so the `@/` alias still resolves inside the example.
    let packages = crate::imports::packages(&files);
    detail.insert("dependencies".into(), json!(packages));
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
    detail.insert("sourceWarnings".into(), json!([]));
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

    #[test]
    fn tree_paths_map_to_components_and_blocks() {
        assert_eq!(
            raw_component("src/components/AreaChart/AreaChart.tsx"),
            Some("AreaChart")
        );
        assert_eq!(
            raw_component("src/components/AreaChart/areachart.stories.tsx"),
            None
        );
        assert_eq!(
            block("src/content/components/kpi-cards/kpi-card-01.tsx"),
            Some(("kpi-cards", "kpi-card-01"))
        );
        assert_eq!(block("src/content/components/kpi-cards/index.ts"), None);
        assert_eq!(kebab("DropdownMenu"), "dropdown-menu");
    }

    #[test]
    fn block_features_name_the_parts_and_static_headings() {
        let text = "import { Card } from '@/components/Card';\nimport { ProgressBar } from '@/components/ProgressBar';\nimport { Bar } from 'recharts';\n<h3 className=\"x\">\n  Lena&#39;s Website\n</h3><h4>{item.name}</h4><h2>Downtime</h2>";
        assert_eq!(
            block_features(text),
            "Built with progress bar, Recharts. Headings: \"Lena's Website\", \"Downtime\"."
        );
        assert_eq!(block_features("export default () => null"), "");
    }

    #[test]
    fn imports_resolve_through_the_alias_and_relative_paths() {
        let tree: HashSet<String> = [
            "src/components/Card.tsx",
            "src/lib/utils.ts",
            "src/utils/cx.ts",
            "src/components/Calendar/Calendar.tsx",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let block = "src/content/components/kpi-cards/kpi-card-01.tsx";
        assert_eq!(
            resolve(block, "@/components/Card", &tree).as_deref(),
            Some("src/components/Card.tsx")
        );
        assert_eq!(
            resolve(block, "@/lib/utils", &tree).as_deref(),
            Some("src/lib/utils.ts")
        );
        let raw = "src/components/DatePicker/DatePicker.tsx";
        assert_eq!(
            resolve(raw, "../../utils/cx", &tree).as_deref(),
            Some("src/utils/cx.ts")
        );
        assert_eq!(
            resolve(raw, "../Calendar/Calendar", &tree).as_deref(),
            Some("src/components/Calendar/Calendar.tsx")
        );
        assert_eq!(resolve(raw, "react", &tree), None);
        assert_eq!(resolve(raw, "../../../../etc", &tree), None);
    }
}
