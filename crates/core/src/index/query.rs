use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Transaction};

use super::db::{Corrupt, PageKey, stored_block_id};
use super::highlight::{Phrases, parts};
use super::tokenizer::{Tokenizer, TokenizerError};
use super::{Backlink, BacklinkBlock, BlockHit, SearchHit, TextPart};
use crate::workspace::records::PageId;

#[derive(Debug)]
pub(crate) enum QueryFault {
    Sql(rusqlite::Error),
    Corrupt(Corrupt),
    Tokenizer(TokenizerError),
}

impl From<rusqlite::Error> for QueryFault {
    fn from(e: rusqlite::Error) -> Self {
        match Corrupt::from_row_error(&e) {
            Some(what) => what.into(),
            None => Self::Sql(e),
        }
    }
}

impl From<Corrupt> for QueryFault {
    fn from(what: Corrupt) -> Self {
        Self::Corrupt(what)
    }
}

impl From<TokenizerError> for QueryFault {
    fn from(e: TokenizerError) -> Self {
        Self::Tokenizer(e)
    }
}

pub(crate) struct Plan<'a> {
    pub expression: &'a str,
    pub words: &'a [&'a str],
    pub limit: usize,
    pub scope: Scope<'a>,
}

pub(crate) enum Scope<'a> {
    All,
    Content { exclude: &'a [&'a PageId] },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Walk {
    pub title_rows: usize,
    pub block_rows: usize,
}

struct Winner {
    page: PageKey,
    block: Option<i64>,
}

pub(crate) fn search(
    conn: &mut Connection,
    plan: &Plan,
) -> Result<(Vec<SearchHit>, Walk), QueryFault> {
    let tx = conn.transaction()?;
    let mut walk = Walk::default();
    let mut winners: Vec<Winner> = Vec::new();
    let mut skip: HashSet<PageKey> = HashSet::new();
    match plan.scope {
        Scope::All => {
            let mut titles = tx.prepare_cached(
                "SELECT rowid FROM title_fts WHERE title_fts MATCH ?1 ORDER BY rowid DESC",
            )?;
            let mut rows = titles.query([plan.expression])?;
            while winners.len() < plan.limit
                && let Some(row) = rows.next()?
            {
                walk.title_rows += 1;
                let page = PageKey::new(row.get(0)?).ok_or(Corrupt::Key)?;
                winners.push(Winner { page, block: None });
            }
            if winners.len() == plan.limit {
                drop(rows);
                drop(titles);
                return finish(tx, plan, winners, walk);
            }
            skip.extend(winners.iter().map(|w| w.page));
        }
        Scope::Content { exclude } => {
            let mut find = tx.prepare_cached("SELECT rowid FROM pages WHERE page_id = ?1")?;
            for id in exclude {
                if let Some(key) = find
                    .query_row([id.as_str()], |row| row.get::<_, i64>(0))
                    .optional()?
                {
                    skip.insert(PageKey::new(key).ok_or(Corrupt::Key)?);
                }
            }
        }
    }
    {
        let mut blocks = tx.prepare_cached(
            "SELECT f.rowid, b.ordinal FROM block_fts f LEFT JOIN blocks b ON b.rowid = f.rowid
             WHERE block_fts MATCH ?1 ORDER BY f.rowid DESC",
        )?;
        let mut rows = blocks.query([plan.expression])?;
        let mut group: Option<(PageKey, Option<(i64, i64)>)> = None;
        loop {
            let row = rows.next()?;
            let next = match row {
                Some(row) => {
                    walk.block_rows += 1;
                    let rowid: i64 = row.get(0)?;
                    let ordinal = row.get::<_, Option<i64>>(1)?.ok_or(Corrupt::MissingRow)?;
                    if ordinal < 0 {
                        return Err(Corrupt::Ordinal.into());
                    }
                    Some((PageKey::of_block(rowid)?, rowid, ordinal))
                }
                None => None,
            };
            let closes = match (&group, &next) {
                (Some((open, _)), Some((page, ..))) => open != page,
                (Some(_), None) => true,
                (None, _) => false,
            };
            if closes && let Some((page, Some((_, rowid)))) = group.take() {
                winners.push(Winner {
                    page,
                    block: Some(rowid),
                });
                if winners.len() == plan.limit {
                    break;
                }
            }
            let Some((page, rowid, ordinal)) = next else {
                break;
            };
            let (_, best) = group.get_or_insert((page, None));
            if !skip.contains(&page) && best.is_none_or(|(o, _)| ordinal < o) {
                *best = Some((ordinal, rowid));
            }
        }
    }
    finish(tx, plan, winners, walk)
}

fn finish(
    tx: Transaction,
    plan: &Plan,
    winners: Vec<Winner>,
    walk: Walk,
) -> Result<(Vec<SearchHit>, Walk), QueryFault> {
    let mut hits = Vec::with_capacity(winners.len());
    {
        let tokenizer = Tokenizer::new(&tx)?;
        let phrases = Phrases::compile(&tokenizer, plan.words)?;
        let mut page =
            tx.prepare_cached("SELECT page_id, title, active FROM pages WHERE rowid = ?1")?;
        let mut block = tx.prepare_cached(
            "SELECT b.block_id, t.text FROM blocks b JOIN block_text t ON t.rowid = b.rowid
             WHERE b.rowid = ?1",
        )?;
        for winner in winners {
            let (id, title, active): (String, String, i64) = page
                .query_row([winner.page.get()], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .optional()?
                .ok_or(Corrupt::MissingRow)?;
            if active != 1 {
                return Err(Corrupt::Field.into());
            }
            let page_id = PageId::new(id).map_err(|_| Corrupt::PageId)?;
            let title = phrases
                .highlight(&tokenizer, &title)?
                .unwrap_or_else(|| parts(&title, []));
            let block = match winner.block {
                None => None,
                Some(rowid) => {
                    let (block_id, text): (String, String) = block
                        .query_row([rowid], |row| Ok((row.get(0)?, row.get(1)?)))
                        .optional()?
                        .ok_or(Corrupt::MissingRow)?;
                    let text: Vec<TextPart> = phrases
                        .highlight(&tokenizer, &text)?
                        .unwrap_or_else(|| parts(&text, []));
                    Some(BlockHit {
                        block_id: stored_block_id(block_id)?,
                        text,
                    })
                }
            };
            hits.push(SearchHit {
                page_id,
                title,
                block,
            });
        }
    }
    tx.finish()?;
    Ok((hits, walk))
}

pub(crate) fn backlinks(
    conn: &mut Connection,
    target: &PageId,
) -> Result<Vec<Backlink>, QueryFault> {
    let tx = conn.transaction()?;
    let mut backlinks: Vec<Backlink> = Vec::new();
    {
        let mut rows = tx.prepare_cached(
            "SELECT p.page_id, p.title, p.active, b.block_id, t.text FROM links l
             LEFT JOIN pages p ON p.rowid = (l.block_rowid >> 20)
             LEFT JOIN blocks b ON b.rowid = l.block_rowid
             LEFT JOIN block_text t ON t.rowid = l.block_rowid
             WHERE l.target_id = ?1
               AND EXISTS (SELECT 1 FROM pages target WHERE target.page_id = ?1 AND target.active = 1)
             ORDER BY p.updated_at DESC, p.page_id, b.ordinal, b.rowid",
        )?;
        let mut rows = rows.query([target.as_str()])?;
        while let Some(row) = rows.next()? {
            let source: String = row
                .get::<_, Option<String>>(0)?
                .ok_or(Corrupt::MissingRow)?;
            if row.get::<_, i64>(2)? != 1 {
                continue;
            }
            let (Some(block_id), Some(text)) = (
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ) else {
                return Err(Corrupt::MissingRow.into());
            };
            let block = BacklinkBlock {
                block_id: stored_block_id(block_id)?,
                text,
            };
            match backlinks.last_mut() {
                Some(last) if last.page_id.as_str() == source => last.blocks.push(block),
                _ => backlinks.push(Backlink {
                    page_id: PageId::new(source).map_err(|_| Corrupt::PageId)?,
                    title: row.get(1)?,
                    blocks: vec![block],
                }),
            }
        }
    }
    tx.finish()?;
    Ok(backlinks)
}
