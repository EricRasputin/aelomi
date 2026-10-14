use std::ffi::{c_char, c_int, c_void};
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;

use rusqlite::{Connection, ffi};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub term: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum Mode {
    Query,
    Document,
}

#[derive(Debug)]
pub(crate) struct TokenizerError(pub String);

pub(crate) struct Tokenizer<'conn> {
    table: ffi::fts5_tokenizer,
    handle: *mut ffi::Fts5Tokenizer,
    _conn: PhantomData<&'conn Connection>,
}

struct Statement(*mut ffi::sqlite3_stmt);

impl Drop for Statement {
    fn drop(&mut self) {
        unsafe { ffi::sqlite3_finalize(self.0) };
    }
}

fn error(what: &str, code: c_int) -> TokenizerError {
    TokenizerError(format!("{what} failed with SQLite code {code}"))
}

impl<'conn> Tokenizer<'conn> {
    pub fn new(conn: &'conn Connection) -> Result<Self, TokenizerError> {
        let api = fts5_api(conn)?;
        let mut table = ffi::fts5_tokenizer {
            xCreate: None,
            xDelete: None,
            xTokenize: None,
        };
        let mut user_data: *mut c_void = ptr::null_mut();
        let find = unsafe { (*api).xFindTokenizer }
            .ok_or_else(|| TokenizerError("FTS5 has no xFindTokenizer".into()))?;
        let code = unsafe { find(api, c"unicode61".as_ptr(), &mut user_data, &mut table) };
        if code != ffi::SQLITE_OK {
            return Err(error("finding unicode61", code));
        }
        let (Some(create), Some(_), Some(_)) = (table.xCreate, table.xDelete, table.xTokenize)
        else {
            return Err(TokenizerError("unicode61 lacks a tokenizer method".into()));
        };
        let mut args = [c"remove_diacritics".as_ptr(), c"2".as_ptr()];
        let mut handle = ptr::null_mut();
        let code = unsafe { create(user_data, args.as_mut_ptr(), 2, &mut handle) };
        if code != ffi::SQLITE_OK || handle.is_null() {
            return Err(error("creating unicode61", code));
        }
        Ok(Self {
            table,
            handle,
            _conn: PhantomData,
        })
    }

    pub fn tokens(&self, text: &str, mode: Mode) -> Result<Vec<Token>, TokenizerError> {
        let len = c_int::try_from(text.len())
            .map_err(|_| TokenizerError("the text is too long to tokenize".into()))?;
        let tokenize = self
            .table
            .xTokenize
            .ok_or_else(|| TokenizerError("unicode61 has no xTokenize".into()))?;
        let mut sink = Sink {
            text,
            tokens: Vec::new(),
            error: None,
        };
        let flags = match mode {
            Mode::Query => ffi::FTS5_TOKENIZE_QUERY,
            Mode::Document => ffi::FTS5_TOKENIZE_DOCUMENT,
        };
        let code = unsafe {
            tokenize(
                self.handle,
                (&mut sink as *mut Sink).cast(),
                flags,
                text.as_ptr().cast(),
                len,
                Some(collect),
            )
        };
        if let Some(error) = sink.error {
            return Err(error);
        }
        if code != ffi::SQLITE_OK {
            return Err(error("tokenizing", code));
        }
        Ok(sink.tokens)
    }
}

impl Drop for Tokenizer<'_> {
    fn drop(&mut self) {
        if let Some(delete) = self.table.xDelete {
            unsafe { delete(self.handle) };
        }
    }
}

/// FTS5's API table, which lives as long as the connection.
fn fts5_api(conn: &Connection) -> Result<*mut ffi::fts5_api, TokenizerError> {
    let db = unsafe { conn.handle() };
    let mut raw = ptr::null_mut();
    let code = unsafe {
        ffi::sqlite3_prepare_v2(
            db,
            c"SELECT fts5(?1)".as_ptr(),
            -1,
            &mut raw,
            ptr::null_mut(),
        )
    };
    let statement = Statement(raw);
    if code != ffi::SQLITE_OK {
        return Err(error("preparing fts5()", code));
    }
    let mut api: *mut ffi::fts5_api = ptr::null_mut();
    let code = unsafe {
        ffi::sqlite3_bind_pointer(
            statement.0,
            1,
            (&mut api as *mut *mut ffi::fts5_api).cast(),
            c"fts5_api_ptr".as_ptr(),
            None,
        )
    };
    if code != ffi::SQLITE_OK {
        return Err(error("binding fts5_api_ptr", code));
    }
    let code = unsafe { ffi::sqlite3_step(statement.0) };
    if code != ffi::SQLITE_ROW {
        return Err(error("reading fts5()", code));
    }
    drop(statement);
    if api.is_null() {
        return Err(TokenizerError("FTS5 returned no API".into()));
    }
    Ok(api)
}

struct Sink<'t> {
    text: &'t str,
    tokens: Vec<Token>,
    error: Option<TokenizerError>,
}

impl Sink<'_> {
    fn push(
        &mut self,
        token: *const c_char,
        len: c_int,
        start: c_int,
        end: c_int,
    ) -> Result<(), TokenizerError> {
        let invalid = |what: &str| TokenizerError(format!("unicode61 returned {what}"));
        let len = usize::try_from(len).map_err(|_| invalid("a negative token length"))?;
        let start = usize::try_from(start).map_err(|_| invalid("a negative token start"))?;
        let end = usize::try_from(end).map_err(|_| invalid("a negative token end"))?;
        if start > end || end > self.text.len() {
            return Err(invalid("a token outside the text"));
        }
        if !self.text.is_char_boundary(start) || !self.text.is_char_boundary(end) {
            return Err(invalid("a token inside a character"));
        }
        let bytes: &[u8] = if len == 0 {
            &[]
        } else if token.is_null() {
            return Err(invalid("a null token"));
        } else {
            unsafe { std::slice::from_raw_parts(token.cast(), len) }
        };
        let term = std::str::from_utf8(bytes)
            .map_err(|_| invalid("a token that is not UTF-8"))?
            .to_owned();
        self.tokens.push(Token { term, start, end });
        Ok(())
    }
}

unsafe extern "C" fn collect(
    ctx: *mut c_void,
    _flags: c_int,
    token: *const c_char,
    len: c_int,
    start: c_int,
    end: c_int,
) -> c_int {
    let sink = ctx.cast::<Sink>();
    let pushed = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        (*sink).push(token, len, start, end)
    }));
    let failure = match pushed {
        Ok(Ok(())) => return ffi::SQLITE_OK,
        Ok(Err(error)) => error,
        Err(_) => TokenizerError("collecting a token panicked".into()),
    };
    let _ = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        (*sink).error.get_or_insert(failure);
    }));
    ffi::SQLITE_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call<'t>(
        text: &'t str,
        token: &[u8],
        len: c_int,
        start: c_int,
        end: c_int,
    ) -> (c_int, Sink<'t>) {
        let mut sink = Sink {
            text,
            tokens: Vec::new(),
            error: None,
        };
        let code = unsafe {
            collect(
                (&mut sink as *mut Sink).cast(),
                0,
                token.as_ptr().cast(),
                len,
                start,
                end,
            )
        };
        (code, sink)
    }

    #[test]
    fn an_invalid_token_stops_tokenizing_with_an_error() {
        let (code, sink) = call("héllo", b"hello", 5, 0, 6);
        assert_eq!(
            (code, sink.tokens.len(), sink.error.is_none()),
            (ffi::SQLITE_OK, 1, true)
        );
        for (len, start, end, why) in [
            (-1, 0, 6, "a negative token length"),
            (5, -1, 6, "a negative token start"),
            (5, 0, 7, "a token outside the text"),
            (5, 3, 2, "a token outside the text"),
            (5, 0, 2, "a token inside a character"),
        ] {
            let (code, sink) = call("héllo", b"hello", len, start, end);
            assert_eq!(code, ffi::SQLITE_ERROR, "{why}");
            assert_eq!(
                sink.error.map(|e| e.0),
                Some(format!("unicode61 returned {why}"))
            );
            assert!(sink.tokens.is_empty());
        }
        let (code, sink) = call("ab", &[0xff, 0xfe], 2, 0, 2);
        assert_eq!(code, ffi::SQLITE_ERROR);
        assert_eq!(
            sink.error.map(|e| e.0),
            Some("unicode61 returned a token that is not UTF-8".to_owned())
        );
    }
}
