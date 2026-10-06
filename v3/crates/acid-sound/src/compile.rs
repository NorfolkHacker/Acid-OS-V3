//! The .snd compiler: tokens straight to bytecode in one pass, no AST.
//! Command arguments that sit side by side (`adsr 2 120 60 80`) are unary
//! expressions, so `-1` is an argument rather than a subtraction; anything
//! bigger goes in parentheses.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::lexer::{lex, CompileError, Tok, Token};
use crate::program::*;

/// Words that can't be variable names.
const RESERVED: &[&str] = &[
    "let", "if", "else", "end", "repeat", "loop", "wait", "stop", "sound", "instrument", "on", "off",
    "release", "and", "or", "not", "rand", "v1", "v2", "both", "song", "play", "tempo", "mute",
    "unmute", "jump", "filter", "res", "row", "beat", "note", "note2", "tick", "order", "wave",
    "duty", "adsr", "gate", "pitch", "fine", "ring", "arp", "arprate", "route",
];

pub fn compile(src: &str) -> Result<Program, CompileError> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0, prog: Program::default() };
    p.file()?;
    Ok(p.prog)
}

fn builtin(w: &str) -> Option<Builtin> {
    Some(match w {
        "note" => Builtin::Note,
        "note2" => Builtin::Note2,
        "tick" => Builtin::Tick,
        "row" => Builtin::Row,
        "beat" => Builtin::Beat,
        "order" => Builtin::Order,
        _ => return None,
    })
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(w) => format!("'{w}'"),
        Tok::Num(n) => format!("'{n}'"),
        Tok::Str(_) => String::from("a string"),
        Tok::Sym(s) => format!("'{s}'"),
        Tok::Newline => String::from("end of line"),
        Tok::Eof => String::from("end of file"),
    }
}

/// One block being compiled.
struct Cx {
    code: Vec<Op>,
    vars: Vec<String>,
    hidden: u32,
    uses_v2: bool,
}

impl Cx {
    fn new() -> Self {
        Self { code: Vec::new(), vars: Vec::new(), hidden: 0, uses_v2: false }
    }

    fn emit(&mut self, op: Op) -> usize {
        self.code.push(op);
        self.code.len() - 1
    }

    /// The next op's index. Clamped; `finish` rejects a block this long.
    fn here(&self) -> u16 {
        self.code.len().min(u16::MAX as usize) as u16
    }

    fn patch(&mut self, at: usize, to: u16) {
        self.code[at] = match self.code[at] {
            Op::Jump(_) => Op::Jump(to),
            Op::JumpIfZero(_) => Op::JumpIfZero(to),
            op => op,
        };
    }

    fn var(&self, name: &str) -> Option<u8> {
        self.vars.iter().position(|v| v == name).map(|i| i as u8)
    }

    fn new_var(&mut self, name: String, line: u32, col: u32) -> Result<u8, CompileError> {
        if let Some(i) = self.var(&name) {
            return Ok(i);
        }
        if self.vars.len() >= MAX_VARS {
            return Err(CompileError::new(line, col, format!("too many variables ({MAX_VARS} max)")));
        }
        self.vars.push(name);
        Ok((self.vars.len() - 1) as u8)
    }

    /// A counter for `repeat`; the leading space keeps it out of reach of names.
    fn hidden_var(&mut self, line: u32, col: u32) -> Result<u8, CompileError> {
        self.hidden += 1;
        self.new_var(format!(" repeat{}", self.hidden), line, col)
    }
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    prog: Program,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn here(&self) -> (u32, u32) {
        let t = &self.toks[self.pos];
        (t.line, t.col)
    }

    fn err_here<T>(&self, msg: impl Into<String>) -> Result<T, CompileError> {
        let (l, c) = self.here();
        Err(CompileError::new(l, c, msg))
    }

    /// Takes the current token; Eof stays put.
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while *self.peek() == Tok::Newline {
            self.bump();
        }
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        let yes = self.is_word(w);
        if yes {
            self.bump();
        }
        yes
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        let yes = matches!(self.peek(), Tok::Sym(x) if *x == s);
        if yes {
            self.bump();
        }
        yes
    }

    fn at_line_end(&self) -> bool {
        matches!(self.peek(), Tok::Newline | Tok::Eof)
    }

    fn ident(&mut self, what: &str) -> Result<String, CompileError> {
        if let Tok::Ident(s) = self.peek().clone() {
            self.bump();
            Ok(s)
        } else {
            self.err_here(format!("expected {what}"))
        }
    }

    fn end_of_statement(&mut self) -> Result<(), CompileError> {
        match self.peek() {
            Tok::Newline => {
                self.bump();
                Ok(())
            }
            Tok::Eof => Ok(()),
            t => {
                let d = describe(t);
                self.err_here(format!("unexpected {d}"))
            }
        }
    }

    fn file(&mut self) -> Result<(), CompileError> {
        self.skip_newlines();
        if *self.peek() == Tok::Eof {
            return self.err_here("nothing to play");
        }
        if !self.is_word("sound") && !self.is_word("instrument") {
            let mut cx = Cx::new();
            self.body(&mut cx, &[], ("", 0, 0))?;
            return self.finish(String::from("main"), BlockKind::Sound, cx, None, (1, 1));
        }
        loop {
            self.skip_newlines();
            if *self.peek() == Tok::Eof {
                return Ok(());
            }
            let (l, c) = self.here();
            let kind = if self.eat_word("sound") {
                BlockKind::Sound
            } else if self.eat_word("instrument") {
                BlockKind::Instrument
            } else {
                return self.err_here("statement outside a block");
            };
            let opener = if kind == BlockKind::Sound { "sound" } else { "instrument" };
            let (nl, nc) = self.here();
            let name = self.ident("a name")?;
            if self.prog.blocks.iter().any(|b| b.name == name) {
                return Err(CompileError::new(nl, nc, format!("'{name}' is already defined")));
            }
            self.end_of_statement()?;
            let mut cx = Cx::new();
            let mut release = None;
            if self.body(&mut cx, &["end", "on"], (opener, l, c))? == "on" {
                let (ol, oc) = self.here();
                self.bump();
                if kind != BlockKind::Instrument {
                    return Err(CompileError::new(ol, oc, "only an instrument has 'on release'"));
                }
                if !self.eat_word("release") {
                    return self.err_here("expected 'release'");
                }
                self.end_of_statement()?;
                cx.emit(Op::End);
                release = Some(cx.here());
                self.body(&mut cx, &["end"], ("on release", ol, oc))?;
            }
            self.bump(); // end
            self.end_of_statement()?;
            self.finish(name, kind, cx, release, (l, c))?;
        }
    }

    /// Statements up to one of `terms` (not consumed). Returns that word,
    /// or "" at the end of the file when `terms` is empty.
    fn body(&mut self, cx: &mut Cx, terms: &[&'static str], opener: (&str, u32, u32)) -> Result<&'static str, CompileError> {
        loop {
            self.skip_newlines();
            if *self.peek() == Tok::Eof {
                if terms.is_empty() {
                    return Ok("");
                }
                let (w, l, c) = opener;
                return Err(CompileError::new(l, c, format!("'{w}' has no 'end'")));
            }
            if let Some(t) = terms.iter().find(|t| self.is_word(t)) {
                return Ok(t);
            }
            self.statement(cx)?;
        }
    }

    fn finish(&mut self, name: String, kind: BlockKind, mut cx: Cx, release_pc: Option<u16>, at: (u32, u32)) -> Result<(), CompileError> {
        cx.emit(Op::End);
        if cx.code.len() > u16::MAX as usize {
            return Err(CompileError::new(at.0, at.1, "script too long"));
        }
        self.prog.blocks.push(Block { name, kind, code: cx.code, release_pc, vars: cx.vars.len() as u8, uses_v2: cx.uses_v2 });
        Ok(())
    }

    fn statement(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        let (l, c) = self.here();
        let word = match self.peek().clone() {
            Tok::Ident(w) => w,
            t => return self.err_here(format!("unexpected {}", describe(&t))),
        };
        self.bump();
        match word.as_str() {
            "let" => {
                let (nl, nc) = self.here();
                let name = self.ident("a variable name")?;
                if RESERVED.contains(&name.as_str()) {
                    return Err(CompileError::new(nl, nc, format!("'{name}' is a reserved word")));
                }
                if !self.eat_sym("=") {
                    return self.err_here("expected '='");
                }
                self.expr(cx)?;
                let slot = cx.new_var(name, nl, nc)?;
                cx.emit(Op::Store(slot));
            }
            "if" => self.if_stmt(cx, l, c)?,
            "repeat" => self.repeat_stmt(cx, l, c)?,
            "loop" => {
                self.end_of_statement()?;
                let top = cx.here();
                self.body(cx, &["end"], ("loop", l, c))?;
                self.bump();
                cx.emit(Op::Jump(top));
            }
            "wait" => {
                if self.eat_word("row") {
                    cx.emit(Op::WaitRow);
                } else if self.eat_word("beat") {
                    cx.emit(Op::WaitBeat);
                } else {
                    self.expr(cx)?;
                    cx.emit(Op::Wait);
                }
            }
            "stop" => {
                cx.emit(Op::Stop);
            }
            "end" | "else" | "on" => return Err(CompileError::new(l, c, format!("unexpected '{word}'"))),
            _ => {
                if let Some(slot) = cx.var(&word) {
                    if !self.eat_sym("=") {
                        return self.err_here("expected '='");
                    }
                    self.expr(cx)?;
                    cx.emit(Op::Store(slot));
                } else if matches!(self.peek(), Tok::Sym("=")) {
                    return Err(CompileError::new(l, c, format!("unknown variable '{word}' (use 'let')")));
                } else {
                    return Err(CompileError::new(l, c, format!("unknown command '{word}'")));
                }
            }
        }
        self.end_of_statement()
    }

    fn if_stmt(&mut self, cx: &mut Cx, l: u32, c: u32) -> Result<(), CompileError> {
        self.expr(cx)?;
        self.end_of_statement()?;
        let jz = cx.emit(Op::JumpIfZero(0));
        if self.body(cx, &["else", "end"], ("if", l, c))? == "else" {
            self.bump();
            self.end_of_statement()?;
            let j = cx.emit(Op::Jump(0));
            let at = cx.here();
            cx.patch(jz, at);
            self.body(cx, &["end"], ("if", l, c))?;
            let at = cx.here();
            cx.patch(j, at);
        } else {
            let at = cx.here();
            cx.patch(jz, at);
        }
        self.bump(); // end
        Ok(())
    }

    fn repeat_stmt(&mut self, cx: &mut Cx, l: u32, c: u32) -> Result<(), CompileError> {
        self.expr(cx)?;
        self.end_of_statement()?;
        let slot = cx.hidden_var(l, c)?;
        cx.emit(Op::Store(slot));
        let top = cx.here();
        cx.emit(Op::Load(slot));
        cx.emit(Op::Push(0));
        cx.emit(Op::Bin(BinOp::Gt));
        let jz = cx.emit(Op::JumpIfZero(0));
        self.body(cx, &["end"], ("repeat", l, c))?;
        self.bump(); // end
        cx.emit(Op::Load(slot));
        cx.emit(Op::Push(1));
        cx.emit(Op::Bin(BinOp::Sub));
        cx.emit(Op::Store(slot));
        cx.emit(Op::Jump(top));
        let at = cx.here();
        cx.patch(jz, at);
        Ok(())
    }

    fn expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.and_expr(cx)?;
        while self.eat_word("or") {
            self.and_expr(cx)?;
            cx.emit(Op::Bin(BinOp::Or));
        }
        Ok(())
    }

    fn and_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.cmp_expr(cx)?;
        while self.eat_word("and") {
            self.cmp_expr(cx)?;
            cx.emit(Op::Bin(BinOp::And));
        }
        Ok(())
    }

    fn cmp_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.add_expr(cx)?;
        let ops = [("==", BinOp::Eq), ("!=", BinOp::Ne), ("<=", BinOp::Le), (">=", BinOp::Ge), ("<", BinOp::Lt), (">", BinOp::Gt)];
        for (s, op) in ops {
            if self.eat_sym(s) {
                self.add_expr(cx)?;
                cx.emit(Op::Bin(op));
                break;
            }
        }
        Ok(())
    }

    fn add_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.mul_expr(cx)?;
        loop {
            let op = if self.eat_sym("+") {
                BinOp::Add
            } else if self.eat_sym("-") {
                BinOp::Sub
            } else {
                return Ok(());
            };
            self.mul_expr(cx)?;
            cx.emit(Op::Bin(op));
        }
    }

    fn mul_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.unary(cx)?;
        loop {
            let op = if self.eat_sym("*") {
                BinOp::Mul
            } else if self.eat_sym("/") {
                BinOp::Div
            } else if self.eat_sym("%") {
                BinOp::Mod
            } else {
                return Ok(());
            };
            self.unary(cx)?;
            cx.emit(Op::Bin(op));
        }
    }

    /// Also the form of a side-by-side command argument.
    fn unary(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        if self.eat_sym("-") {
            self.unary(cx)?;
            cx.emit(Op::Neg);
        } else if self.eat_word("not") {
            self.unary(cx)?;
            cx.emit(Op::Not);
        } else if self.eat_word("rand") {
            self.unary(cx)?;
            cx.emit(Op::Rand);
        } else {
            self.primary(cx)?;
        }
        Ok(())
    }

    fn primary(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        let (l, c) = self.here();
        match self.bump() {
            Tok::Num(n) => {
                cx.emit(Op::Push(n));
            }
            Tok::Sym("(") => {
                self.expr(cx)?;
                if !self.eat_sym(")") {
                    return self.err_here("expected ')'");
                }
            }
            Tok::Ident(w) => {
                if let Some(s) = cx.var(&w) {
                    cx.emit(Op::Load(s));
                } else if let Some(b) = builtin(&w) {
                    cx.emit(Op::Get(b));
                } else {
                    return Err(CompileError::new(l, c, format!("unknown name '{w}'")));
                }
            }
            t => return Err(CompileError::new(l, c, format!("expected a value, found {}", describe(&t)))),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn code(src: &str) -> Vec<Op> {
        compile(src).unwrap().blocks[0].code.clone()
    }

    fn err(src: &str) -> String {
        compile(src).unwrap_err().to_string()
    }

    #[test]
    fn a_file_without_blocks_is_sound_main() {
        let p = compile("wait 2").unwrap();
        assert_eq!(p.blocks.len(), 1);
        assert_eq!(p.blocks[0].name, "main");
        assert_eq!(p.blocks[0].kind, BlockKind::Sound);
        assert_eq!(p.blocks[0].code, vec![Op::Push(2), Op::Wait, Op::End]);
    }

    #[test]
    fn named_blocks_and_on_release() {
        let p = compile("sound a\nwait 1\nend\n\ninstrument b\nwait 1\non release\nwait 2\nend\n").unwrap();
        assert_eq!(p.block("a"), Some(0));
        assert_eq!(p.block("b"), Some(1));
        let b = &p.blocks[1];
        assert_eq!(b.kind, BlockKind::Instrument);
        assert_eq!(b.code, vec![Op::Push(1), Op::Wait, Op::End, Op::Push(2), Op::Wait, Op::End]);
        assert_eq!(b.release_pc, Some(3));
    }

    #[test]
    fn precedence_and_variables() {
        assert_eq!(
            code("let x = 1 + 2 * 3\nx = x - 1"),
            vec![
                Op::Push(1), Op::Push(2), Op::Push(3), Op::Bin(BinOp::Mul), Op::Bin(BinOp::Add), Op::Store(0),
                Op::Load(0), Op::Push(1), Op::Bin(BinOp::Sub), Op::Store(0), Op::End,
            ]
        );
    }

    #[test]
    fn unary_logic_and_builtins() {
        assert_eq!(
            code("let a = not -note and rand 6 >= tick"),
            vec![
                Op::Get(Builtin::Note), Op::Neg, Op::Not,
                Op::Push(6), Op::Rand, Op::Get(Builtin::Tick), Op::Bin(BinOp::Ge),
                Op::Bin(BinOp::And), Op::Store(0), Op::End,
            ]
        );
    }

    #[test]
    fn if_else_jumps() {
        assert_eq!(
            code("if 1\nwait 1\nelse\nwait 2\nend"),
            vec![
                Op::Push(1), Op::JumpIfZero(5), Op::Push(1), Op::Wait, Op::Jump(7),
                Op::Push(2), Op::Wait, Op::End,
            ]
        );
    }

    #[test]
    fn repeat_counts_down_a_hidden_variable() {
        assert_eq!(
            code("repeat 3\nwait 1\nend"),
            vec![
                Op::Push(3), Op::Store(0),
                Op::Load(0), Op::Push(0), Op::Bin(BinOp::Gt), Op::JumpIfZero(13),
                Op::Push(1), Op::Wait,
                Op::Load(0), Op::Push(1), Op::Bin(BinOp::Sub), Op::Store(0), Op::Jump(2),
                Op::End,
            ]
        );
    }

    #[test]
    fn loop_and_wait_forms() {
        assert_eq!(
            code("loop\nwait row\nwait beat\nend"),
            vec![Op::WaitRow, Op::WaitBeat, Op::Jump(0), Op::End]
        );
    }

    #[test]
    fn error_messages() {
        assert_eq!(err("x = 1"), "1:1 unknown variable 'x' (use 'let')");
        assert_eq!(err("wait y"), "1:6 unknown name 'y'");
        assert_eq!(err("wav saw"), "1:1 unknown command 'wav'");
        assert_eq!(err("wait 1 2"), "1:8 unexpected '2'");
        assert_eq!(err("let let = 1"), "1:5 'let' is a reserved word");
        assert_eq!(err("wait (1"), "1:8 expected ')'");
        assert_eq!(err("sound a\nrepeat 2\nwait 1"), "2:1 'repeat' has no 'end'");
        assert_eq!(err("sound a\nwait 1"), "1:1 'sound' has no 'end'");
        assert_eq!(err("sound a\nend\nwait 1"), "3:1 statement outside a block");
        assert_eq!(err("sound a\nend\nsound a\nend"), "3:7 'a' is already defined");
        assert_eq!(err("sound a\non release\nend"), "2:1 only an instrument has 'on release'");
        assert_eq!(err("end"), "1:1 unexpected 'end'");
        assert_eq!(err("\n\n"), "2:1 nothing to play");
    }

    #[test]
    fn at_most_64_variables() {
        let src: String = (0..65).map(|i| alloc::format!("let x{i} = 0\n")).collect();
        assert_eq!(err(&src), "65:5 too many variables (64 max)");
    }
}
