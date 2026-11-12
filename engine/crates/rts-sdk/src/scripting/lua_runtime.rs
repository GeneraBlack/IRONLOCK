use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;

use super::{ScriptBackend, ScriptContext, ScriptLang, WorldEventContext};
use crate::schema::TriggerAction;

const MAX_LUA_OPS: usize = 50_000;
const MAX_LUA_CALL_DEPTH: usize = 32;
const MAX_LUA_STRING_BYTES: usize = 16_384;
const MAX_LUA_TABLE_ENTRIES: usize = 256;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    // Keywords
    Function,
    Local,
    If,
    Then,
    ElseIf,
    Else,
    End,
    While,
    Do,
    For,
    Repeat,
    Until,
    Return,
    Break,
    True,
    False,
    Nil,
    And,
    Or,
    Not,
    // Operators & punctuation
    Plus,
    Minus,
    Star,
    Slash,
    FloorDiv,
    Percent,
    Concat,
    Eq,
    Ne,
    Le,
    Ge,
    Lt,
    Gt,
    Assign,
    Hash,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    Dot,
    Eof,
}

fn tokenize(src: &str) -> Result<Vec<Token>> {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < len {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Comments: `--[[ ... ]]` or `-- ... \n`
        if b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            i += 2;
            if i + 1 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' {
                i += 2;
                let mut closed = false;
                while i + 1 < len {
                    if bytes[i] == b']' && bytes[i + 1] == b']' {
                        i += 2;
                        closed = true;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    bail!("Unterminated Lua block comment '--[['");
                }
            } else {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            continue;
        }

        // String literals: `"..."` or `'...'`
        if b == b'"' || b == b'\'' {
            let quote = b;
            i += 1;
            let mut out = String::new();
            let mut closed = false;
            while i < len {
                let ch_b = bytes[i];
                if ch_b == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                if ch_b == b'\\' {
                    i += 1;
                    if i >= len {
                        bail!("Unterminated escape sequence in Lua string");
                    }
                    match bytes[i] {
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'\\' => out.push('\\'),
                        b'"' => out.push('"'),
                        b'\'' => out.push('\''),
                        other => out.push(other as char),
                    }
                    i += 1;
                } else {
                    let ch = src[i..].chars().next().unwrap();
                    out.push(ch);
                    i += ch.len_utf8();
                }
                if out.len() > MAX_LUA_STRING_BYTES {
                    bail!("Lua string literal exceeds maximum size ({MAX_LUA_STRING_BYTES} bytes)");
                }
            }
            if !closed {
                bail!("Unterminated Lua string literal");
            }
            tokens.push(Token::Str(out));
            continue;
        }

        // Numeric literals (hex `0x...`, exact integer `i64`, or float `f64`)
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'0' && i + 1 < len && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
                i += 2;
                let hex_start = i;
                while i < len && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                if i == hex_start {
                    bail!("Invalid hexadecimal literal in Lua script");
                }
                let val = i64::from_str_radix(&src[hex_start..i], 16)
                    .map_err(|e| anyhow!("Invalid Lua hex integer '{}': {e}", &src[start..i]))?;
                tokens.push(Token::Int(val));
                continue;
            }

            let mut is_float = false;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < len && bytes[i] == b'.' && (i + 1 >= len || bytes[i + 1] != b'.') {
                is_float = true;
                i += 1;
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
                is_float = true;
                i += 1;
                if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                let exp_start = i;
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i == exp_start {
                    bail!("Invalid exponent in Lua number '{}'", &src[start..i]);
                }
            }
            let slice = &src[start..i];
            if is_float {
                let f = slice
                    .parse::<f64>()
                    .map_err(|e| anyhow!("Invalid Lua float '{slice}': {e}"))?;
                tokens.push(Token::Float(f));
            } else if let Ok(iv) = slice.parse::<i64>() {
                tokens.push(Token::Int(iv));
            } else {
                let f = slice
                    .parse::<f64>()
                    .map_err(|e| anyhow!("Invalid Lua number '{slice}': {e}"))?;
                tokens.push(Token::Float(f));
            }
            continue;
        }

        // Identifiers & keywords
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = &src[start..i];
            let tok = match ident {
                "function" => Token::Function,
                "local" => Token::Local,
                "if" => Token::If,
                "then" => Token::Then,
                "elseif" => Token::ElseIf,
                "else" => Token::Else,
                "end" => Token::End,
                "while" => Token::While,
                "do" => Token::Do,
                "for" => Token::For,
                "repeat" => Token::Repeat,
                "until" => Token::Until,
                "return" => Token::Return,
                "break" => Token::Break,
                "true" => Token::True,
                "false" => Token::False,
                "nil" => Token::Nil,
                "and" => Token::And,
                "or" => Token::Or,
                "not" => Token::Not,
                _ => Token::Ident(ident.to_string()),
            };
            tokens.push(tok);
            continue;
        }

        // Two-character operators
        if i + 1 < len {
            let two = &bytes[i..i + 2];
            let matched = match two {
                b"==" => Some(Token::Eq),
                b"~=" => Some(Token::Ne),
                b"<=" => Some(Token::Le),
                b">=" => Some(Token::Ge),
                b".." => Some(Token::Concat),
                b"//" => Some(Token::FloorDiv),
                _ => None,
            };
            if let Some(t) = matched {
                tokens.push(t);
                i += 2;
                continue;
            }
        }

        // Single-character operators
        let single = match b {
            b'+' => Token::Plus,
            b'-' => Token::Minus,
            b'*' => Token::Star,
            b'/' => Token::Slash,
            b'%' => Token::Percent,
            b'<' => Token::Lt,
            b'>' => Token::Gt,
            b'=' => Token::Assign,
            b'#' => Token::Hash,
            b'(' => Token::LParen,
            b')' => Token::RParen,
            b'{' => Token::LBrace,
            b'}' => Token::RBrace,
            b'[' => Token::LBracket,
            b']' => Token::RBracket,
            b',' => Token::Comma,
            b';' => Token::Semi,
            b':' => Token::Colon,
            b'.' => Token::Dot,
            _ => bail!(
                "Unexpected character '{}' in Lua script",
                src[i..].chars().next().unwrap()
            ),
        };
        tokens.push(single);
        i += 1;
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

#[derive(Debug, Clone)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone)]
enum UnOp {
    Neg,
    Not,
    Len,
}

#[derive(Debug, Clone)]
enum TableFieldInit {
    Named(String, LuaExpr),
    Indexed(LuaExpr, LuaExpr),
    Positional(LuaExpr),
}

#[derive(Debug, Clone)]
enum LuaExpr {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Var(String),
    Table(Vec<TableFieldInit>),
    Field(Box<LuaExpr>, String),
    Index(Box<LuaExpr>, Box<LuaExpr>),
    Unary(UnOp, Box<LuaExpr>),
    Binary(BinOp, Box<LuaExpr>, Box<LuaExpr>),
    Call {
        callee: Box<LuaExpr>,
        args: Vec<LuaExpr>,
    },
    MethodCall {
        receiver: Box<LuaExpr>,
        method: String,
        args: Vec<LuaExpr>,
    },
}

#[derive(Debug, Clone)]
enum LValue {
    Var(String),
    Field(LuaExpr, String),
    Index(LuaExpr, LuaExpr),
}

#[derive(Debug, Clone)]
enum LuaStmt {
    Local {
        names: Vec<String>,
        values: Vec<LuaExpr>,
    },
    Assign {
        targets: Vec<LValue>,
        values: Vec<LuaExpr>,
    },
    ExprStmt(LuaExpr),
    If {
        branches: Vec<(LuaExpr, Vec<LuaStmt>)>,
        else_body: Vec<LuaStmt>,
    },
    While {
        cond: LuaExpr,
        body: Vec<LuaStmt>,
    },
    Repeat {
        body: Vec<LuaStmt>,
        until_cond: LuaExpr,
    },
    ForNumeric {
        var: String,
        start: LuaExpr,
        stop: LuaExpr,
        step: Option<LuaExpr>,
        body: Vec<LuaStmt>,
    },
    DoBlock(Vec<LuaStmt>),
    Break,
    Return(Option<LuaExpr>),
}

#[derive(Debug, Clone)]
struct LuaFunction {
    params: Vec<String>,
    body: Vec<LuaStmt>,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn next_tok(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, expected: &Token) -> bool {
        if self.peek() == expected {
            self.next_tok();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        let got = self.next_tok();
        if &got == expected {
            Ok(())
        } else {
            bail!("Expected Lua token {expected:?}, got {got:?}")
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.next_tok() {
            Token::Ident(s) => Ok(s),
            other => bail!("Expected Lua identifier, got {other:?}"),
        }
    }

    fn skip_semicolons(&mut self) {
        while self.eat(&Token::Semi) {}
    }

    fn is_block_terminator(&self) -> bool {
        matches!(
            self.peek(),
            Token::End | Token::Else | Token::ElseIf | Token::Until | Token::Eof
        )
    }

    fn parse_function_def(&mut self) -> Result<(String, LuaFunction)> {
        let mut name = self.expect_ident()?;
        while self.eat(&Token::Dot) {
            let sub = self.expect_ident()?;
            name.push('.');
            name.push_str(&sub);
        }
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();
        if !self.eat(&Token::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::RParen)?;
        }
        let body = self.parse_block()?;
        self.expect(&Token::End)?;
        Ok((name, LuaFunction { params, body }))
    }

    fn parse_block(&mut self) -> Result<Vec<LuaStmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_semicolons();
            if self.is_block_terminator() {
                break;
            }
            stmts.push(self.parse_stmt()?);
            self.skip_semicolons();
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<LuaStmt> {
        match self.peek().clone() {
            Token::Local => {
                self.next_tok();
                let mut names = vec![self.expect_ident()?];
                while self.eat(&Token::Comma) {
                    names.push(self.expect_ident()?);
                }
                let mut values = Vec::new();
                if self.eat(&Token::Assign) {
                    values.push(self.parse_expr()?);
                    while self.eat(&Token::Comma) {
                        values.push(self.parse_expr()?);
                    }
                }
                Ok(LuaStmt::Local { names, values })
            }
            Token::If => {
                self.next_tok();
                let cond = self.parse_expr()?;
                self.expect(&Token::Then)?;
                let then_body = self.parse_block()?;
                let mut branches = vec![(cond, then_body)];
                while self.eat(&Token::ElseIf) {
                    let e_cond = self.parse_expr()?;
                    self.expect(&Token::Then)?;
                    let e_body = self.parse_block()?;
                    branches.push((e_cond, e_body));
                }
                let else_body = if self.eat(&Token::Else) {
                    self.parse_block()?
                } else {
                    Vec::new()
                };
                self.expect(&Token::End)?;
                Ok(LuaStmt::If {
                    branches,
                    else_body,
                })
            }
            Token::While => {
                self.next_tok();
                let cond = self.parse_expr()?;
                self.expect(&Token::Do)?;
                let body = self.parse_block()?;
                self.expect(&Token::End)?;
                Ok(LuaStmt::While { cond, body })
            }
            Token::Repeat => {
                self.next_tok();
                let body = self.parse_block()?;
                self.expect(&Token::Until)?;
                let until_cond = self.parse_expr()?;
                Ok(LuaStmt::Repeat { body, until_cond })
            }
            Token::For => {
                self.next_tok();
                let var = self.expect_ident()?;
                self.expect(&Token::Assign)?;
                let start = self.parse_expr()?;
                self.expect(&Token::Comma)?;
                let stop = self.parse_expr()?;
                let step = if self.eat(&Token::Comma) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(&Token::Do)?;
                let body = self.parse_block()?;
                self.expect(&Token::End)?;
                Ok(LuaStmt::ForNumeric {
                    var,
                    start,
                    stop,
                    step,
                    body,
                })
            }
            Token::Do => {
                self.next_tok();
                let body = self.parse_block()?;
                self.expect(&Token::End)?;
                Ok(LuaStmt::DoBlock(body))
            }
            Token::Break => {
                self.next_tok();
                Ok(LuaStmt::Break)
            }
            Token::Return => {
                self.next_tok();
                if self.is_block_terminator() || self.peek() == &Token::Semi {
                    Ok(LuaStmt::Return(None))
                } else {
                    let expr = self.parse_expr()?;
                    Ok(LuaStmt::Return(Some(expr)))
                }
            }
            _ => {
                let first = self.parse_postfix_expr()?;
                if self.peek() == &Token::Assign || self.peek() == &Token::Comma {
                    let mut targets = vec![Self::expr_to_lvalue(first)?];
                    while self.eat(&Token::Comma) {
                        let next_e = self.parse_postfix_expr()?;
                        targets.push(Self::expr_to_lvalue(next_e)?);
                    }
                    self.expect(&Token::Assign)?;
                    let mut values = vec![self.parse_expr()?];
                    while self.eat(&Token::Comma) {
                        values.push(self.parse_expr()?);
                    }
                    Ok(LuaStmt::Assign { targets, values })
                } else {
                    match &first {
                        LuaExpr::Call { .. } | LuaExpr::MethodCall { .. } => {
                            Ok(LuaStmt::ExprStmt(first))
                        }
                        _ => bail!("Invalid standalone Lua expression statement: {first:?}"),
                    }
                }
            }
        }
    }

    fn expr_to_lvalue(expr: LuaExpr) -> Result<LValue> {
        match expr {
            LuaExpr::Var(name) => Ok(LValue::Var(name)),
            LuaExpr::Field(base, field) => Ok(LValue::Field(*base, field)),
            LuaExpr::Index(base, idx) => Ok(LValue::Index(*base, *idx)),
            other => bail!("Invalid Lua assignment target: {other:?}"),
        }
    }

    fn parse_expr(&mut self) -> Result<LuaExpr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<LuaExpr> {
        let mut left = self.parse_and()?;
        while self.eat(&Token::Or) {
            let right = self.parse_and()?;
            left = LuaExpr::Binary(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<LuaExpr> {
        let mut left = self.parse_cmp()?;
        while self.eat(&Token::And) {
            let right = self.parse_cmp()?;
            left = LuaExpr::Binary(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<LuaExpr> {
        let mut left = self.parse_concat()?;
        loop {
            let op = match self.peek() {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.next_tok();
            let right = self.parse_concat()?;
            left = LuaExpr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<LuaExpr> {
        let left = self.parse_add_sub()?;
        if self.eat(&Token::Concat) {
            let right = self.parse_concat()?;
            Ok(LuaExpr::Binary(
                BinOp::Concat,
                Box::new(left),
                Box::new(right),
            ))
        } else {
            Ok(left)
        }
    }

    fn parse_add_sub(&mut self) -> Result<LuaExpr> {
        let mut left = self.parse_mul_div()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.next_tok();
            let right = self.parse_mul_div()?;
            left = LuaExpr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<LuaExpr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::FloorDiv => BinOp::FloorDiv,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.next_tok();
            let right = self.parse_unary()?;
            left = LuaExpr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<LuaExpr> {
        match self.peek() {
            Token::Minus => {
                self.next_tok();
                let inner = self.parse_unary()?;
                Ok(LuaExpr::Unary(UnOp::Neg, Box::new(inner)))
            }
            Token::Not => {
                self.next_tok();
                let inner = self.parse_unary()?;
                Ok(LuaExpr::Unary(UnOp::Not, Box::new(inner)))
            }
            Token::Hash => {
                self.next_tok();
                let inner = self.parse_unary()?;
                Ok(LuaExpr::Unary(UnOp::Len, Box::new(inner)))
            }
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_postfix_expr(&mut self) -> Result<LuaExpr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(&Token::Dot) {
                let field = self.expect_ident()?;
                expr = LuaExpr::Field(Box::new(expr), field);
            } else if self.eat(&Token::LBracket) {
                let idx = self.parse_expr()?;
                self.expect(&Token::RBracket)?;
                expr = LuaExpr::Index(Box::new(expr), Box::new(idx));
            } else if self.eat(&Token::Colon) {
                let method = self.expect_ident()?;
                let args = self.parse_call_args()?;
                expr = LuaExpr::MethodCall {
                    receiver: Box::new(expr),
                    method,
                    args,
                };
            } else if self.peek() == &Token::LParen {
                let args = self.parse_call_args()?;
                expr = LuaExpr::Call {
                    callee: Box::new(expr),
                    args,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<LuaExpr>> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !self.eat(&Token::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::RParen)?;
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<LuaExpr> {
        match self.next_tok() {
            Token::Nil => Ok(LuaExpr::Nil),
            Token::True => Ok(LuaExpr::Bool(true)),
            Token::False => Ok(LuaExpr::Bool(false)),
            Token::Int(v) => Ok(LuaExpr::Int(v)),
            Token::Float(v) => Ok(LuaExpr::Float(v)),
            Token::Str(s) => Ok(LuaExpr::Str(s)),
            Token::Ident(name) => Ok(LuaExpr::Var(name)),
            Token::LParen => {
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Token::LBrace => {
                let mut fields = Vec::new();
                while !self.eat(&Token::RBrace) {
                    if self.eat(&Token::LBracket) {
                        let k = self.parse_expr()?;
                        self.expect(&Token::RBracket)?;
                        self.expect(&Token::Assign)?;
                        let v = self.parse_expr()?;
                        fields.push(TableFieldInit::Indexed(k, v));
                    } else if matches!(self.peek(), Token::Ident(_))
                        && self.tokens.get(self.pos + 1) == Some(&Token::Assign)
                    {
                        let k = self.expect_ident()?;
                        self.expect(&Token::Assign)?;
                        let v = self.parse_expr()?;
                        fields.push(TableFieldInit::Named(k, v));
                    } else {
                        let v = self.parse_expr()?;
                        fields.push(TableFieldInit::Positional(v));
                    }
                    if !self.eat(&Token::Comma) && !self.eat(&Token::Semi) {
                        self.expect(&Token::RBrace)?;
                        break;
                    }
                }
                Ok(LuaExpr::Table(fields))
            }
            other => bail!("Unexpected token in Lua expression: {other:?}"),
        }
    }
}

#[derive(Debug, Clone)]
enum LuaValue {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Table(BTreeMap<String, LuaValue>),
}

impl PartialEq for LuaValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LuaValue::Nil, LuaValue::Nil) => true,
            (LuaValue::Bool(a), LuaValue::Bool(b)) => a == b,
            (LuaValue::Int(a), LuaValue::Int(b)) => a == b,
            (LuaValue::Float(a), LuaValue::Float(b)) => a == b,
            (LuaValue::Int(a), LuaValue::Float(b)) => (*a as f64) == *b,
            (LuaValue::Float(a), LuaValue::Int(b)) => *a == (*b as f64),
            (LuaValue::Str(a), LuaValue::Str(b)) => a == b,
            (LuaValue::Table(a), LuaValue::Table(b)) => a == b,
            _ => false,
        }
    }
}

impl LuaValue {
    fn is_truthy(&self) -> bool {
        !matches!(self, LuaValue::Nil | LuaValue::Bool(false))
    }

    fn as_f64(&self) -> Result<f64> {
        match self {
            LuaValue::Int(i) => Ok(*i as f64),
            LuaValue::Float(f) => Ok(*f),
            _ => bail!("Expected numeric Lua value, got {self:?}"),
        }
    }

    fn as_f32(&self) -> Result<f32> {
        Ok(self.as_f64()? as f32)
    }

    fn as_i64(&self) -> Result<i64> {
        match self {
            LuaValue::Int(i) => Ok(*i),
            LuaValue::Float(f) => Ok(f.round() as i64),
            _ => bail!("Expected integer Lua value, got {self:?}"),
        }
    }

    fn as_bool_strict(&self) -> Result<bool> {
        match self {
            LuaValue::Bool(b) => Ok(*b),
            _ => bail!("Expected boolean Lua value, got {self:?}"),
        }
    }

    fn as_str(&self) -> Result<String> {
        match self {
            LuaValue::Str(s) => Ok(s.clone()),
            _ => bail!("Expected string Lua value, got {self:?}"),
        }
    }

    fn to_lua_string(&self) -> String {
        match self {
            LuaValue::Nil => "nil".to_string(),
            LuaValue::Bool(b) => b.to_string(),
            LuaValue::Int(i) => i.to_string(),
            LuaValue::Float(f) => {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            LuaValue::Str(s) => s.clone(),
            LuaValue::Table(_) => "table".to_string(),
        }
    }

    fn to_table_key(&self) -> Result<String> {
        match self {
            LuaValue::Str(s) => Ok(s.clone()),
            LuaValue::Int(i) => Ok(i.to_string()),
            LuaValue::Float(f) if f.fract() == 0.0 => Ok((f.round() as i64).to_string()),
            LuaValue::Bool(b) => Ok(b.to_string()),
            _ => bail!("Invalid Lua table key: {self:?}"),
        }
    }
}

enum FlowSignal {
    None,
    Break,
    Return(LuaValue),
}

enum HookTarget<'a> {
    None,
    Unit {
        ctx_var: String,
        ctx: &'a mut ScriptContext,
    },
    World {
        ctx_var: String,
        ctx: &'a mut WorldEventContext,
        focus_x: Option<i32>,
        focus_z: Option<i32>,
        spawn_tx: Option<i32>,
        spawn_tz: Option<i32>,
        attack_x: Option<f32>,
        attack_z: Option<f32>,
        eva_speaker: Option<String>,
        eva_text: Option<String>,
    },
}

struct LuaVm<'a> {
    backend: &'a LuaBackend,
    globals: BTreeMap<String, LuaValue>,
    scopes: Vec<BTreeMap<String, LuaValue>>,
    ops_used: usize,
    call_depth: usize,
    target: HookTarget<'a>,
}

impl<'a> LuaVm<'a> {
    fn new(backend: &'a LuaBackend, target: HookTarget<'a>) -> Self {
        Self {
            backend,
            globals: backend.base_globals.clone(),
            scopes: vec![BTreeMap::new()],
            ops_used: 0,
            call_depth: 0,
            target,
        }
    }

    fn tick_op(&mut self) -> Result<()> {
        self.ops_used += 1;
        if self.ops_used > MAX_LUA_OPS {
            bail!("Lua execution exceeded maximum operation cap ({MAX_LUA_OPS})");
        }
        Ok(())
    }

    fn is_ctx_var(&self, name: &str) -> bool {
        match &self.target {
            HookTarget::Unit { ctx_var, .. } | HookTarget::World { ctx_var, .. } => {
                !ctx_var.is_empty() && ctx_var == name
            }
            HookTarget::None => false,
        }
    }

    fn get_var(&self, name: &str) -> LuaValue {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return v.clone();
            }
        }
        if self.is_ctx_var(name) {
            return LuaValue::Str("__rts_ctx__".to_string());
        }
        self.globals.get(name).cloned().unwrap_or(LuaValue::Nil)
    }

    fn set_var(&mut self, name: &str, val: LuaValue) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = val;
                return;
            }
        }
        self.globals.insert(name.to_string(), val);
    }

    fn define_local(&mut self, name: String, val: LuaValue) {
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name, val);
        }
    }

    fn read_ctx_field(&self, field: &str) -> Result<LuaValue> {
        match &self.target {
            HookTarget::Unit { ctx, .. } => match field {
                "entity_id" => Ok(LuaValue::Int(ctx.entity_id as i64)),
                "unit_id" => Ok(LuaValue::Str(ctx.unit_id.clone())),
                "player_id" => Ok(LuaValue::Int(ctx.player_id as i64)),
                "pos_x" => Ok(LuaValue::Float(ctx.self_pos[0] as f64)),
                "pos_y" => Ok(LuaValue::Float(ctx.self_pos[1] as f64)),
                "pos_z" => Ok(LuaValue::Float(ctx.self_pos[2] as f64)),
                "health" => Ok(LuaValue::Float(ctx.health as f64)),
                "max_health" => Ok(LuaValue::Float(ctx.max_health as f64)),
                "energy" => Ok(LuaValue::Float(ctx.energy as f64)),
                "max_energy" => Ok(LuaValue::Float(ctx.max_energy as f64)),
                "speed_multiplier" => Ok(LuaValue::Float(ctx.speed_multiplier as f64)),
                "armor_multiplier" => Ok(LuaValue::Float(ctx.armor_multiplier as f64)),
                "damage_multiplier" => Ok(LuaValue::Float(ctx.damage_multiplier as f64)),
                "range_bonus" => Ok(LuaValue::Float(ctx.range_bonus as f64)),
                "deployed" => Ok(LuaValue::Bool(ctx.deployed)),
                "has_target_pos" => Ok(LuaValue::Bool(ctx.target_pos.is_some())),
                "target_x" => Ok(LuaValue::Float(
                    ctx.target_pos.map(|p| p[0]).unwrap_or(0.0) as f64
                )),
                "target_y" => Ok(LuaValue::Float(
                    ctx.target_pos.map(|p| p[1]).unwrap_or(0.0) as f64
                )),
                "target_z" => Ok(LuaValue::Float(
                    ctx.target_pos.map(|p| p[2]).unwrap_or(0.0) as f64
                )),
                "target_entity_id" => Ok(LuaValue::Int(ctx.target_entity_id.unwrap_or(0) as i64)),
                "attacker_id" => Ok(LuaValue::Int(ctx.attacker_id.unwrap_or(0) as i64)),
                other => {
                    if let Some(flag) = other.strip_prefix("flag_") {
                        let val = ctx.custom_flags.get(flag).copied().unwrap_or(false);
                        Ok(LuaValue::Bool(val))
                    } else {
                        bail!("Unknown ctx field '{other}' in Lua script")
                    }
                }
            },
            HookTarget::World {
                ctx,
                focus_x,
                focus_z,
                spawn_tx,
                spawn_tz,
                attack_x,
                attack_z,
                eva_speaker,
                eva_text,
                ..
            } => match field {
                "event_type" => Ok(LuaValue::Str(ctx.event_type.clone())),
                "tick" => Ok(LuaValue::Int(ctx.tick as i64)),
                "entity_id" => Ok(LuaValue::Int(ctx.entity_id as i64)),
                "player_id" => Ok(LuaValue::Int(ctx.player_id as i64)),
                "health" | "bonus_credits" => Ok(LuaValue::Int(ctx.bonus_credits as i64)),
                "spawn_unit_id" => Ok(LuaValue::Str(ctx.spawn_unit_id.clone().unwrap_or_default())),
                "spawn_count" => Ok(LuaValue::Int(ctx.spawn_count as i64)),
                "spawn_player_id" => Ok(LuaValue::Int(
                    ctx.spawn_player_id.unwrap_or(ctx.player_id) as i64,
                )),
                "spawn_tile_x" => Ok(LuaValue::Int(spawn_tx.unwrap_or(0) as i64)),
                "spawn_tile_z" => Ok(LuaValue::Int(spawn_tz.unwrap_or(0) as i64)),
                "attack_move_x" => Ok(LuaValue::Float(attack_x.unwrap_or(0.0) as f64)),
                "attack_move_z" => Ok(LuaValue::Float(attack_z.unwrap_or(0.0) as f64)),
                "objective_text" => Ok(LuaValue::Str(
                    ctx.objective_text.clone().unwrap_or_default(),
                )),
                "declared_winner" => Ok(LuaValue::Int(ctx.declared_winner.unwrap_or(0) as i64)),
                "focus_tile_x" | "camera_focus_x" => Ok(LuaValue::Int(focus_x.unwrap_or(0) as i64)),
                "focus_tile_z" | "camera_focus_z" => Ok(LuaValue::Int(focus_z.unwrap_or(0) as i64)),
                "eva_speaker" => Ok(LuaValue::Str(eva_speaker.clone().unwrap_or_default())),
                "eva_text" => Ok(LuaValue::Str(eva_text.clone().unwrap_or_default())),
                "vars" => {
                    let mut m = BTreeMap::new();
                    for (k, v) in &ctx.trigger_vars {
                        m.insert(k.clone(), LuaValue::Int(*v));
                    }
                    Ok(LuaValue::Table(m))
                }
                other => {
                    if let Some(flag) = other.strip_prefix("flag_") {
                        let val = ctx.custom_flags.get(flag).copied().unwrap_or(false);
                        Ok(LuaValue::Bool(val))
                    } else if let Some(var_name) = other.strip_prefix("var_") {
                        let val = ctx.trigger_vars.get(var_name).copied().unwrap_or(0);
                        Ok(LuaValue::Int(val))
                    } else {
                        bail!("Unknown world ctx field '{other}' in Lua script")
                    }
                }
            },
            HookTarget::None => bail!("No active hook context for field '{field}'"),
        }
    }

    fn write_ctx_field(&mut self, field: &str, val: LuaValue) -> Result<()> {
        match &mut self.target {
            HookTarget::Unit { ctx, .. } => match field {
                "health" => ctx.health = val.as_f32()?,
                "energy" => ctx.energy = val.as_f32()?,
                "speed_multiplier" => ctx.speed_multiplier = val.as_f32()?,
                "armor_multiplier" => ctx.armor_multiplier = val.as_f32()?,
                "damage_multiplier" => ctx.damage_multiplier = val.as_f32()?,
                "range_bonus" => ctx.range_bonus = val.as_f32()?,
                "deployed" => ctx.deployed = val.as_bool_strict()?,
                other if other.starts_with("flag_") => {
                    let flag_name = other.trim_start_matches("flag_").to_string();
                    ctx.custom_flags.insert(flag_name, val.as_bool_strict()?);
                }
                other => bail!("Invalid assignment to ctx.{other} in Lua unit script"),
            },
            HookTarget::World {
                ctx,
                focus_x,
                focus_z,
                spawn_tx,
                spawn_tz,
                attack_x,
                attack_z,
                eva_speaker,
                eva_text,
                ..
            } => match field {
                "bonus_credits" | "health" => ctx.bonus_credits = val.as_i64()? as i32,
                "spawn_unit_id" => {
                    let s = val.as_str()?;
                    if !s.is_empty() {
                        ctx.spawn_unit_id = Some(s);
                    }
                }
                "spawn_count" => ctx.spawn_count = val.as_i64()?.max(0) as u32,
                "spawn_tile_x" => *spawn_tx = Some(val.as_i64()? as i32),
                "spawn_tile_z" => *spawn_tz = Some(val.as_i64()? as i32),
                "spawn_player_id" => {
                    ctx.spawn_player_id = Some(val.as_i64()?.clamp(0, 8) as u8);
                }
                "attack_move_x" => *attack_x = Some(val.as_f32()?),
                "attack_move_z" => *attack_z = Some(val.as_f32()?),
                "objective_text" => {
                    let s = val.as_str()?;
                    if !s.is_empty() {
                        ctx.objective_text = Some(s);
                    }
                }
                "declared_winner" => {
                    let w = val.as_i64()?;
                    if w > 0 {
                        ctx.declared_winner = Some(w as u8);
                    }
                }
                "focus_tile_x" | "camera_focus_x" => *focus_x = Some(val.as_i64()? as i32),
                "focus_tile_z" | "camera_focus_z" => *focus_z = Some(val.as_i64()? as i32),
                "eva_speaker" => *eva_speaker = Some(val.as_str()?),
                "eva_text" => *eva_text = Some(val.as_str()?),
                other if other.starts_with("flag_") => {
                    let flag_name = other.trim_start_matches("flag_").to_string();
                    ctx.custom_flags.insert(flag_name, val.as_bool_strict()?);
                }
                other if other.starts_with("var_") => {
                    let var_name = other.trim_start_matches("var_").to_string();
                    ctx.trigger_vars.insert(var_name, val.as_i64()?);
                }
                other => bail!("Invalid assignment to world ctx.{other} in Lua script"),
            },
            HookTarget::None => bail!("No active hook context for assignment to '{field}'"),
        }
        Ok(())
    }

    fn exec_queue_method(&mut self, method: &str, eval_args: Vec<LuaValue>) -> Result<LuaValue> {
        let maybe_script_action = match method {
            "apply_buff" => {
                if eval_args.len() < 3 || eval_args.len() > 4 {
                    bail!("apply_buff expects (target_entity_id, status_id, duration_secs [, stacks])");
                }
                let stacks = if eval_args.len() == 4 {
                    eval_args[3].as_i64()?.clamp(1, 255) as u8
                } else {
                    1
                };
                Some(super::ScriptAction::ApplyBuff {
                    target_entity_id: eval_args[0].as_i64()?.max(0) as u64,
                    status_id: eval_args[1].as_str()?,
                    duration_secs: eval_args[2].as_f32()?,
                    stacks,
                })
            }
            "spawn_projectile" => {
                if eval_args.len() != 7 {
                    bail!("spawn_projectile expects (weapon_id, ox, oy, oz, tx, ty, tz)");
                }
                Some(super::ScriptAction::SpawnProjectile {
                    weapon_id: eval_args[0].as_str()?,
                    origin: [
                        eval_args[1].as_f32()?,
                        eval_args[2].as_f32()?,
                        eval_args[3].as_f32()?,
                    ],
                    target_pos: [
                        eval_args[4].as_f32()?,
                        eval_args[5].as_f32()?,
                        eval_args[6].as_f32()?,
                    ],
                    target_entity_id: None,
                })
            }
            "spawn_entity" => {
                if eval_args.len() != 5 {
                    bail!("spawn_entity expects (def_id, player_id, x, y, z)");
                }
                Some(super::ScriptAction::SpawnEntity {
                    def_id: eval_args[0].as_str()?,
                    player_id: eval_args[1].as_i64()?.clamp(0, 8) as u8,
                    pos: [
                        eval_args[2].as_f32()?,
                        eval_args[3].as_f32()?,
                        eval_args[4].as_f32()?,
                    ],
                    is_building: false,
                })
            }
            "deal_area_damage" => {
                if eval_args.len() != 6 {
                    bail!("deal_area_damage expects (x, y, z, radius, damage, friendly_fire)");
                }
                Some(super::ScriptAction::DealAreaDamage {
                    center: [
                        eval_args[0].as_f32()?,
                        eval_args[1].as_f32()?,
                        eval_args[2].as_f32()?,
                    ],
                    radius: eval_args[3].as_f32()?,
                    damage: eval_args[4].as_f32()?,
                    friendly_fire: eval_args[5].as_bool_strict()?,
                })
            }
            "issue_command" => {
                if eval_args.len() != 5 {
                    bail!("issue_command expects (entity_id, command, tx, ty, tz)");
                }
                Some(super::ScriptAction::IssueCommand {
                    entity_id: eval_args[0].as_i64()?.max(0) as u64,
                    command: eval_args[1].as_str()?,
                    target_pos: Some([
                        eval_args[2].as_f32()?,
                        eval_args[3].as_f32()?,
                        eval_args[4].as_f32()?,
                    ]),
                    target_entity_id: None,
                })
            }
            _ => None,
        };
        if let Some(sa) = maybe_script_action {
            match &mut self.target {
                HookTarget::Unit { ctx, .. } => {
                    ctx.emitted_actions.push(sa);
                    return Ok(LuaValue::Nil);
                }
                HookTarget::World { ctx, .. } => {
                    ctx.emitted_actions.push(sa);
                    return Ok(LuaValue::Nil);
                }
                HookTarget::None => bail!("No active hook context for '{method}'"),
            }
        }

        let HookTarget::World { ctx, .. } = &mut self.target else {
            bail!("World method '{method}' is only supported in world-event hooks");
        };
        match method {
            "queue_grant_resource" => {
                if eval_args.len() != 3 {
                    bail!("queue_grant_resource expects (player_id, resource_id, delta)");
                }
                ctx.resource_grants.push(super::ScriptResourceGrant {
                    player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    resource_id: eval_args[1].as_str()?,
                    delta: eval_args[2].as_i64()?,
                });
                return Ok(LuaValue::Nil);
            }
            "query_player_resource" => {
                if eval_args.len() != 2 {
                    bail!("query_player_resource expects (player_id, resource_id)");
                }
                let pid = eval_args[0].as_i64()?.clamp(0, 255) as u8;
                let res_id = eval_args[1].as_str()?;
                if let Some(ps) = ctx.player_snapshots.get(&pid) {
                    if let Some(&amt) = ps.resources.get(&res_id) {
                        return Ok(LuaValue::Int(amt as i64));
                    }
                    if res_id == "credits" {
                        return Ok(LuaValue::Int(ps.credits as i64));
                    }
                    if res_id == "power" {
                        return Ok(LuaValue::Int(ps.power_balance as i64));
                    }
                }
                return Ok(LuaValue::Int(0));
            }
            "query_unit_count" => {
                if eval_args.len() != 1 {
                    bail!("query_unit_count expects (player_id)");
                }
                let pid = eval_args[0].as_i64()?.clamp(0, 255) as u8;
                let cnt = ctx
                    .player_snapshots
                    .get(&pid)
                    .map(|p| p.unit_count as i64)
                    .unwrap_or(0);
                return Ok(LuaValue::Int(cnt));
            }
            "query_building_count" => {
                if eval_args.len() != 1 {
                    bail!("query_building_count expects (player_id)");
                }
                let pid = eval_args[0].as_i64()?.clamp(0, 255) as u8;
                let cnt = ctx
                    .player_snapshots
                    .get(&pid)
                    .map(|p| p.building_count as i64)
                    .unwrap_or(0);
                return Ok(LuaValue::Int(cnt));
            }
            "query_entities_in_radius" => {
                if eval_args.len() != 4 {
                    bail!(
                        "query_entities_in_radius expects (tile_x, tile_z, radius_tiles, owner_filter)"
                    );
                }
                let tx = eval_args[0].as_i64()?;
                let tz = eval_args[1].as_i64()?;
                let r = eval_args[2].as_i64()?.max(0);
                let owner_filter = eval_args[3].as_i64()?;
                let ids: Vec<u64> = if let Some(sp) = &ctx.spatial_query {
                    sp.0.query_ids_in_radius(tx as i32, tz as i32, r as i32, owner_filter as i32)
                } else {
                    let r2 = r.saturating_mul(r);
                    let mut v: Vec<u64> = ctx
                        .entity_snapshots
                        .iter()
                        .filter(|e| {
                            if owner_filter > 0 && e.player_id as i64 != owner_filter {
                                return false;
                            }
                            let dx = e.tile_pos[0] as i64 - tx;
                            let dz = e.tile_pos[1] as i64 - tz;
                            dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)) <= r2
                        })
                        .map(|e| e.entity_id)
                        .collect();
                    v.sort_unstable();
                    v
                };
                let mut tbl = BTreeMap::new();
                for (i, eid) in ids.into_iter().enumerate() {
                    tbl.insert((i + 1).to_string(), LuaValue::Int(eid as i64));
                }
                return Ok(LuaValue::Table(tbl));
            }
            "count_entities_in_radius" => {
                if eval_args.len() != 4 {
                    bail!(
                        "count_entities_in_radius expects (tile_x, tile_z, radius_tiles, owner_filter)"
                    );
                }
                let tx = eval_args[0].as_i64()?;
                let tz = eval_args[1].as_i64()?;
                let r = eval_args[2].as_i64()?.max(0);
                let owner_filter = eval_args[3].as_i64()?;
                if let Some(sp) = &ctx.spatial_query {
                    let cnt =
                        sp.0.count_in_radius(tx as i32, tz as i32, r as i32, owner_filter as i32);
                    return Ok(LuaValue::Int(cnt as i64));
                }
                let r2 = r.saturating_mul(r);
                let cnt = ctx
                    .entity_snapshots
                    .iter()
                    .filter(|e| {
                        if owner_filter > 0 && e.player_id as i64 != owner_filter {
                            return false;
                        }
                        let dx = e.tile_pos[0] as i64 - tx;
                        let dz = e.tile_pos[1] as i64 - tz;
                        dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)) <= r2
                    })
                    .count();
                return Ok(LuaValue::Int(cnt as i64));
            }
            _ => {}
        }
        let action = match method {
            "queue_set_variable" => {
                if eval_args.len() != 2 {
                    bail!("queue_set_variable expects (name, value)");
                }
                TriggerAction::SetVariable {
                    name: eval_args[0].as_str()?,
                    value: eval_args[1].as_i64()?,
                }
            }
            "queue_add_variable" => {
                if eval_args.len() != 2 {
                    bail!("queue_add_variable expects (name, delta)");
                }
                TriggerAction::AddVariable {
                    name: eval_args[0].as_str()?,
                    delta: eval_args[1].as_i64()?,
                }
            }
            "queue_grant_credits" => {
                if eval_args.len() != 2 {
                    bail!("queue_grant_credits expects (player_id, amount)");
                }
                TriggerAction::GrantCredits {
                    player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    amount: eval_args[1].as_i64()?.max(0) as u32,
                }
            }
            "queue_spawn_reinforcements" => {
                if eval_args.len() != 5 {
                    bail!(
                        "queue_spawn_reinforcements expects (player_id, unit_id, count, tile_x, tile_z)"
                    );
                }
                TriggerAction::SpawnReinforcements {
                    player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    unit_id: eval_args[1].as_str()?,
                    count: eval_args[2].as_i64()?.max(1) as u32,
                    tile: [eval_args[3].as_i64()? as i32, eval_args[4].as_i64()? as i32],
                }
            }
            "queue_spawn_building" => {
                if eval_args.len() != 4 {
                    bail!("queue_spawn_building expects (player_id, building_id, tile_x, tile_z)");
                }
                TriggerAction::SpawnBuilding {
                    player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    building_id: eval_args[1].as_str()?,
                    tile: [eval_args[2].as_i64()? as i32, eval_args[3].as_i64()? as i32],
                }
            }
            "queue_focus_camera" => {
                if eval_args.len() != 2 {
                    bail!("queue_focus_camera expects (tile_x, tile_z)");
                }
                TriggerAction::FocusCamera {
                    tile: [eval_args[0].as_i64()? as i32, eval_args[1].as_i64()? as i32],
                }
            }
            "queue_eva" | "queue_eva_transmission" => {
                if eval_args.len() < 2 {
                    bail!("queue_eva_transmission expects (speaker, text)");
                }
                TriggerAction::EvaTransmission {
                    speaker: eval_args[0].as_str()?,
                    text: eval_args[1].as_str()?,
                    portrait: eval_args
                        .get(2)
                        .and_then(|v| v.as_str().ok())
                        .filter(|s| !s.is_empty()),
                    audio_cue: eval_args
                        .get(3)
                        .and_then(|v| v.as_str().ok())
                        .filter(|s| !s.is_empty()),
                    radar_video: eval_args
                        .get(4)
                        .and_then(|v| v.as_str().ok())
                        .filter(|s| !s.is_empty()),
                }
            }
            "queue_set_objective" => {
                if eval_args.len() != 1 {
                    bail!("queue_set_objective expects (text)");
                }
                TriggerAction::SetObjective {
                    text: eval_args[0].as_str()?,
                }
            }
            "queue_set_weather" => {
                if eval_args.len() != 1 {
                    bail!("queue_set_weather expects (preset)");
                }
                TriggerAction::SetWeather {
                    preset: eval_args[0].as_str()?,
                }
            }
            "queue_declare_winner" => {
                if eval_args.len() != 1 {
                    bail!("queue_declare_winner expects (player_id)");
                }
                TriggerAction::DeclareWinner {
                    player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                }
            }
            "queue_reveal_area" => {
                if eval_args.len() != 3 {
                    bail!("queue_reveal_area expects (tile_x, tile_z, radius)");
                }
                TriggerAction::RevealArea {
                    tile: [eval_args[0].as_i64()? as i32, eval_args[1].as_i64()? as i32],
                    radius: eval_args[2].as_i64()? as i32,
                }
            }
            "queue_change_owner" => {
                if eval_args.len() < 2 || eval_args.len() > 3 {
                    bail!(
                        "queue_change_owner expects (from_player_id, to_player_id [, building_id])"
                    );
                }
                let building_id = if eval_args.len() == 3 && eval_args[2] != LuaValue::Nil {
                    let s = eval_args[2].as_str()?;
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                } else {
                    None
                };
                TriggerAction::ChangeOwner {
                    from_player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    to_player_id: eval_args[1].as_i64()?.clamp(0, 8) as u8,
                    building_id,
                }
            }
            "queue_change_building_owner" => {
                if eval_args.len() != 3 {
                    bail!(
                        "queue_change_building_owner expects (from_player_id, to_player_id, building_id)"
                    );
                }
                TriggerAction::ChangeOwner {
                    from_player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    to_player_id: eval_args[1].as_i64()?.clamp(0, 8) as u8,
                    building_id: Some(eval_args[2].as_str()?),
                }
            }
            "queue_fire_support_power" => {
                if eval_args.len() != 4 {
                    bail!("queue_fire_support_power expects (player_id, power_id, target_x, target_z)");
                }
                TriggerAction::FireSupportPower {
                    player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                    power_id: eval_args[1].as_str()?,
                    target_tile: [eval_args[2].as_i64()? as i32, eval_args[3].as_i64()? as i32],
                }
            }
            other => bail!("Unknown Lua world queue method '{other}'"),
        };
        ctx.queued_actions.push(action);
        Ok(LuaValue::Nil)
    }

    fn eval_builtin_call(&self, name: &str, args: &[LuaValue]) -> Result<Option<LuaValue>> {
        match name {
            "tostring" => {
                let v = args.first().unwrap_or(&LuaValue::Nil);
                Ok(Some(LuaValue::Str(v.to_lua_string())))
            }
            "tonumber" => {
                let v = args.first().unwrap_or(&LuaValue::Nil);
                let out = match v {
                    LuaValue::Int(_) | LuaValue::Float(_) => v.clone(),
                    LuaValue::Str(s) => {
                        let t = s.trim();
                        if let Ok(i) = t.parse::<i64>() {
                            LuaValue::Int(i)
                        } else if let Ok(f) = t.parse::<f64>() {
                            LuaValue::Float(f)
                        } else {
                            LuaValue::Nil
                        }
                    }
                    _ => LuaValue::Nil,
                };
                Ok(Some(out))
            }
            "type" => {
                let v = args.first().unwrap_or(&LuaValue::Nil);
                let ty = match v {
                    LuaValue::Nil => "nil",
                    LuaValue::Bool(_) => "boolean",
                    LuaValue::Int(_) | LuaValue::Float(_) => "number",
                    LuaValue::Str(_) => "string",
                    LuaValue::Table(_) => "table",
                };
                Ok(Some(LuaValue::Str(ty.to_string())))
            }
            "math.abs" | "abs" | "det_abs" | "math.det_abs" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                match &args[0] {
                    LuaValue::Int(i) => Ok(Some(LuaValue::Int(i.saturating_abs()))),
                    LuaValue::Float(f) => Ok(Some(LuaValue::Float(super::det_abs(*f)))),
                    other => bail!("{name} expects number, got {other:?}"),
                }
            }
            "math.floor" | "floor" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                match &args[0] {
                    LuaValue::Int(i) => Ok(Some(LuaValue::Int(*i))),
                    LuaValue::Float(f) => Ok(Some(LuaValue::Int(super::det_floor(*f)))),
                    other => bail!("{name} expects number, got {other:?}"),
                }
            }
            "math.ceil" | "ceil" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                match &args[0] {
                    LuaValue::Int(i) => Ok(Some(LuaValue::Int(*i))),
                    LuaValue::Float(f) => Ok(Some(LuaValue::Int(super::det_ceil(*f)))),
                    other => bail!("{name} expects number, got {other:?}"),
                }
            }
            "math.round" | "round" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                match &args[0] {
                    LuaValue::Int(i) => Ok(Some(LuaValue::Int(*i))),
                    LuaValue::Float(f) => Ok(Some(LuaValue::Int(super::det_round(*f)))),
                    other => bail!("{name} expects number, got {other:?}"),
                }
            }
            "math.sin" | "sin" | "det_sin" | "math.det_sin" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                Ok(Some(LuaValue::Float(super::det_sin(args[0].as_f64()?))))
            }
            "math.cos" | "cos" | "det_cos" | "math.det_cos" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                Ok(Some(LuaValue::Float(super::det_cos(args[0].as_f64()?))))
            }
            "math.sqrt" | "sqrt" | "det_sqrt" | "math.det_sqrt" => {
                if args.len() != 1 {
                    bail!("{name} expects 1 argument");
                }
                Ok(Some(LuaValue::Float(super::det_sqrt(args[0].as_f64()?))))
            }
            "math.atan2" | "atan2" | "det_atan2" | "math.det_atan2" | "math.atan" => {
                if args.len() == 2 {
                    Ok(Some(LuaValue::Float(super::det_atan2(
                        args[0].as_f64()?,
                        args[1].as_f64()?,
                    ))))
                } else if name == "math.atan" && args.len() == 1 {
                    Ok(Some(LuaValue::Float(super::det_atan2(
                        args[0].as_f64()?,
                        1.0,
                    ))))
                } else {
                    bail!("{name} expects (y, x) arguments")
                }
            }
            "det_hypot" | "hypot" | "math.hypot" | "math.det_hypot" => {
                if args.len() != 2 {
                    bail!("{name} expects (dx, dz)");
                }
                Ok(Some(LuaValue::Float(super::det_hypot(
                    args[0].as_f64()?,
                    args[1].as_f64()?,
                ))))
            }
            "det_dist" | "dist" | "math.dist" | "math.det_dist" => {
                if args.len() == 4 {
                    Ok(Some(LuaValue::Float(super::det_dist(
                        args[0].as_f64()?,
                        args[1].as_f64()?,
                        args[2].as_f64()?,
                        args[3].as_f64()?,
                    ))))
                } else if args.len() == 2 {
                    Ok(Some(LuaValue::Float(super::det_hypot(
                        args[0].as_f64()?,
                        args[1].as_f64()?,
                    ))))
                } else {
                    bail!("{name} expects (x1, z1, x2, z2) or (dx, dz)")
                }
            }
            "math.min" | "min" => {
                if args.is_empty() {
                    bail!("{name} expects at least 1 argument");
                }
                let mut best = args[0].clone();
                for a in &args[1..] {
                    if super::f64_to_fixed(a.as_f64()?) < super::f64_to_fixed(best.as_f64()?) {
                        best = a.clone();
                    }
                }
                Ok(Some(best))
            }
            "math.max" | "max" => {
                if args.is_empty() {
                    bail!("{name} expects at least 1 argument");
                }
                let mut best = args[0].clone();
                for a in &args[1..] {
                    if super::f64_to_fixed(a.as_f64()?) > super::f64_to_fixed(best.as_f64()?) {
                        best = a.clone();
                    }
                }
                Ok(Some(best))
            }
            "math.clamp" | "clamp" | "det_clamp" | "math.det_clamp" => {
                if args.len() != 3 {
                    bail!("{name} expects (val, min, max)");
                }
                if matches!(args[0], LuaValue::Int(_))
                    && matches!(args[1], LuaValue::Int(_))
                    && matches!(args[2], LuaValue::Int(_))
                {
                    let lo = args[1].as_i64()?;
                    let hi = args[2].as_i64()?;
                    let actual_lo = lo.min(hi);
                    let actual_hi = lo.max(hi);
                    Ok(Some(LuaValue::Int(
                        args[0].as_i64()?.clamp(actual_lo, actual_hi),
                    )))
                } else {
                    Ok(Some(LuaValue::Float(super::det_clamp(
                        args[0].as_f64()?,
                        args[1].as_f64()?,
                        args[2].as_f64()?,
                    ))))
                }
            }
            _ => Ok(None),
        }
    }

    fn call_user_function(&mut self, name: &str, args: Vec<LuaValue>) -> Result<LuaValue> {
        let all_fns = self.backend.all_functions();
        let func = all_fns
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("Unknown Lua function '{name}'"))?;
        if self.call_depth >= MAX_LUA_CALL_DEPTH {
            bail!("Lua call stack exceeded maximum depth ({MAX_LUA_CALL_DEPTH})");
        }
        self.call_depth += 1;
        let mut frame = BTreeMap::new();
        for (idx, param) in func.params.iter().enumerate() {
            let v = args.get(idx).cloned().unwrap_or(LuaValue::Nil);
            frame.insert(param.clone(), v);
        }
        self.scopes.push(frame);
        let res = self.exec_block(&func.body);
        self.scopes.pop();
        self.call_depth -= 1;
        match res? {
            FlowSignal::Return(v) => Ok(v),
            FlowSignal::None => Ok(LuaValue::Nil),
            FlowSignal::Break => bail!("'break' outside loop in Lua function '{name}'"),
        }
    }

    fn eval_expr(&mut self, expr: &LuaExpr) -> Result<LuaValue> {
        self.tick_op()?;
        match expr {
            LuaExpr::Nil => Ok(LuaValue::Nil),
            LuaExpr::Bool(b) => Ok(LuaValue::Bool(*b)),
            LuaExpr::Int(i) => Ok(LuaValue::Int(*i)),
            LuaExpr::Float(f) => Ok(LuaValue::Float(*f)),
            LuaExpr::Str(s) => Ok(LuaValue::Str(s.clone())),
            LuaExpr::Var(name) => Ok(self.get_var(name)),
            LuaExpr::Table(fields) => {
                if fields.len() > MAX_LUA_TABLE_ENTRIES {
                    bail!("Lua table exceeds maximum size ({MAX_LUA_TABLE_ENTRIES})");
                }
                let mut map = BTreeMap::new();
                let mut seq_idx: i64 = 1;
                for f in fields {
                    match f {
                        TableFieldInit::Named(k, ve) => {
                            let v = self.eval_expr(ve)?;
                            if v != LuaValue::Nil {
                                map.insert(k.clone(), v);
                            }
                        }
                        TableFieldInit::Indexed(ke, ve) => {
                            let k = self.eval_expr(ke)?.to_table_key()?;
                            let v = self.eval_expr(ve)?;
                            if v != LuaValue::Nil {
                                map.insert(k, v);
                            }
                        }
                        TableFieldInit::Positional(ve) => {
                            let v = self.eval_expr(ve)?;
                            if v != LuaValue::Nil {
                                map.insert(seq_idx.to_string(), v);
                            }
                            seq_idx += 1;
                        }
                    }
                    if map.len() > MAX_LUA_TABLE_ENTRIES {
                        bail!("Lua table exceeds maximum size ({MAX_LUA_TABLE_ENTRIES})");
                    }
                }
                Ok(LuaValue::Table(map))
            }
            LuaExpr::Field(base, field) => {
                if let LuaExpr::Var(vname) = base.as_ref() {
                    if self.is_ctx_var(vname) {
                        return self.read_ctx_field(field);
                    }
                }
                let base_val = self.eval_expr(base)?;
                if base_val == LuaValue::Str("__rts_ctx__".to_string()) {
                    return self.read_ctx_field(field);
                }
                match base_val {
                    LuaValue::Table(m) => Ok(m.get(field).cloned().unwrap_or(LuaValue::Nil)),
                    other => bail!("Cannot index field '{field}' on non-table {other:?}"),
                }
            }
            LuaExpr::Index(base, idx_expr) => {
                let idx_val = self.eval_expr(idx_expr)?;
                let key = idx_val.to_table_key()?;
                if let LuaExpr::Var(vname) = base.as_ref() {
                    if self.is_ctx_var(vname) {
                        return self.read_ctx_field(&key);
                    }
                }
                let base_val = self.eval_expr(base)?;
                if base_val == LuaValue::Str("__rts_ctx__".to_string()) {
                    return self.read_ctx_field(&key);
                }
                match base_val {
                    LuaValue::Table(m) => Ok(m.get(&key).cloned().unwrap_or(LuaValue::Nil)),
                    other => bail!("Cannot index [{key}] on non-table {other:?}"),
                }
            }
            LuaExpr::Unary(op, inner) => {
                let v = self.eval_expr(inner)?;
                match op {
                    UnOp::Not => Ok(LuaValue::Bool(!v.is_truthy())),
                    UnOp::Neg => match v {
                        LuaValue::Int(i) => Ok(LuaValue::Int(i.wrapping_neg())),
                        LuaValue::Float(f) => Ok(LuaValue::Float(-f)),
                        other => bail!("Unary '-' requires numeric Lua value, got {other:?}"),
                    },
                    UnOp::Len => match v {
                        LuaValue::Str(s) => Ok(LuaValue::Int(s.len() as i64)),
                        LuaValue::Table(m) => {
                            let mut n: i64 = 0;
                            while m.contains_key(&(n + 1).to_string()) {
                                n += 1;
                            }
                            Ok(LuaValue::Int(n))
                        }
                        other => bail!("Length '#' requires string or table, got {other:?}"),
                    },
                }
            }
            LuaExpr::Binary(op, lhs, rhs) => match op {
                BinOp::And => {
                    let l = self.eval_expr(lhs)?;
                    if !l.is_truthy() {
                        Ok(l)
                    } else {
                        self.eval_expr(rhs)
                    }
                }
                BinOp::Or => {
                    let l = self.eval_expr(lhs)?;
                    if l.is_truthy() {
                        Ok(l)
                    } else {
                        self.eval_expr(rhs)
                    }
                }
                _ => {
                    let l = self.eval_expr(lhs)?;
                    let r = self.eval_expr(rhs)?;
                    Self::eval_binary_values(op, l, r)
                }
            },
            LuaExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let is_ctx = match receiver.as_ref() {
                    LuaExpr::Var(vname) if self.is_ctx_var(vname) => true,
                    _ => self.eval_expr(receiver)? == LuaValue::Str("__rts_ctx__".to_string()),
                };
                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    eval_args.push(self.eval_expr(a)?);
                }
                if is_ctx {
                    self.exec_queue_method(method, eval_args)
                } else {
                    bail!("Method call ':{method}' is only supported on hook context")
                }
            }
            LuaExpr::Call { callee, args } => {
                // Support `ctx.queue_*(...)` dot-style calls as well
                if let LuaExpr::Field(base, method) = callee.as_ref() {
                    if let LuaExpr::Var(vname) = base.as_ref() {
                        if self.is_ctx_var(vname) {
                            let mut eval_args = Vec::with_capacity(args.len());
                            for a in args {
                                eval_args.push(self.eval_expr(a)?);
                            }
                            // Strip optional leading `ctx` self arg if passed as `ctx.queue_x(ctx, ...)`
                            if eval_args.first() == Some(&LuaValue::Str("__rts_ctx__".to_string()))
                            {
                                eval_args.remove(0);
                            }
                            return self.exec_queue_method(method, eval_args);
                        }
                        let dotted = format!("{vname}.{method}");
                        let mut eval_args = Vec::with_capacity(args.len());
                        for a in args {
                            eval_args.push(self.eval_expr(a)?);
                        }
                        if let Some(res) = self.eval_builtin_call(&dotted, &eval_args)? {
                            return Ok(res);
                        }
                        return self.call_user_function(&dotted, eval_args);
                    }
                }
                if let LuaExpr::Var(fn_name) = callee.as_ref() {
                    let mut eval_args = Vec::with_capacity(args.len());
                    for a in args {
                        eval_args.push(self.eval_expr(a)?);
                    }
                    if let Some(res) = self.eval_builtin_call(fn_name, &eval_args)? {
                        return Ok(res);
                    }
                    if matches!(
                        fn_name.as_str(),
                        "apply_buff"
                            | "spawn_projectile"
                            | "spawn_entity"
                            | "deal_area_damage"
                            | "issue_command"
                            | "query_entities_in_radius"
                            | "count_entities_in_radius"
                            | "query_player_resource"
                            | "query_unit_count"
                            | "query_building_count"
                    ) || fn_name.starts_with("queue_")
                    {
                        if eval_args.first() == Some(&LuaValue::Str("__rts_ctx__".to_string())) {
                            eval_args.remove(0);
                        }
                        return self.exec_queue_method(fn_name, eval_args);
                    }
                    return self.call_user_function(fn_name, eval_args);
                }
                bail!("Unsupported dynamic Lua callee: {callee:?}")
            }
        }
    }

    fn eval_binary_values(op: &BinOp, left: LuaValue, right: LuaValue) -> Result<LuaValue> {
        match op {
            BinOp::Add => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Int(a.wrapping_add(*b))),
                _ => Ok(LuaValue::Float(left.as_f64()? + right.as_f64()?)),
            },
            BinOp::Sub => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Int(a.wrapping_sub(*b))),
                _ => Ok(LuaValue::Float(left.as_f64()? - right.as_f64()?)),
            },
            BinOp::Mul => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Int(a.wrapping_mul(*b))),
                _ => Ok(LuaValue::Float(left.as_f64()? * right.as_f64()?)),
            },
            BinOp::Div => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => {
                    if *b == 0 {
                        bail!("Division by zero in Lua '/'");
                    }
                    if *a != i64::MIN && a % b == 0 {
                        Ok(LuaValue::Int(a / b))
                    } else {
                        Ok(LuaValue::Float(*a as f64 / *b as f64))
                    }
                }
                _ => {
                    let rf = right.as_f64()?;
                    if rf == 0.0 {
                        bail!("Division by zero in Lua '/'");
                    }
                    Ok(LuaValue::Float(left.as_f64()? / rf))
                }
            },
            BinOp::FloorDiv => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => {
                    if *b == 0 {
                        bail!("Division by zero in Lua '//'");
                    }
                    let q = a / b;
                    let r = a % b;
                    let floor_q = if (r > 0 && *b < 0) || (r < 0 && *b > 0) {
                        q - 1
                    } else {
                        q
                    };
                    Ok(LuaValue::Int(floor_q))
                }
                _ => {
                    let rf = right.as_f64()?;
                    if rf == 0.0 {
                        bail!("Division by zero in Lua '//'");
                    }
                    Ok(LuaValue::Float((left.as_f64()? / rf).floor()))
                }
            },
            BinOp::Mod => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => {
                    if *b == 0 {
                        bail!("Modulo by zero in Lua '%'");
                    }
                    let r = a % b;
                    let lua_mod = if (r > 0 && *b < 0) || (r < 0 && *b > 0) {
                        r + b
                    } else {
                        r
                    };
                    Ok(LuaValue::Int(lua_mod))
                }
                _ => {
                    let lf = left.as_f64()?;
                    let rf = right.as_f64()?;
                    if rf == 0.0 {
                        bail!("Modulo by zero in Lua '%'");
                    }
                    Ok(LuaValue::Float(lf - (lf / rf).floor() * rf))
                }
            },
            BinOp::Concat => {
                let s = format!("{}{}", left.to_lua_string(), right.to_lua_string());
                if s.len() > MAX_LUA_STRING_BYTES {
                    bail!(
                        "Lua concatenated string exceeds maximum size ({MAX_LUA_STRING_BYTES} bytes)"
                    );
                }
                Ok(LuaValue::Str(s))
            }
            BinOp::Eq => Ok(LuaValue::Bool(left == right)),
            BinOp::Ne => Ok(LuaValue::Bool(left != right)),
            BinOp::Lt => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Bool(a < b)),
                (LuaValue::Str(a), LuaValue::Str(b)) => Ok(LuaValue::Bool(a < b)),
                _ => Ok(LuaValue::Bool(left.as_f64()? < right.as_f64()?)),
            },
            BinOp::Le => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Bool(a <= b)),
                (LuaValue::Str(a), LuaValue::Str(b)) => Ok(LuaValue::Bool(a <= b)),
                _ => Ok(LuaValue::Bool(left.as_f64()? <= right.as_f64()?)),
            },
            BinOp::Gt => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Bool(a > b)),
                (LuaValue::Str(a), LuaValue::Str(b)) => Ok(LuaValue::Bool(a > b)),
                _ => Ok(LuaValue::Bool(left.as_f64()? > right.as_f64()?)),
            },
            BinOp::Ge => match (&left, &right) {
                (LuaValue::Int(a), LuaValue::Int(b)) => Ok(LuaValue::Bool(a >= b)),
                (LuaValue::Str(a), LuaValue::Str(b)) => Ok(LuaValue::Bool(a >= b)),
                _ => Ok(LuaValue::Bool(left.as_f64()? >= right.as_f64()?)),
            },
            BinOp::And | BinOp::Or => unreachable!("handled with short-circuiting"),
        }
    }

    fn assign_lvalue(&mut self, target: &LValue, val: LuaValue) -> Result<()> {
        match target {
            LValue::Var(name) => {
                if self.is_ctx_var(name) {
                    bail!("Cannot overwrite hook parameter '{name}' directly");
                }
                self.set_var(name, val);
                Ok(())
            }
            LValue::Field(base, field) => {
                if let LuaExpr::Var(vname) = base {
                    if self.is_ctx_var(vname) {
                        return self.write_ctx_field(field, val);
                    }
                    // Also support `ctx.vars.foo = 5`
                    if let LuaValue::Table(mut m) = self.get_var(vname) {
                        if val == LuaValue::Nil {
                            m.remove(field);
                        } else {
                            if m.len() >= MAX_LUA_TABLE_ENTRIES && !m.contains_key(field) {
                                bail!("Lua table exceeds maximum size ({MAX_LUA_TABLE_ENTRIES})");
                            }
                            m.insert(field.clone(), val);
                        }
                        self.set_var(vname, LuaValue::Table(m));
                        return Ok(());
                    }
                }
                if let LuaExpr::Field(outer, mid) = base {
                    if let LuaExpr::Var(vname) = outer.as_ref() {
                        if self.is_ctx_var(vname) && mid == "vars" {
                            return self.write_ctx_field(&format!("var_{field}"), val);
                        }
                    }
                }
                bail!("Unsupported nested field assignment '.{field}'")
            }
            LValue::Index(base, idx_expr) => {
                let key = self.eval_expr(idx_expr)?.to_table_key()?;
                if let LuaExpr::Var(vname) = base {
                    if self.is_ctx_var(vname) {
                        return self.write_ctx_field(&key, val);
                    }
                    if let LuaValue::Table(mut m) = self.get_var(vname) {
                        if val == LuaValue::Nil {
                            m.remove(&key);
                        } else {
                            if m.len() >= MAX_LUA_TABLE_ENTRIES && !m.contains_key(&key) {
                                bail!("Lua table exceeds maximum size ({MAX_LUA_TABLE_ENTRIES})");
                            }
                            m.insert(key, val);
                        }
                        self.set_var(vname, LuaValue::Table(m));
                        return Ok(());
                    }
                }
                if let LuaExpr::Field(outer, mid) = base {
                    if let LuaExpr::Var(vname) = outer.as_ref() {
                        if self.is_ctx_var(vname) && mid == "vars" {
                            return self.write_ctx_field(&format!("var_{key}"), val);
                        }
                    }
                }
                bail!("Unsupported indexed assignment '[{key}]'")
            }
        }
    }

    fn exec_block(&mut self, stmts: &[LuaStmt]) -> Result<FlowSignal> {
        self.scopes.push(BTreeMap::new());
        let res = self.exec_stmts_in_current_scope(stmts);
        self.scopes.pop();
        res
    }

    fn exec_stmts_in_current_scope(&mut self, stmts: &[LuaStmt]) -> Result<FlowSignal> {
        for stmt in stmts {
            self.tick_op()?;
            match stmt {
                LuaStmt::Local { names, values } => {
                    let mut eval_vals = Vec::with_capacity(values.len());
                    for v in values {
                        eval_vals.push(self.eval_expr(v)?);
                    }
                    for (idx, name) in names.iter().enumerate() {
                        let val = eval_vals.get(idx).cloned().unwrap_or(LuaValue::Nil);
                        self.define_local(name.clone(), val);
                    }
                }
                LuaStmt::Assign { targets, values } => {
                    let mut eval_vals = Vec::with_capacity(values.len());
                    for v in values {
                        eval_vals.push(self.eval_expr(v)?);
                    }
                    for (idx, target) in targets.iter().enumerate() {
                        let val = eval_vals.get(idx).cloned().unwrap_or(LuaValue::Nil);
                        self.assign_lvalue(target, val)?;
                    }
                }
                LuaStmt::ExprStmt(expr) => {
                    self.eval_expr(expr)?;
                }
                LuaStmt::If {
                    branches,
                    else_body,
                } => {
                    let mut matched = false;
                    for (cond, body) in branches {
                        if self.eval_expr(cond)?.is_truthy() {
                            matched = true;
                            let sig = self.exec_block(body)?;
                            if !matches!(sig, FlowSignal::None) {
                                return Ok(sig);
                            }
                            break;
                        }
                    }
                    if !matched && !else_body.is_empty() {
                        let sig = self.exec_block(else_body)?;
                        if !matches!(sig, FlowSignal::None) {
                            return Ok(sig);
                        }
                    }
                }
                LuaStmt::While { cond, body } => {
                    while self.eval_expr(cond)?.is_truthy() {
                        self.tick_op()?;
                        match self.exec_block(body)? {
                            FlowSignal::None => {}
                            FlowSignal::Break => break,
                            FlowSignal::Return(v) => return Ok(FlowSignal::Return(v)),
                        }
                    }
                }
                LuaStmt::Repeat { body, until_cond } => loop {
                    self.tick_op()?;
                    self.scopes.push(BTreeMap::new());
                    let sig = self.exec_stmts_in_current_scope(body);
                    let cond_done = match &sig {
                        Ok(FlowSignal::None) => self.eval_expr(until_cond)?.is_truthy(),
                        _ => false,
                    };
                    self.scopes.pop();
                    match sig? {
                        FlowSignal::None => {
                            if cond_done {
                                break;
                            }
                        }
                        FlowSignal::Break => break,
                        FlowSignal::Return(v) => return Ok(FlowSignal::Return(v)),
                    }
                },
                LuaStmt::ForNumeric {
                    var,
                    start,
                    stop,
                    step,
                    body,
                } => {
                    let start_v = self.eval_expr(start)?;
                    let stop_v = self.eval_expr(stop)?;
                    let step_v = match step {
                        Some(se) => self.eval_expr(se)?,
                        None => LuaValue::Int(1),
                    };
                    let all_ints = matches!(start_v, LuaValue::Int(_))
                        && matches!(stop_v, LuaValue::Int(_))
                        && matches!(step_v, LuaValue::Int(_));

                    if all_ints {
                        let mut cur = start_v.as_i64()?;
                        let end = stop_v.as_i64()?;
                        let st = step_v.as_i64()?;
                        if st == 0 {
                            bail!("Lua numeric 'for' step cannot be 0");
                        }
                        loop {
                            if (st > 0 && cur > end) || (st < 0 && cur < end) {
                                break;
                            }
                            self.tick_op()?;
                            self.scopes
                                .push(BTreeMap::from([(var.clone(), LuaValue::Int(cur))]));
                            let sig = self.exec_stmts_in_current_scope(body);
                            self.scopes.pop();
                            match sig? {
                                FlowSignal::None => {}
                                FlowSignal::Break => break,
                                FlowSignal::Return(v) => return Ok(FlowSignal::Return(v)),
                            }
                            let Some(next_cur) = cur.checked_add(st) else {
                                break;
                            };
                            cur = next_cur;
                        }
                    } else {
                        let mut cur = start_v.as_f64()?;
                        let end = stop_v.as_f64()?;
                        let st = step_v.as_f64()?;
                        if st == 0.0 {
                            bail!("Lua numeric 'for' step cannot be 0");
                        }
                        loop {
                            if (st > 0.0 && cur > end) || (st < 0.0 && cur < end) {
                                break;
                            }
                            self.tick_op()?;
                            self.scopes
                                .push(BTreeMap::from([(var.clone(), LuaValue::Float(cur))]));
                            let sig = self.exec_stmts_in_current_scope(body);
                            self.scopes.pop();
                            match sig? {
                                FlowSignal::None => {}
                                FlowSignal::Break => break,
                                FlowSignal::Return(v) => return Ok(FlowSignal::Return(v)),
                            }
                            cur += st;
                        }
                    }
                }
                LuaStmt::DoBlock(body) => {
                    let sig = self.exec_block(body)?;
                    if !matches!(sig, FlowSignal::None) {
                        return Ok(sig);
                    }
                }
                LuaStmt::Break => return Ok(FlowSignal::Break),
                LuaStmt::Return(opt_expr) => {
                    let v = match opt_expr {
                        Some(e) => self.eval_expr(e)?,
                        None => LuaValue::Nil,
                    };
                    return Ok(FlowSignal::Return(v));
                }
            }
        }
        Ok(FlowSignal::None)
    }
}

/// Deterministic, sandboxed pure-Rust Lua hook runtime for RTS mods.
/// Supports `local`, `if/elseif/else`, bounded `while`/`for`/`repeat` loops, helper functions,
/// tables, exact `i64` integer arithmetic, parenthesized expressions, `--` comments, and
/// isolated per-invocation global environments.
pub struct LuaBackend {
    hook_functions: BTreeMap<String, LuaFunction>,
    helper_functions: BTreeMap<String, LuaFunction>,
    base_globals: BTreeMap<String, LuaValue>,
}

impl Default for LuaBackend {
    fn default() -> Self {
        let mut math_tbl = BTreeMap::new();
        math_tbl.insert("pi".to_string(), LuaValue::Float(super::DET_PI));
        math_tbl.insert("tau".to_string(), LuaValue::Float(super::DET_TAU));
        let mut base_globals = BTreeMap::new();
        base_globals.insert("math".to_string(), LuaValue::Table(math_tbl));
        base_globals.insert("PI".to_string(), LuaValue::Float(super::DET_PI));
        base_globals.insert("TAU".to_string(), LuaValue::Float(super::DET_TAU));
        Self {
            hook_functions: BTreeMap::new(),
            helper_functions: BTreeMap::new(),
            base_globals,
        }
    }
}

impl LuaBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn all_functions(&self) -> BTreeMap<&str, &LuaFunction> {
        let mut out = BTreeMap::new();
        for (k, v) in &self.helper_functions {
            out.insert(k.as_str(), v);
        }
        for (k, v) in &self.hook_functions {
            out.insert(k.as_str(), v);
        }
        out
    }

    /// Executes a world-event hook with full 1:1 `WorldEventContext` parity and isolated globals.
    pub fn invoke_world_event(&self, hook_name: &str, ctx: &mut WorldEventContext) -> Result<()> {
        let func = self
            .hook_functions
            .get(hook_name)
            .ok_or_else(|| anyhow!("Lua world hook '{hook_name}' not found"))?;
        let ctx_var = func
            .params
            .first()
            .cloned()
            .unwrap_or_else(|| "ctx".to_string());
        let focus_x = ctx.camera_focus_tile.map(|t| t[0]);
        let focus_z = ctx.camera_focus_tile.map(|t| t[1]);
        let spawn_tx = ctx.spawn_tile.map(|t| t[0]);
        let spawn_tz = ctx.spawn_tile.map(|t| t[1]);
        let attack_x = ctx.attack_move_target.map(|p| p[0]);
        let attack_z = ctx.attack_move_target.map(|p| p[2]);
        let eva_speaker = ctx.eva_transmission.as_ref().map(|(s, _)| s.clone());
        let eva_text = ctx.eva_transmission.as_ref().map(|(_, t)| t.clone());

        let mut vm = LuaVm::new(
            self,
            HookTarget::World {
                ctx_var,
                ctx,
                focus_x,
                focus_z,
                spawn_tx,
                spawn_tz,
                attack_x,
                attack_z,
                eva_speaker,
                eva_text,
            },
        );
        match vm.exec_stmts_in_current_scope(&func.body)? {
            FlowSignal::Break => bail!("'break' outside loop in Lua world hook '{hook_name}'"),
            FlowSignal::None | FlowSignal::Return(_) => {}
        }
        if let HookTarget::World {
            ctx,
            focus_x,
            focus_z,
            spawn_tx,
            spawn_tz,
            attack_x,
            attack_z,
            eva_speaker,
            eva_text,
            ..
        } = vm.target
        {
            if let (Some(tx), Some(tz)) = (spawn_tx, spawn_tz) {
                ctx.spawn_tile = Some([tx, tz]);
            }
            if let (Some(ax), Some(az)) = (attack_x, attack_z) {
                ctx.attack_move_target = Some([ax, 0.0, az]);
            }
            if let (Some(fx), Some(fz)) = (focus_x, focus_z) {
                ctx.camera_focus_tile = Some([fx, fz]);
            }
            if let Some(text) = eva_text {
                if !text.is_empty() {
                    let speaker = eva_speaker.unwrap_or_else(|| "EVA".to_string());
                    ctx.eva_transmission = Some((speaker, text));
                }
            }
        }
        Ok(())
    }
}

impl ScriptBackend for LuaBackend {
    fn language(&self) -> ScriptLang {
        ScriptLang::Lua
    }

    fn load_script(&mut self, name: &str, source: &str) -> Result<Vec<String>> {
        let tokens = tokenize(source).with_context(|| format!("In Lua script '{name}'"))?;
        let mut parser = Parser::new(tokens);
        let mut hooks = Vec::new();
        let mut top_level_stmts = Vec::new();

        while parser.peek() != &Token::Eof {
            parser.skip_semicolons();
            if parser.peek() == &Token::Eof {
                break;
            }
            if parser.eat(&Token::Function) {
                let (fn_name, fn_def) = parser
                    .parse_function_def()
                    .with_context(|| format!("In Lua script '{name}'"))?;
                hooks.push(fn_name.clone());
                self.hook_functions.insert(fn_name, fn_def);
            } else if parser.peek() == &Token::Local
                && parser.tokens.get(parser.pos + 1) == Some(&Token::Function)
            {
                parser.next_tok(); // local
                parser.next_tok(); // function
                let (fn_name, fn_def) = parser
                    .parse_function_def()
                    .with_context(|| format!("In Lua script '{name}'"))?;
                self.helper_functions.insert(fn_name, fn_def);
            } else {
                let stmt = parser
                    .parse_stmt()
                    .with_context(|| format!("In top-level Lua statement of '{name}'"))?;
                top_level_stmts.push(stmt);
            }
        }

        if !top_level_stmts.is_empty() {
            let mut init_vm = LuaVm::new(self, HookTarget::None);
            init_vm
                .exec_stmts_in_current_scope(&top_level_stmts)
                .with_context(|| format!("Evaluating top-level declarations in '{name}'"))?;
            let mut merged = init_vm.globals;
            if let Some(top_locals) = init_vm.scopes.into_iter().next() {
                for (k, v) in top_locals {
                    merged.insert(k, v);
                }
            }
            self.base_globals = merged;
        }

        Ok(hooks)
    }

    fn has_hook(&self, hook_name: &str) -> bool {
        self.hook_functions.contains_key(hook_name)
    }

    fn invoke_hook(&self, hook_name: &str, ctx: &mut ScriptContext) -> Result<()> {
        let func = self
            .hook_functions
            .get(hook_name)
            .ok_or_else(|| anyhow!("Lua hook '{hook_name}' not found"))?;
        let ctx_var = func
            .params
            .first()
            .cloned()
            .unwrap_or_else(|| "ctx".to_string());
        let mut vm = LuaVm::new(self, HookTarget::Unit { ctx_var, ctx });
        match vm.exec_stmts_in_current_scope(&func.body)? {
            FlowSignal::Break => bail!("'break' outside loop in Lua hook '{hook_name}'"),
            FlowSignal::None | FlowSignal::Return(_) => {}
        }
        Ok(())
    }
}
