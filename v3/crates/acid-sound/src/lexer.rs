//! .snd tokens. Lines matter (a statement ends at a newline or `;`), so
//! the lexer emits a Newline token at the end of every line.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl CompileError {
    pub fn new(line: u32, col: u32, message: impl Into<String>) -> Self {
        Self { line, col, message: message.into() }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} {}", self.line, self.col, self.message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Num(i32),
    Str(String),
    Sym(&'static str),
    Newline,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    pub col: u32,
}

/// Longest first, so "<=" wins over "<".
const SYMS: [&str; 15] = ["==", "!=", "<=", ">=", "<", ">", "=", "+", "-", "*", "/", "%", "(", ")", ";"];

pub fn lex(src: &str) -> Result<Vec<Token>, CompileError> {
    let mut out = Vec::new();
    for (li, text) in src.lines().enumerate() {
        let line = li as u32 + 1;
        let b = text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let col = i as u32 + 1;
            if c == b' ' || c == b'\t' {
                i += 1;
                continue;
            }
            if c == b'#' {
                break;
            }
            if c.is_ascii_digit() {
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                let n = text[start..i].parse().map_err(|_| CompileError::new(line, col, "number too big"))?;
                out.push(Token { tok: Tok::Num(n), line, col });
                continue;
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push(Token { tok: Tok::Ident(text[start..i].to_string()), line, col });
                continue;
            }
            if c == b'"' {
                let start = i + 1;
                let end = text[start..]
                    .find('"')
                    .ok_or_else(|| CompileError::new(line, col, "string has no closing '\"'"))?;
                out.push(Token { tok: Tok::Str(text[start..start + end].to_string()), line, col });
                i = start + end + 1;
                continue;
            }
            let Some(&s) = SYMS.iter().find(|s| text[i..].starts_with(**s)) else {
                let ch = text[i..].chars().next().unwrap_or('?');
                return Err(CompileError::new(line, col, format!("unexpected '{ch}'")));
            };
            out.push(Token { tok: if s == ";" { Tok::Newline } else { Tok::Sym(s) }, line, col });
            i += s.len();
        }
        out.push(Token { tok: Tok::Newline, line, col: b.len() as u32 + 1 });
    }
    let line = out.last().map_or(1, |t| t.line);
    out.push(Token { tok: Tok::Eof, line, col: 1 });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    fn id(s: &str) -> Tok {
        Tok::Ident(s.into())
    }

    #[test]
    fn words_numbers_and_symbols() {
        assert_eq!(toks("duty +8"), vec![id("duty"), Tok::Sym("+"), Tok::Num(8), Tok::Newline, Tok::Eof]);
    }

    #[test]
    fn two_character_symbols_win() {
        assert_eq!(toks("a<=b")[1], Tok::Sym("<="));
        assert_eq!(toks("a == b")[1], Tok::Sym("=="));
        assert_eq!(toks("a = b")[1], Tok::Sym("="));
    }

    #[test]
    fn semicolons_split_and_hashes_comment() {
        assert_eq!(
            toks("gate on; wait 1 # done"),
            vec![id("gate"), id("on"), Tok::Newline, id("wait"), Tok::Num(1), Tok::Newline, Tok::Eof]
        );
    }

    #[test]
    fn strings_keep_their_text() {
        assert_eq!(toks("song \"music/a.trk\"")[1], Tok::Str("music/a.trk".into()));
    }

    #[test]
    fn positions_are_one_based() {
        let t = lex("wave saw\n  gate on").unwrap();
        assert_eq!((t[3].line, t[3].col), (2, 3));
    }

    #[test]
    fn errors_carry_line_and_column() {
        assert_eq!(lex("wait 1\n  @").unwrap_err().to_string(), "2:3 unexpected '@'");
        assert_eq!(lex("wait 99999999999").unwrap_err().to_string(), "1:6 number too big");
        assert_eq!(lex("song \"x").unwrap_err().to_string(), "1:6 string has no closing '\"'");
    }
}
