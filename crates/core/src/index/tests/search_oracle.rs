//! An independent model of the search rule, read straight from the fixture files, for comparing
//! with the index. It shares no code with the index's query, highlighter, or tokenizer, nor with
//! the workspace's tree or body reader: it tokenizes ASCII text itself and refuses anything else.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// A page as the fixture defines it.
pub(crate) struct Page {
    pub id: String,
    pub created_at: u64,
    pub title: String,
    pub active: bool,
    /// Each block's ID and text, in document order, empty blocks included.
    pub blocks: Vec<(String, String)>,
}

/// One expected hit: page ID, title parts, and the block ID with its parts.
pub(crate) type Hit = (String, Parts, Option<(String, Parts)>);
pub(crate) type Parts = Vec<(String, bool)>;

pub(crate) fn generated(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/generated")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

/// The 10,000 pages, created a second apart in file order from `created_at`.
pub(crate) fn search_pages(created_at: u64) -> Vec<Page> {
    let lines: Vec<serde_json::Value> = generated("search-10k-pages.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let parents: BTreeMap<String, (Option<String>, bool)> = lines
        .iter()
        .map(|page| {
            (
                page["id"].as_str().unwrap().to_owned(),
                (
                    page["parentId"].as_str().map(str::to_owned),
                    page["trash"].is_object(),
                ),
            )
        })
        .collect();
    let active = |start: &str| {
        let mut id = start.to_owned();
        for _ in 0..=parents.len() {
            let Some((parent, trashed)) = parents.get(&id) else {
                return true;
            };
            if *trashed {
                return false;
            }
            match parent {
                None => return true,
                Some(parent) => id = parent.clone(),
            }
        }
        panic!("the fixture has a parent cycle at {start}")
    };
    lines
        .iter()
        .enumerate()
        .map(|(line, page)| {
            let id = page["id"].as_str().unwrap().to_owned();
            Page {
                created_at: created_at + line as u64 * 1000,
                title: page["title"].as_str().unwrap().to_owned(),
                active: active(&id),
                blocks: page["body"]
                    .as_str()
                    .unwrap()
                    .split('\n')
                    .enumerate()
                    .map(|(i, text)| (format!("{id}-{i}"), text.to_owned()))
                    .collect(),
                id,
            }
        })
        .collect()
}

/// `page-5000-blocks.json` as one page, its blocks flattened parents first.
pub(crate) fn big_page(id: &str, title: &str, created_at: u64) -> Page {
    fn flatten(blocks: &serde_json::Value, out: &mut Vec<(String, String)>) {
        for block in blocks.as_array().into_iter().flatten() {
            let mut text = String::new();
            for inline in block["content"].as_array().into_iter().flatten() {
                match inline["type"].as_str() {
                    Some("link") => {
                        for run in inline["content"].as_array().into_iter().flatten() {
                            text.push_str(run["text"].as_str().unwrap());
                        }
                    }
                    _ => text.push_str(inline["text"].as_str().unwrap()),
                }
            }
            if block["type"] == "image" {
                text.push_str(block["props"]["caption"].as_str().unwrap_or_default());
            }
            out.push((block["id"].as_str().unwrap().to_owned(), text));
            flatten(&block["children"], out);
        }
    }
    let json: serde_json::Value =
        serde_json::from_str(&generated("page-5000-blocks.json")).unwrap();
    let mut blocks = Vec::new();
    flatten(&json, &mut blocks);
    Page {
        id: id.to_owned(),
        created_at,
        title: title.to_owned(),
        active: true,
        blocks,
    }
}

/// Maximal runs of ASCII letters and digits, lowercased, with their byte ranges.
fn tokens(text: &str) -> Vec<(String, usize, usize)> {
    assert!(text.is_ascii(), "the oracle reads ASCII only: {text:?}");
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_alphanumeric() {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_alphanumeric() {
            at += 1;
        }
        out.push((text[start..at].to_ascii_lowercase(), start, at));
    }
    out
}

/// Each whitespace word with a letter or digit is a phrase; the last word's last token is a
/// prefix; words without tokens are dropped.
fn phrases(query: &str) -> Vec<(Vec<String>, bool)> {
    let words: Vec<&str> = query
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .collect();
    words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            (
                tokens(word).into_iter().map(|t| t.0).collect::<Vec<_>>(),
                i + 1 == words.len(),
            )
        })
        .filter(|(terms, _)| !terms.is_empty())
        .collect()
}

/// The highlighted parts of `text`, or `None` if some phrase does not occur.
fn highlight(text: &str, phrases: &[(Vec<String>, bool)]) -> Option<Parts> {
    if phrases.is_empty() {
        return None;
    }
    let tokens = tokens(text);
    let mut spans = Vec::new();
    for (terms, prefix) in phrases {
        let n = terms.len();
        let mut found = false;
        for at in 0..tokens.len() {
            if at + n > tokens.len() {
                break;
            }
            let ok = (0..n).all(|k| {
                let token = &tokens[at + k].0;
                if *prefix && k + 1 == n {
                    token.starts_with(&terms[k])
                } else {
                    *token == terms[k]
                }
            });
            if ok {
                spans.push((at, at + n - 1));
                found = true;
            }
        }
        if !found {
            return None;
        }
    }
    spans.sort();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in spans {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    let mut parts = Vec::new();
    let mut at = 0;
    for (s, e) in merged {
        let (start, end) = (tokens[s].1, tokens[e].2);
        if start > at {
            parts.push((text[at..start].to_owned(), false));
        }
        parts.push((text[start..end].to_owned(), true));
        at = end;
    }
    if at < text.len() {
        parts.push((text[at..].to_owned(), false));
    }
    Some(parts)
}

fn plain(text: &str) -> Parts {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![(text.to_owned(), false)]
    }
}

pub(crate) enum Scope<'a> {
    All,
    Content(&'a HashSet<String>),
}

/// What a search must return, and how many block matches a walk that stops after the last
/// returned page reads.
pub(crate) fn expected(
    pages: &[Page],
    query: &str,
    limit: usize,
    scope: Scope,
) -> (Vec<Hit>, usize) {
    let phrases = phrases(query);
    let mut order: Vec<&Page> = pages.iter().filter(|p| p.active).collect();
    order.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    let mut hits: Vec<Hit> = Vec::new();
    let mut titled: HashSet<&str> = HashSet::new();
    if phrases.is_empty() {
        return (hits, 0);
    }
    if let Scope::All = scope {
        for page in &order {
            if hits.len() == limit {
                return (hits, 0);
            }
            if let Some(parts) = highlight(&page.title, &phrases) {
                hits.push((page.id.clone(), parts, None));
                titled.insert(&page.id);
            }
        }
    }
    let excluded = |id: &str| match scope {
        Scope::All => titled.contains(id),
        Scope::Content(exclude) => exclude.contains(id),
    };
    let mut walked = 0;
    for page in &order {
        let matching: Vec<(usize, Parts)> = page
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(i, (_, text))| highlight(text, &phrases).map(|parts| (i, parts)))
            .collect();
        if matching.is_empty() {
            continue;
        }
        if hits.len() == limit {
            return (hits, walked + 1);
        }
        walked += matching.len();
        if excluded(&page.id) {
            continue;
        }
        let (first, parts) = matching.into_iter().next().unwrap();
        let title = highlight(&page.title, &phrases).unwrap_or_else(|| plain(&page.title));
        hits.push((
            page.id.clone(),
            title,
            Some((page.blocks[first].0.clone(), parts)),
        ));
    }
    (hits, walked)
}
