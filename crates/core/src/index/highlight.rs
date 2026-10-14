//! Highlights as FTS5's `highlight()` marks them, computed from SQLite's tokenizer on the few
//! texts a search returns, so no marker character can collide with note text.

use super::TextPart;
use super::tokenizer::{Mode, Token, Tokenizer, TokenizerError};

/// The words of a query that can match: each holds a letter or digit.
pub(crate) fn words(query: &str) -> Vec<&str> {
    query
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .collect()
}

/// FTS5's expression for `words`: each word quoted as a phrase, all required, the last a prefix.
pub(crate) fn match_expression(words: &[&str]) -> Option<String> {
    let (last, rest) = words.split_last()?;
    let quote = |word: &str| format!("\"{}\"", word.replace('"', "\"\""));
    let mut expression: Vec<String> = rest.iter().map(|word| quote(word)).collect();
    expression.push(format!("{}*", quote(last)));
    Some(expression.join(" "))
}

/// The query's phrases as SQLite tokenizes them. A word without tokens matches nothing and
/// FTS5 drops it, so it is dropped here, after the last word took the prefix.
pub(crate) struct Phrases(Vec<Phrase>);

struct Phrase {
    terms: Vec<String>,
    prefix: bool,
}

impl Phrases {
    pub fn compile(tokenizer: &Tokenizer, words: &[&str]) -> Result<Self, TokenizerError> {
        let mut phrases = Vec::with_capacity(words.len());
        for (i, word) in words.iter().enumerate() {
            let terms: Vec<String> = tokenizer
                .tokens(word, Mode::Query)?
                .into_iter()
                .map(|t| t.term)
                .collect();
            if !terms.is_empty() {
                phrases.push(Phrase {
                    terms,
                    prefix: i + 1 == words.len(),
                });
            }
        }
        Ok(Self(phrases))
    }

    /// `text` split into parts with each phrase occurrence highlighted, overlapping occurrences
    /// merged. `None` when the text does not match: some phrase does not occur, or the query has
    /// no phrase at all.
    pub fn highlight(
        &self,
        tokenizer: &Tokenizer,
        text: &str,
    ) -> Result<Option<Vec<TextPart>>, TokenizerError> {
        if self.0.is_empty() {
            return Ok(None);
        }
        let tokens = tokenizer.tokens(text, Mode::Document)?;
        let mut found: Vec<(usize, usize)> = Vec::new();
        for phrase in &self.0 {
            let before = found.len();
            found.extend(phrase.occurrences(&tokens));
            if found.len() == before {
                return Ok(None);
            }
        }
        found.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(found.len());
        for (first, last) in found {
            match merged.last_mut() {
                Some(open) if first <= open.1 => open.1 = open.1.max(last),
                _ => merged.push((first, last)),
            }
        }
        let spans = merged
            .into_iter()
            .map(|(first, last)| (tokens[first].start, tokens[last].end));
        Ok(Some(parts(text, spans)))
    }
}

impl Phrase {
    /// Each occurrence as its first and last token index.
    fn occurrences<'a>(&'a self, tokens: &'a [Token]) -> impl Iterator<Item = (usize, usize)> + 'a {
        let n = self.terms.len();
        (0..tokens.len().saturating_sub(n - 1)).filter_map(move |at| {
            let matched = self.terms.iter().enumerate().all(|(k, term)| {
                let found = tokens[at + k].term.as_str();
                if self.prefix && k + 1 == n {
                    found.starts_with(term.as_str())
                } else {
                    found == term
                }
            });
            matched.then_some((at, at + n - 1))
        })
    }
}

/// `text` as parts, highlighting the byte ranges in `spans`, which are ordered and disjoint.
pub(crate) fn parts(text: &str, spans: impl IntoIterator<Item = (usize, usize)>) -> Vec<TextPart> {
    let mut parts = Vec::new();
    let mut at = 0;
    for (start, end) in spans {
        if start > at {
            parts.push(TextPart {
                text: text[at..start].to_owned(),
                highlighted: false,
            });
        }
        if end > start {
            parts.push(TextPart {
                text: text[start..end].to_owned(),
                highlighted: true,
            });
        }
        at = end;
    }
    if at < text.len() {
        parts.push(TextPart {
            text: text[at..].to_owned(),
            highlighted: false,
        });
    }
    parts
}
