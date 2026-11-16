//! Query expansion and ranking over normalized candidates, with measured large-catalog rejection.

use crate::js::{self, Object};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

const SYNONYMS: &[(&str, &[&str])] = &[
    ("carousel", &["slider"]),
    ("toast", &["notification"]),
    ("notification", &["toast"]),
    ("drawer", &["sheet"]),
    ("sheet", &["drawer"]),
    ("modal", &["dialog"]),
    ("dialog", &["modal"]),
    ("palette", &["command"]),
    ("counter", &["number"]),
];

const STOPWORDS: &[&str] = &[
    "and",
    "or",
    "to",
    "in",
    "on",
    "that",
    "i",
    "the",
    "a",
    "an",
    "with",
    "for",
    "of",
    "component",
    "components",
];

pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect()
}

fn tokenize(text: &str) -> Vec<String> {
    normalize(text)
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn singular(token: &str) -> String {
    if token.chars().count() > 3 && token.ends_with('s') {
        token[..token.len() - 1].to_string()
    } else {
        token.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Group {
    pub token: String,
    pub synonyms: Vec<String>,
}

/// One group per query word; a candidate matches the word through any term in its group.
pub fn expand_query(query: &str) -> Vec<Group> {
    tokenize(query)
        .into_iter()
        .filter(|token| !STOPWORDS.contains(&token.as_str()))
        .map(|token| {
            let base = singular(&token);
            let synonyms = SYNONYMS
                .iter()
                .find(|(word, _)| *word == base)
                .map_or(Vec::new(), |(_, list)| {
                    list.iter().map(|s| s.to_string()).collect()
                });
            Group { token, synonyms }
        })
        .collect()
}

struct Scored<'a> {
    candidate: &'a Object,
    score: f64,
}

fn id_of(candidate: &Object) -> &str {
    candidate.get("id").and_then(Value::as_str).unwrap_or("")
}

fn compare(a: &Scored, b: &Scored) -> Ordering {
    // `b.score - a.score || localeCompare`: an exact tie falls through to the id.
    let difference = b.score - a.score;
    if difference > 0.0 {
        Ordering::Greater
    } else if difference < 0.0 {
        Ordering::Less
    } else {
        js::locale_compare(id_of(a.candidate), id_of(b.candidate))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub limit: usize,
    pub per_source: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            limit: 20,
            per_source: 5,
        }
    }
}

fn select_top(ordered: Vec<Scored>, limits: Limits) -> Vec<Object> {
    let mut taken: HashMap<&str, usize> = HashMap::new();
    let mut result = Vec::new();
    for Scored { candidate, score } in ordered {
        let source = candidate
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("");
        let count = taken.entry(source).or_insert(0);
        if *count >= limits.per_source {
            continue;
        }
        *count += 1;
        let mut item = candidate.clone();
        item.insert(
            "score".into(),
            js::number_value(js::math_round(score * 1000.0) / 1000.0),
        );
        result.push(item);
        if result.len() >= limits.limit {
            break;
        }
    }
    result
}

const K1: f64 = 1.2;
const B: f64 = 0.75;
const FIELD_WEIGHTS: [(&str, f64); 4] = [
    ("name", 2.0),
    ("title", 1.0),
    ("keywords", 1.0),
    ("description", 1.0),
];

fn field_text(candidate: &Object, field: &str) -> String {
    match candidate.get(field) {
        None | Some(Value::Null) => String::new(),
        Some(Value::Array(items)) => js::join(items, " "),
        Some(value) => js::string_of(value),
    }
}

fn weighted_terms(candidate: &Object) -> (HashMap<String, f64>, f64) {
    let mut counts = HashMap::new();
    let mut length = 0.0;
    for (field, weight) in FIELD_WEIGHTS {
        for token in tokenize(&field_text(candidate, field)) {
            *counts.entry(singular(&token)).or_insert(0.0) += weight;
            length += weight;
        }
    }
    (counts, length)
}

/// Offline fallback: BM25 over name, title, keywords and description.
/// Reference policy retained for fixed-catalog experiments and Node parity.
pub fn rank(candidates: &[Object], query: &str, limits: Limits) -> Vec<Object> {
    rank_with_confidence(candidates, query, limits, 0.0)
}

// The 13-source reference catalog contained 678 rows. A larger eligible pool
// needs stronger evidence; scoped searches over a small library keep their recall.
const REFERENCE_CATALOG_SIZE: usize = 678;

/// Default CLI fallback, calibrated on the expanded development catalog.
pub fn rank_default(candidates: &[Object], query: &str, limits: Limits) -> Vec<Object> {
    let ratio = if candidates.len() > REFERENCE_CATALOG_SIZE {
        0.25
    } else {
        0.0
    };
    rank_with_confidence(candidates, query, limits, ratio)
}

fn rank_with_confidence(
    candidates: &[Object],
    query: &str,
    limits: Limits,
    min_idf_ratio: f64,
) -> Vec<Object> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for group in expand_query(query) {
        for term in std::iter::once(group.token).chain(group.synonyms) {
            let term = singular(&term);
            if seen.insert(term.clone()) {
                terms.push(term);
            }
        }
    }
    if terms.is_empty() || candidates.is_empty() {
        return Vec::new();
    }
    let documents: Vec<_> = candidates
        .iter()
        .map(|candidate| (candidate, weighted_terms(candidate)))
        .collect();
    let total = documents.len() as f64;
    let mut average = documents
        .iter()
        .map(|(_, (_, length))| *length)
        .sum::<f64>()
        / total;
    if average == 0.0 || average.is_nan() {
        average = 1.0;
    }
    let idf: Vec<f64> = terms
        .iter()
        .map(|term| {
            let found = documents
                .iter()
                .filter(|(_, (counts, _))| counts.contains_key(term))
                .count() as f64;
            js::log(1.0 + (total - found + 0.5) / (found + 0.5))
        })
        .collect();
    let minimum = min_idf_ratio * idf.iter().sum::<f64>();
    let mut ordered: Vec<Scored> = documents
        .iter()
        .map(|(candidate, (counts, length))| {
            let mut score = 0.0;
            for (term, weight) in terms.iter().zip(&idf) {
                let frequency = counts.get(term).copied().unwrap_or(0.0);
                score += weight * (frequency * (K1 + 1.0))
                    / (frequency + K1 * (1.0 - B + B * (length / average)));
            }
            Scored { candidate, score }
        })
        // Compare before display rounding: a very common exact match can round to 0.
        .filter(|scored| scored.score > 0.0 && scored.score >= minimum)
        .collect();
    ordered.sort_by(compare);
    select_top(ordered, limits)
}

/// Weights chosen on the Node version's evaluation split.
#[derive(Debug, Clone, Copy)]
pub struct Ranking {
    pub gap_penalty: f64,
    pub curated_bonus: f64,
    pub min_similarity: f64,
}

pub const RANKING: Ranking = Ranking {
    gap_penalty: 0.08,
    curated_bonus: 0.04,
    min_similarity: 0.6,
};

/// Default raw-cosine floor; the gap penalty and curated bonus remain unchanged.
pub fn default_ranking(candidate_count: usize) -> Ranking {
    Ranking {
        min_similarity: if candidate_count > REFERENCE_CATALOG_SIZE {
            0.66
        } else {
            RANKING.min_similarity
        },
        ..RANKING
    }
}

// Gaps that stop the code from being downloaded. Cosmetic gaps never affect order.
const CODE_BLOCKING: [&str; 3] = ["needs-key", "pro-only", "no-source-listing"];

fn has_blocking_gap(candidate: &Object) -> bool {
    candidate
        .get("gaps")
        .and_then(Value::as_array)
        .is_some_and(|gaps| {
            gaps.iter().any(|gap| {
                gap.as_str()
                    .is_some_and(|text| CODE_BLOCKING.contains(&text))
            })
        })
}

/// Primary ranking: cosine similarity.
pub fn rank_semantic(
    candidates: &[Object],
    similarity: &HashMap<String, f64>,
    limits: Limits,
    ranking: Ranking,
) -> Vec<Object> {
    let mut ordered: Vec<Scored> = candidates
        .iter()
        .filter_map(|candidate| {
            let value = similarity.get(id_of(candidate)).copied().unwrap_or(0.0);
            let score = value
                - if has_blocking_gap(candidate) {
                    ranking.gap_penalty
                } else {
                    0.0
                }
                + if js::truthy(candidate.get("curated")) {
                    ranking.curated_bonus
                } else {
                    0.0
                };
            (value >= ranking.min_similarity).then_some(Scored { candidate, score })
        })
        .collect();
    ordered.sort_by(compare);
    select_top(ordered, limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog(count: usize, name: &str) -> Vec<Object> {
        (0..count)
            .map(|i| {
                json!({"id": format!("test/{i}"), "source": "test", "name": name})
                    .as_object()
                    .unwrap()
                    .clone()
            })
            .collect()
    }

    #[test]
    fn large_keyword_pool_requires_query_support_but_small_pool_keeps_recall() {
        let query = "button qqqzzz rust native gtk desktop editor spreadsheet";
        let small = catalog(678, "button");
        let large = catalog(679, "button");
        assert!(!rank_default(&small, query, Limits::default()).is_empty());
        assert!(!rank(&large, query, Limits::default()).is_empty());
        assert!(rank_default(&large, query, Limits::default()).is_empty());
        assert!(!rank_default(&large, "button", Limits::default()).is_empty());
    }

    #[test]
    fn keyword_confidence_uses_unrounded_score() {
        let candidates = catalog(4000, "button");
        let result = rank_default(&candidates, "button", Limits::default());
        assert_eq!(result.len(), 5);
        assert_eq!(result[0]["score"], 0.0);
    }

    #[test]
    fn empty_keyword_queries_and_documents_stay_empty() {
        let candidates = catalog(679, "button");
        for query in ["", "the and of", "qqqzzz"] {
            assert!(rank_default(&candidates, query, Limits::default()).is_empty());
        }
        assert!(rank_default(&[], "button", Limits::default()).is_empty());
    }

    #[test]
    fn semantic_floor_depends_on_eligible_pool_and_precedes_bonus() {
        let mut candidates = catalog(679, "button");
        candidates[0].insert("curated".into(), json!(true));
        let scores = HashMap::from([("test/0".into(), 0.65), ("test/1".into(), 0.66)]);
        let small = rank_semantic(
            &candidates[..678],
            &scores,
            Limits::default(),
            default_ranking(678),
        );
        assert_eq!(small[0]["id"], "test/0");
        let large = rank_semantic(
            &candidates,
            &scores,
            Limits::default(),
            default_ranking(679),
        );
        assert_eq!(large.len(), 1);
        assert_eq!(large[0]["id"], "test/1");
        assert_eq!(default_ranking(37).min_similarity, 0.6);
        assert_eq!(default_ranking(679).curated_bonus, RANKING.curated_bonus);
        assert_eq!(default_ranking(679).gap_penalty, RANKING.gap_penalty);
    }
}
