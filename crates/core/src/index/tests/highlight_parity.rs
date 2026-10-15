use rusqlite::Connection;

use super::search_oracle;
use crate::index::TextPart;
use crate::index::highlight::{Phrases, match_expression, words};
use crate::index::tokenizer::Tokenizer;

const MARKERS: [(char, char); 4] = [
    ('\u{E000}', '\u{E001}'),
    ('\u{E002}', '\u{E003}'),
    ('\u{F8FE}', '\u{F8FF}'),
    ('\u{10FFFD}', '\u{10FFFC}'),
];

fn sqlite_parts(conn: &Connection, expression: &str, rowid: i64, text: &str) -> Vec<TextPart> {
    let (open, close) = MARKERS
        .into_iter()
        .find(|(o, c)| !text.contains(*o) && !text.contains(*c))
        .expect("a free marker pair");
    let marked: String = conn
        .query_row(
            "SELECT highlight(t, 0, ?2, ?3) FROM t WHERE t MATCH ?1 AND rowid = ?4",
            rusqlite::params![expression, open.to_string(), close.to_string(), rowid],
            |row| row.get(0),
        )
        .unwrap();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut highlighted = false;
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

fn assert_parity(texts: &[String], queries: &[&str]) -> usize {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE t USING fts5 (x, tokenize = 'unicode61 remove_diacritics 2',
           prefix = '1 2 3');",
    )
    .unwrap();
    for (i, text) in texts.iter().enumerate() {
        conn.execute(
            "INSERT INTO t (rowid, x) VALUES (?1, ?2)",
            rusqlite::params![i as i64 + 1, text],
        )
        .unwrap();
    }
    let tokenizer = Tokenizer::new(&conn).unwrap();
    let mut compared = 0;
    for query in queries {
        let words = words(query);
        let Some(expression) = match_expression(&words) else {
            continue;
        };
        let matched: Vec<i64> = conn
            .prepare("SELECT rowid FROM t WHERE t MATCH ?1 ORDER BY rowid")
            .unwrap()
            .query_map([&expression], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let phrases = Phrases::compile(&tokenizer, &words).unwrap();
        for (i, text) in texts.iter().enumerate() {
            let rowid = i as i64 + 1;
            let ours = phrases.highlight(&tokenizer, text).unwrap();
            if matched.contains(&rowid) {
                assert_eq!(
                    ours,
                    Some(sqlite_parts(&conn, &expression, rowid, text)),
                    "{query:?} on {text:?}"
                );
                compared += 1;
            } else {
                assert_eq!(ours, None, "{query:?} does not match {text:?} in SQLite");
            }
        }
    }
    compared
}

#[test]
fn highlights_equal_sqlite_highlight_on_unicode_punctuation_and_marker_text() {
    let texts: Vec<String> = [
        "Café crème brûlée",
        "CAFÉ society",
        "naïve résumé of a cafe",
        "decomposed cafe\u{301} here",
        "don't stop, don’t",
        "foo-bar baz foo bar",
        "\u{E000}literal\u{E001} markers \u{F8FF}stay",
        "<b>html</b> &amp; text",
        "日本語のテキスト と 日本",
        "emoji 😀 text 😀😀",
        "aa aa aa aab",
        "ab abc abcd ab",
        "snake_case_word and v1.2.3",
        "tab\tseparated\nnew line",
        "Ⅷ roman and ½ half",
        "ﬁne ligature",
        "İstanbul ıi",
        "x y z x y",
        "",
        "   ",
        "ա Armenian աբգ",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let queries = [
        "cafe",
        "Café",
        "creme brul",
        "naive resume",
        "don't",
        "don",
        "foo-ba",
        "foo bar",
        "literal",
        "markers stay",
        "html amp",
        "日本",
        "テキ",
        "😀",
        "aa aa",
        "aa",
        "ab abc",
        "a",
        "snake",
        "v1",
        "1.2",
        "tab separ",
        "viii",
        "fine",
        "istanbul",
        "x y",
        "x y z x",
        "\"quoted\" stop",
        "a*b",
        "ա",
        "ा",
        "go ा",
        "ा go",
    ];
    let compared = assert_parity(&texts, &queries);
    assert!(compared > 25, "{compared} highlights compared");
}

#[test]
fn highlights_equal_sqlite_highlight_on_every_block_of_the_5000_block_page() {
    let page = search_oracle::big_page("big", "Big", 0);
    let texts: Vec<String> = page.blocks.into_iter().map(|(_, text)| text).collect();
    let compared = assert_parity(
        &texts,
        &[
            "je",
            "to",
            "waje",
            "s",
            "l",
            "je to",
            "nogujo hiseji",
            "pirufa wa",
        ],
    );
    assert!(compared > 5_000, "{compared} highlights compared");
}

#[test]
fn the_tokenizer_returns_folded_terms_with_byte_ranges() {
    let conn = Connection::open_in_memory().unwrap();
    let tokenizer = Tokenizer::new(&conn).unwrap();
    let tokens = tokenizer
        .tokens("Ünïcode words", crate::index::tokenizer::Mode::Document)
        .unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|t| (t.term.as_str(), t.start, t.end))
            .collect::<Vec<_>>(),
        [("unicode", 0, 9), ("words", 10, 15)]
    );
}
