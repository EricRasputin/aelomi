use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, params};

use super::{Backlink, BacklinkBlock, BlockHit, SearchHit, TextPart};
use crate::workspace::body::BlockId;
use crate::workspace::records::PageId;

pub(crate) fn match_expression(query: &str) -> Option<String> {
    let words: Vec<&str> = query
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .collect();
    let (last, rest) = words.split_last()?;
    let quote = |word: &str| format!("\"{}\"", word.replace('"', "\"\""));
    let mut expression: Vec<String> = rest.iter().map(|word| quote(word)).collect();
    expression.push(format!("{}*", quote(last)));
    Some(expression.join(" "))
}

struct Candidate {
    rank: f64,
    title_rowid: Option<i64>,
    block: Option<(f64, i64, i64, String)>,
}

pub(crate) fn search(
    conn: &mut Connection,
    expression: &str,
    limit: usize,
) -> rusqlite::Result<Vec<SearchHit>> {
    let tx = conn.transaction()?;
    let mut pages: BTreeMap<String, Candidate> = BTreeMap::new();
    {
        let mut titles = tx.prepare_cached(
            "SELECT p.page_id, p.rowid, bm25(title_fts) FROM title_fts
             JOIN pages p ON p.rowid = title_fts.rowid
             WHERE title_fts MATCH ?1 AND p.active = 1",
        )?;
        let rows = titles.query_map([expression], |row| {
            Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?))
        })?;
        for row in rows {
            let (page, rowid, rank) = row?;
            pages.insert(
                page,
                Candidate {
                    rank,
                    title_rowid: Some(rowid),
                    block: None,
                },
            );
        }
        let mut blocks = tx.prepare_cached(
            "SELECT b.page_id, b.rowid, b.ordinal, b.block_id, bm25(block_fts) FROM block_fts
             JOIN blocks b ON b.rowid = block_fts.rowid
             JOIN pages p ON p.page_id = b.page_id
             WHERE block_fts MATCH ?1 AND p.active = 1",
        )?;
        let rows = blocks.query_map([expression], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })?;
        for row in rows {
            let (page, rowid, ordinal, block_id, rank) = row?;
            let candidate = pages.entry(page).or_insert(Candidate {
                rank,
                title_rowid: None,
                block: None,
            });
            candidate.rank = candidate.rank.min(rank);
            let better = candidate
                .block
                .as_ref()
                .is_none_or(|(r, o, ..)| (rank, ordinal) < (*r, *o));
            if better {
                candidate.block = Some((rank, ordinal, rowid, block_id));
            }
        }
    }
    let mut ranked: Vec<(String, Candidate)> = pages.into_iter().collect();
    ranked.sort_by(|(a, x), (b, y)| x.rank.total_cmp(&y.rank).then_with(|| a.cmp(b)));
    ranked.truncate(limit);
    let mut hits = Vec::with_capacity(ranked.len());
    for (page, candidate) in ranked {
        let title = match candidate.title_rowid {
            Some(rowid) => highlight(&tx, "title_fts", expression, rowid)?,
            None => plain(
                tx.prepare_cached("SELECT title FROM pages WHERE page_id = ?1")?
                    .query_row([&page], |row| row.get(0))?,
            ),
        };
        let block = match candidate.block {
            Some((_, _, rowid, block_id)) => Some(BlockHit {
                block_id: BlockId::new(block_id).expect("stored block IDs are not empty"),
                text: highlight(&tx, "block_fts", expression, rowid)?,
            }),
            None => None,
        };
        hits.push(SearchHit {
            page_id: PageId::new(page).expect("stored page IDs are valid"),
            title,
            block,
        });
    }
    tx.finish()?;
    Ok(hits)
}

const MARKERS: [(char, char); 4] = [
    ('\u{E000}', '\u{E001}'),
    ('\u{E002}', '\u{E003}'),
    ('\u{F8FE}', '\u{F8FF}'),
    ('\u{10FFFD}', '\u{10FFFC}'),
];

fn highlight(
    tx: &rusqlite::Transaction,
    table: &str,
    expression: &str,
    rowid: i64,
) -> rusqlite::Result<Vec<TextPart>> {
    let column = if table == "title_fts" {
        "title"
    } else {
        "text"
    };
    let mut read = tx.prepare_cached(&format!(
        "SELECT {column}, highlight({table}, 0, ?3, ?4) FROM {table}
         WHERE {table} MATCH ?1 AND rowid = ?2"
    ))?;
    let mut text: Option<String> = None;
    for (open, close) in MARKERS {
        let (plain, marked): (String, String) = read
            .query_row(
                params![expression, rowid, open.to_string(), close.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .unwrap_or_default();
        if !plain.contains(open) && !plain.contains(close) {
            return Ok(parts(&marked, open, close));
        }
        text = Some(plain);
    }
    Ok(self::plain(text.unwrap_or_default()))
}

fn parts(marked: &str, open: char, close: char) -> Vec<TextPart> {
    let mut parts = Vec::new();
    let mut highlighted = false;
    let mut current = String::new();
    for c in marked.chars() {
        if c == open || c == close {
            if !current.is_empty() {
                parts.push(TextPart {
                    text: std::mem::take(&mut current),
                    highlighted,
                });
            }
            highlighted = c == open;
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        parts.push(TextPart {
            text: current,
            highlighted,
        });
    }
    parts
}

fn plain(text: String) -> Vec<TextPart> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![TextPart {
        text,
        highlighted: false,
    }]
}

pub(crate) fn backlinks(conn: &mut Connection, target: &PageId) -> rusqlite::Result<Vec<Backlink>> {
    let tx = conn.transaction()?;
    let mut backlinks: Vec<Backlink> = Vec::new();
    {
        let mut rows = tx.prepare_cached(
            "SELECT l.source_id, p.title, b.block_id, b.text FROM links l
             JOIN blocks b ON b.rowid = l.block_rowid
             JOIN pages p ON p.page_id = l.source_id AND p.active = 1
             WHERE l.target_id = ?1
               AND EXISTS (SELECT 1 FROM pages t WHERE t.page_id = ?1 AND t.active = 1)
             ORDER BY p.updated_at DESC, l.source_id, b.ordinal, b.rowid",
        )?;
        let rows = rows.query_map([target.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (source, title, block_id, text) = row?;
            let block = BacklinkBlock {
                block_id: BlockId::new(block_id).expect("stored block IDs are not empty"),
                text,
            };
            match backlinks.last_mut() {
                Some(last) if last.page_id.as_str() == source => last.blocks.push(block),
                _ => backlinks.push(Backlink {
                    page_id: PageId::new(source).expect("stored page IDs are valid"),
                    title,
                    blocks: vec![block],
                }),
            }
        }
    }
    tx.finish()?;
    Ok(backlinks)
}
