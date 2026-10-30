use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Hard security & resource limits for sandboxed Lua transformation scripts.
pub const MAX_LUA_SCRIPT_BYTES: usize = 65_536;
pub const MAX_LUA_TOKENS: usize = 10_000;
pub const MAX_LUA_GAS: usize = 50_000;
pub const MAX_LUA_CALL_DEPTH: usize = 24;
pub const MAX_LUA_EXPR_DEPTH: usize = 48;
pub const MAX_LUA_STRING_BYTES: usize = 16_384;
pub const MAX_LUA_TABLE_ENTRIES: usize = 1_024;
pub const MAX_LUA_STDOUT_LINES: usize = 200;

/// Pre-validates a user or rule Lua script for forbidden host/filesystem/OS/process APIs,
/// script size limits, and token limits before execution.
pub fn validate_lua_sandbox_security(script: &str) -> Result<(), String> {
  if script.len() > MAX_LUA_SCRIPT_BYTES {
    return Err(format!(
      "[Lua Sandbox Security Violation] Script size ({} bytes) exceeds maximum allowed {} bytes.",
      script.len(),
      MAX_LUA_SCRIPT_BYTES
    ));
  }

  let mut lexer = Lexer::new(script);
  let mut token_count = 0usize;
  loop {
    let tok = lexer.next_token()?;
    token_count += 1;
    if token_count > MAX_LUA_TOKENS {
      return Err(format!(
        "[Lua Sandbox Security Violation] Script token count exceeds maximum allowed {} tokens.",
        MAX_LUA_TOKENS
      ));
    }
    match &tok {
      Token::Eof => break,
      Token::StringLit(s) => {
        if s.len() > MAX_LUA_STRING_BYTES {
          return Err(format!(
            "[Lua Sandbox Security Violation] String literal ({} bytes) exceeds {} byte limit.",
            s.len(),
            MAX_LUA_STRING_BYTES
          ));
        }
      }
      Token::Ident(id) => match id.as_str() {
        "io" => {
          return Err(
            "[Lua Sandbox Security Violation] Filesystem access ('io.*') is strictly forbidden in sandboxed transformations."
              .to_string(),
          );
        }
        "os" => {
          return Err(
            "[Lua Sandbox Security Violation] Operating System / Process calls ('os.*', execute/fork) are strictly forbidden in sandboxed transformations."
              .to_string(),
          );
        }
        "dofile" | "loadfile" => {
          return Err(format!(
            "[Lua Sandbox Security Violation] External file execution ('{}') is strictly forbidden in sandboxed transformations.",
            id
          ));
        }
        "load" | "loadstring" => {
          return Err(format!(
            "[Lua Sandbox Security Violation] Dynamic bytecode/string evaluation ('{}') is strictly forbidden in sandboxed transformations.",
            id
          ));
        }
        "require" | "package" => {
          return Err(format!(
            "[Lua Sandbox Security Violation] External module loading ('{}') is strictly forbidden in sandboxed transformations.",
            id
          ));
        }
        "debug" => {
          return Err(
            "[Lua Sandbox Security Violation] VM memory/stack introspection ('debug.*') is strictly forbidden in sandboxed transformations."
              .to_string(),
          );
        }
        "coroutine" => {
          return Err(
            "[Lua Sandbox Security Violation] Coroutine spawning ('coroutine.*') is strictly forbidden in sandboxed transformations."
              .to_string(),
          );
        }
        "setfenv" | "getfenv" | "rawget" | "rawset" | "collectgarbage" => {
          return Err(format!(
            "[Lua Sandbox Security Violation] Sandbox environment tampering ('{}') is strictly forbidden.",
            id
          ));
        }
        _ => {}
      },
      _ => {}
    }
  }

  Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub enum LuaVal {
  Nil,
  Boolean(bool),
  Number(f64),
  String(String),
  Table(Rc<RefCell<HashMap<String, LuaVal>>>),
  Function(String, Vec<String>, Vec<Stmt>),
  NativeFn(String),
}

impl LuaVal {
  pub fn is_truthy(&self) -> bool {
    match self {
      LuaVal::Nil => false,
      LuaVal::Boolean(b) => *b,
      _ => true,
    }
  }

  pub fn to_lua_string(&self) -> String {
    match self {
      LuaVal::Nil => "nil".to_string(),
      LuaVal::Boolean(b) => b.to_string(),
      LuaVal::Number(n) => {
        if n.fract() == 0.0 {
          format!("{:.0}", n)
        } else {
          format!("{}", n)
        }
      }
      LuaVal::String(s) => s.clone(),
      LuaVal::Table(t) => {
        let map = t.borrow();
        let mut entries = Vec::new();
        for (k, v) in map.iter() {
          entries.push(format!("[\"{}\"] = {}", k, v.to_lua_string()));
        }
        format!("{{ {} }}", entries.join(", "))
      }
      LuaVal::Function(name, _, _) => format!("<function {}>", name),
      LuaVal::NativeFn(name) => format!("<native {}>", name),
    }
  }

  pub fn to_json(&self) -> serde_json::Value {
    match self {
      LuaVal::Nil => serde_json::Value::Null,
      LuaVal::Boolean(b) => serde_json::Value::Bool(*b),
      LuaVal::Number(n) => {
        if let Some(i) = serde_json::Number::from_f64(*n) {
          serde_json::Value::Number(i)
        } else {
          serde_json::Value::Null
        }
      }
      LuaVal::String(s) => serde_json::Value::String(s.clone()),
      LuaVal::Table(t) => {
        let map = t.borrow();
        // Check if it is an array (keys "1", "2", ...)
        let mut is_array = !map.is_empty();
        for i in 1..=map.len() {
          if !map.contains_key(&i.to_string()) {
            is_array = false;
            break;
          }
        }
        if is_array {
          let mut arr = Vec::new();
          for i in 1..=map.len() {
            if let Some(val) = map.get(&i.to_string()) {
              arr.push(val.to_json());
            }
          }
          serde_json::Value::Array(arr)
        } else {
          let mut obj = serde_json::Map::new();
          for (k, v) in map.iter() {
            obj.insert(k.clone(), v.to_json());
          }
          serde_json::Value::Object(obj)
        }
      }
      _ => serde_json::Value::Null,
    }
  }

  pub fn from_json(val: &serde_json::Value) -> LuaVal {
    match val {
      serde_json::Value::Null => LuaVal::Nil,
      serde_json::Value::Bool(b) => LuaVal::Boolean(*b),
      serde_json::Value::Number(n) => LuaVal::Number(n.as_f64().unwrap_or(0.0)),
      serde_json::Value::String(s) => LuaVal::String(s.clone()),
      serde_json::Value::Array(arr) => {
        let mut map = HashMap::new();
        for (idx, item) in arr.iter().enumerate() {
          map.insert((idx + 1).to_string(), LuaVal::from_json(item));
        }
        LuaVal::Table(Rc::new(RefCell::new(map)))
      }
      serde_json::Value::Object(obj) => {
        let mut map = HashMap::new();
        for (k, v) in obj.iter() {
          map.insert(k.clone(), LuaVal::from_json(v));
        }
        LuaVal::Table(Rc::new(RefCell::new(map)))
      }
    }
  }
}

impl fmt::Display for LuaVal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.to_lua_string())
  }
}

// AST definitions
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
  Literal(LuaVal),
  Variable(String),
  TableIndex(Box<Expr>, Box<Expr>),
  TableField(Box<Expr>, String),
  TableConstructor(Vec<(Option<Expr>, Expr)>),
  UnaryOp(UnaryOp, Box<Expr>),
  BinaryOp(BinaryOp, Box<Expr>, Box<Expr>),
  FunctionCall(Box<Expr>, Vec<Expr>),
  FunctionDef(Vec<String>, Vec<Stmt>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnaryOp {
  Not,
  Negate,
  Length,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BinaryOp {
  Add,
  Sub,
  Mul,
  Div,
  Mod,
  Concat,
  Eq,
  Neq,
  Lt,
  Lte,
  Gt,
  Gte,
  And,
  Or,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
  Assign(Vec<Expr>, Vec<Expr>),
  LocalAssign(Vec<String>, Vec<Expr>),
  FunctionDef(String, Vec<String>, Vec<Stmt>),
  FunctionCall(Expr),
  If {
    cond: Expr,
    then_block: Vec<Stmt>,
    elseif_blocks: Vec<(Expr, Vec<Stmt>)>,
    else_block: Option<Vec<Stmt>>,
  },
  While {
    cond: Expr,
    body: Vec<Stmt>,
  },
  ForNumeric {
    var: String,
    start: Expr,
    stop: Expr,
    step: Option<Expr>,
    body: Vec<Stmt>,
  },
  ForGeneric {
    vars: Vec<String>,
    iter_expr: Expr,
    body: Vec<Stmt>,
  },
  Return(Vec<Expr>),
}

// Lexer
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
  Ident(String),
  Number(f64),
  StringLit(String),
  Function,
  End,
  If,
  Then,
  ElseIf,
  Else,
  While,
  Do,
  For,
  In,
  Return,
  Local,
  And,
  Or,
  Not,
  Nil,
  True,
  False,
  Plus,
  Minus,
  Star,
  Slash,
  Percent,
  DotDot,
  Dot,
  Colon,
  Comma,
  Semicolon,
  Eq,
  EqEq,
  TildeEq,
  Lt,
  Lte,
  Gt,
  Gte,
  LParen,
  RParen,
  LBracket,
  RBracket,
  LBrace,
  RBrace,
  Hash,
  Eof,
}

pub struct Lexer<'a> {
  #[allow(dead_code)]
  input: &'a str,
  chars: Vec<char>,
  pos: usize,
}

impl<'a> Lexer<'a> {
  pub fn new(input: &'a str) -> Self {
    Self {
      input,
      chars: input.chars().collect(),
      pos: 0,
    }
  }

  fn peek(&self) -> Option<char> {
    self.chars.get(self.pos).copied()
  }

  fn advance(&mut self) -> Option<char> {
    let ch = self.chars.get(self.pos).copied();
    if ch.is_some() {
      self.pos += 1;
    }
    ch
  }

  fn skip_whitespace_and_comments(&mut self) {
    while let Some(ch) = self.peek() {
      if ch.is_whitespace() {
        self.advance();
      } else if ch == '-' && self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '-' {
        // Comment
        self.advance();
        self.advance();
        while let Some(c) = self.advance() {
          if c == '\n' {
            break;
          }
        }
      } else {
        break;
      }
    }
  }

  pub fn next_token(&mut self) -> Result<Token, String> {
    self.skip_whitespace_and_comments();
    let ch = match self.advance() {
      Some(c) => c,
      None => return Ok(Token::Eof),
    };

    match ch {
      '+' => Ok(Token::Plus),
      '-' => Ok(Token::Minus),
      '*' => Ok(Token::Star),
      '/' => Ok(Token::Slash),
      '%' => Ok(Token::Percent),
      '#' => Ok(Token::Hash),
      '(' => Ok(Token::LParen),
      ')' => Ok(Token::RParen),
      '[' => Ok(Token::LBracket),
      ']' => Ok(Token::RBracket),
      '{' => Ok(Token::LBrace),
      '}' => Ok(Token::RBrace),
      ';' => Ok(Token::Semicolon),
      ',' => Ok(Token::Comma),
      ':' => Ok(Token::Colon),
      '.' => {
        if self.peek() == Some('.') {
          self.advance();
          Ok(Token::DotDot)
        } else {
          Ok(Token::Dot)
        }
      }
      '=' => {
        if self.peek() == Some('=') {
          self.advance();
          Ok(Token::EqEq)
        } else {
          Ok(Token::Eq)
        }
      }
      '~' => {
        if self.peek() == Some('=') {
          self.advance();
          Ok(Token::TildeEq)
        } else {
          Err("Unexpected '~', did you mean '~='?".to_string())
        }
      }
      '<' => {
        if self.peek() == Some('=') {
          self.advance();
          Ok(Token::Lte)
        } else {
          Ok(Token::Lt)
        }
      }
      '>' => {
        if self.peek() == Some('=') {
          self.advance();
          Ok(Token::Gte)
        } else {
          Ok(Token::Gt)
        }
      }
      '"' | '\'' => {
        let quote = ch;
        let mut s = String::new();
        while let Some(c) = self.advance() {
          if c == quote {
            return Ok(Token::StringLit(s));
          } else if c == '\\' {
            if let Some(esc) = self.advance() {
              match esc {
                'n' => s.push('\n'),
                'r' => s.push('\r'),
                't' => s.push('\t'),
                '\\' => s.push('\\'),
                '"' => s.push('"'),
                '\'' => s.push('\''),
                _ => {
                  s.push('\\');
                  s.push(esc);
                }
              }
            }
          } else {
            s.push(c);
          }
        }
        Err("Unterminated string literal".to_string())
      }
      c if c.is_ascii_digit() => {
        let mut num_str = c.to_string();
        let mut seen_dot = false;
        while let Some(p) = self.peek() {
          if p.is_ascii_digit() {
            num_str.push(self.advance().unwrap());
          } else if p == '.'
            && !seen_dot
            && self.pos + 1 < self.chars.len()
            && self.chars[self.pos + 1] != '.'
          {
            seen_dot = true;
            num_str.push(self.advance().unwrap());
          } else {
            break;
          }
        }
        let n: f64 = num_str
          .parse()
          .map_err(|e| format!("Invalid number: {}", e))?;
        Ok(Token::Number(n))
      }
      c if c.is_alphanumeric() || c == '_' => {
        let mut ident = c.to_string();
        while let Some(p) = self.peek() {
          if p.is_alphanumeric() || p == '_' {
            ident.push(self.advance().unwrap());
          } else {
            break;
          }
        }
        match ident.as_str() {
          "function" => Ok(Token::Function),
          "end" => Ok(Token::End),
          "if" => Ok(Token::If),
          "then" => Ok(Token::Then),
          "elseif" => Ok(Token::ElseIf),
          "else" => Ok(Token::Else),
          "while" => Ok(Token::While),
          "do" => Ok(Token::Do),
          "for" => Ok(Token::For),
          "in" => Ok(Token::In),
          "return" => Ok(Token::Return),
          "local" => Ok(Token::Local),
          "and" => Ok(Token::And),
          "or" => Ok(Token::Or),
          "not" => Ok(Token::Not),
          "nil" => Ok(Token::Nil),
          "true" => Ok(Token::True),
          "false" => Ok(Token::False),
          _ => Ok(Token::Ident(ident)),
        }
      }
      _ => Err(format!("Unexpected character: {}", ch)),
    }
  }
}

// Parser
pub struct Parser {
  tokens: Vec<Token>,
  pos: usize,
}

impl Parser {
  pub fn new(mut lexer: Lexer) -> Result<Self, String> {
    let mut tokens = Vec::new();
    loop {
      let tok = lexer.next_token()?;
      let is_eof = tok == Token::Eof;
      tokens.push(tok);
      if is_eof {
        break;
      }
    }
    Ok(Self { tokens, pos: 0 })
  }

  fn peek(&self) -> &Token {
    self.tokens.get(self.pos).unwrap_or(&Token::Eof)
  }

  fn advance(&mut self) -> Token {
    let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
    if self.pos < self.tokens.len() {
      self.pos += 1;
    }
    tok
  }

  fn match_token(&mut self, expected: &Token) -> bool {
    if self.peek() == expected {
      self.advance();
      true
    } else {
      false
    }
  }

  fn expect(&mut self, expected: &Token) -> Result<(), String> {
    if self.peek() == expected {
      self.advance();
      Ok(())
    } else {
      Err(format!("Expected {:?}, got {:?}", expected, self.peek()))
    }
  }

  pub fn parse_chunk(&mut self) -> Result<Vec<Stmt>, String> {
    let mut stmts = Vec::new();
    while self.peek() != &Token::Eof
      && !matches!(self.peek(), Token::End | Token::ElseIf | Token::Else)
    {
      stmts.push(self.parse_stmt()?);
      while self.match_token(&Token::Semicolon) {}
    }
    Ok(stmts)
  }

  fn parse_stmt(&mut self) -> Result<Stmt, String> {
    match self.peek() {
      Token::Local => {
        self.advance();
        if self.match_token(&Token::Function) {
          let name = match self.advance() {
            Token::Ident(n) => n,
            _ => return Err("Expected function name".to_string()),
          };
          let (params, body) = self.parse_function_body()?;
          Ok(Stmt::FunctionDef(name, params, body))
        } else {
          let mut names = Vec::new();
          loop {
            match self.advance() {
              Token::Ident(n) => names.push(n),
              _ => return Err("Expected identifier in local declaration".to_string()),
            }
            if !self.match_token(&Token::Comma) {
              break;
            }
          }
          let mut values = Vec::new();
          if self.match_token(&Token::Eq) {
            loop {
              values.push(self.parse_expr()?);
              if !self.match_token(&Token::Comma) {
                break;
              }
            }
          }
          Ok(Stmt::LocalAssign(names, values))
        }
      }
      Token::Function => {
        self.advance();
        let name = match self.advance() {
          Token::Ident(n) => n,
          _ => return Err("Expected function name".to_string()),
        };
        let (params, body) = self.parse_function_body()?;
        Ok(Stmt::FunctionDef(name, params, body))
      }
      Token::If => {
        self.advance();
        let cond = self.parse_expr()?;
        self.expect(&Token::Then)?;
        let then_block = self.parse_chunk()?;
        let mut elseif_blocks = Vec::new();
        while self.match_token(&Token::ElseIf) {
          let elif_cond = self.parse_expr()?;
          self.expect(&Token::Then)?;
          let elif_body = self.parse_chunk()?;
          elseif_blocks.push((elif_cond, elif_body));
        }
        let else_block = if self.match_token(&Token::Else) {
          Some(self.parse_chunk()?)
        } else {
          None
        };
        self.expect(&Token::End)?;
        Ok(Stmt::If {
          cond,
          then_block,
          elseif_blocks,
          else_block,
        })
      }
      Token::While => {
        self.advance();
        let cond = self.parse_expr()?;
        self.expect(&Token::Do)?;
        let body = self.parse_chunk()?;
        self.expect(&Token::End)?;
        Ok(Stmt::While { cond, body })
      }
      Token::For => {
        self.advance();
        let first_var = match self.advance() {
          Token::Ident(n) => n,
          _ => return Err("Expected variable name after 'for'".to_string()),
        };
        if self.match_token(&Token::Eq) {
          let start = self.parse_expr()?;
          self.expect(&Token::Comma)?;
          let stop = self.parse_expr()?;
          let step = if self.match_token(&Token::Comma) {
            Some(self.parse_expr()?)
          } else {
            None
          };
          self.expect(&Token::Do)?;
          let body = self.parse_chunk()?;
          self.expect(&Token::End)?;
          Ok(Stmt::ForNumeric {
            var: first_var,
            start,
            stop,
            step,
            body,
          })
        } else {
          let mut vars = vec![first_var];
          while self.match_token(&Token::Comma) {
            match self.advance() {
              Token::Ident(n) => vars.push(n),
              _ => return Err("Expected identifier in for loop".to_string()),
            }
          }
          self.expect(&Token::In)?;
          let iter_expr = self.parse_expr()?;
          self.expect(&Token::Do)?;
          let body = self.parse_chunk()?;
          self.expect(&Token::End)?;
          Ok(Stmt::ForGeneric {
            vars,
            iter_expr,
            body,
          })
        }
      }
      Token::Return => {
        self.advance();
        let mut exprs = Vec::new();
        if !matches!(
          self.peek(),
          Token::End | Token::ElseIf | Token::Else | Token::Semicolon | Token::Eof
        ) {
          loop {
            exprs.push(self.parse_expr()?);
            if !self.match_token(&Token::Comma) {
              break;
            }
          }
        }
        Ok(Stmt::Return(exprs))
      }
      _ => {
        let first_expr = self.parse_suffixed_expr()?;
        if self.peek() == &Token::Comma || self.peek() == &Token::Eq {
          let mut targets = vec![first_expr];
          while self.match_token(&Token::Comma) {
            targets.push(self.parse_suffixed_expr()?);
          }
          self.expect(&Token::Eq)?;
          let mut values = Vec::new();
          loop {
            values.push(self.parse_expr()?);
            if !self.match_token(&Token::Comma) {
              break;
            }
          }
          Ok(Stmt::Assign(targets, values))
        } else {
          Ok(Stmt::FunctionCall(first_expr))
        }
      }
    }
  }

  fn parse_function_body(&mut self) -> Result<(Vec<String>, Vec<Stmt>), String> {
    self.expect(&Token::LParen)?;
    let mut params = Vec::new();
    if self.peek() != &Token::RParen {
      loop {
        match self.advance() {
          Token::Ident(n) => params.push(n),
          _ => return Err("Expected parameter name".to_string()),
        }
        if !self.match_token(&Token::Comma) {
          break;
        }
      }
    }
    self.expect(&Token::RParen)?;
    let body = self.parse_chunk()?;
    self.expect(&Token::End)?;
    Ok((params, body))
  }

  pub fn parse_expr(&mut self) -> Result<Expr, String> {
    self.parse_or()
  }

  fn parse_or(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_and()?;
    while self.match_token(&Token::Or) {
      let right = self.parse_and()?;
      expr = Expr::BinaryOp(BinaryOp::Or, Box::new(expr), Box::new(right));
    }
    Ok(expr)
  }

  fn parse_and(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_comparison()?;
    while self.match_token(&Token::And) {
      let right = self.parse_comparison()?;
      expr = Expr::BinaryOp(BinaryOp::And, Box::new(expr), Box::new(right));
    }
    Ok(expr)
  }

  fn parse_comparison(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_concat()?;
    loop {
      let op = match self.peek() {
        Token::EqEq => BinaryOp::Eq,
        Token::TildeEq => BinaryOp::Neq,
        Token::Lt => BinaryOp::Lt,
        Token::Lte => BinaryOp::Lte,
        Token::Gt => BinaryOp::Gt,
        Token::Gte => BinaryOp::Gte,
        _ => break,
      };
      self.advance();
      let right = self.parse_concat()?;
      expr = Expr::BinaryOp(op, Box::new(expr), Box::new(right));
    }
    Ok(expr)
  }

  fn parse_concat(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_additive()?;
    if self.match_token(&Token::DotDot) {
      let right = self.parse_concat()?; // Right-associative
      expr = Expr::BinaryOp(BinaryOp::Concat, Box::new(expr), Box::new(right));
    }
    Ok(expr)
  }

  fn parse_additive(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_multiplicative()?;
    while let Some(op) = match self.peek() {
      Token::Plus => Some(BinaryOp::Add),
      Token::Minus => Some(BinaryOp::Sub),
      _ => None,
    } {
      self.advance();
      let right = self.parse_multiplicative()?;
      expr = Expr::BinaryOp(op, Box::new(expr), Box::new(right));
    }
    Ok(expr)
  }

  fn parse_multiplicative(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_unary()?;
    while let Some(op) = match self.peek() {
      Token::Star => Some(BinaryOp::Mul),
      Token::Slash => Some(BinaryOp::Div),
      Token::Percent => Some(BinaryOp::Mod),
      _ => None,
    } {
      self.advance();
      let right = self.parse_unary()?;
      expr = Expr::BinaryOp(op, Box::new(expr), Box::new(right));
    }
    Ok(expr)
  }

  fn parse_unary(&mut self) -> Result<Expr, String> {
    if self.match_token(&Token::Not) {
      let expr = self.parse_unary()?;
      Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(expr)))
    } else if self.match_token(&Token::Minus) {
      let expr = self.parse_unary()?;
      Ok(Expr::UnaryOp(UnaryOp::Negate, Box::new(expr)))
    } else if self.match_token(&Token::Hash) {
      let expr = self.parse_unary()?;
      Ok(Expr::UnaryOp(UnaryOp::Length, Box::new(expr)))
    } else {
      self.parse_suffixed_expr()
    }
  }

  fn parse_suffixed_expr(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_primary()?;
    loop {
      match self.peek() {
        Token::Dot => {
          self.advance();
          match self.advance() {
            Token::Ident(field) => {
              expr = Expr::TableField(Box::new(expr), field);
            }
            _ => return Err("Expected identifier after '.'".to_string()),
          }
        }
        Token::LBracket => {
          self.advance();
          let index = self.parse_expr()?;
          self.expect(&Token::RBracket)?;
          expr = Expr::TableIndex(Box::new(expr), Box::new(index));
        }
        Token::LParen => {
          self.advance();
          let mut args = Vec::new();
          if self.peek() != &Token::RParen {
            loop {
              args.push(self.parse_expr()?);
              if !self.match_token(&Token::Comma) {
                break;
              }
            }
          }
          self.expect(&Token::RParen)?;
          expr = Expr::FunctionCall(Box::new(expr), args);
        }
        Token::Colon => {
          // Method call: foo:bar(x) -> foo.bar(foo, x)
          self.advance();
          let method = match self.advance() {
            Token::Ident(m) => m,
            _ => return Err("Expected method name after ':'".to_string()),
          };
          self.expect(&Token::LParen)?;
          let mut args = vec![expr.clone()];
          if self.peek() != &Token::RParen {
            loop {
              args.push(self.parse_expr()?);
              if !self.match_token(&Token::Comma) {
                break;
              }
            }
          }
          self.expect(&Token::RParen)?;
          let func_expr = Expr::TableField(Box::new(expr), method);
          expr = Expr::FunctionCall(Box::new(func_expr), args);
        }
        _ => break,
      }
    }
    Ok(expr)
  }

  fn parse_primary(&mut self) -> Result<Expr, String> {
    match self.peek() {
      Token::Nil => {
        self.advance();
        Ok(Expr::Literal(LuaVal::Nil))
      }
      Token::True => {
        self.advance();
        Ok(Expr::Literal(LuaVal::Boolean(true)))
      }
      Token::False => {
        self.advance();
        Ok(Expr::Literal(LuaVal::Boolean(false)))
      }
      Token::Number(n) => {
        let val = *n;
        self.advance();
        Ok(Expr::Literal(LuaVal::Number(val)))
      }
      Token::StringLit(s) => {
        let val = s.clone();
        self.advance();
        Ok(Expr::Literal(LuaVal::String(val)))
      }
      Token::Ident(name) => {
        let n = name.clone();
        self.advance();
        Ok(Expr::Variable(n))
      }
      Token::LParen => {
        self.advance();
        let expr = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(expr)
      }
      Token::LBrace => {
        self.advance();
        let mut fields = Vec::new();
        let mut implicit_idx = 1;
        while self.peek() != &Token::RBrace && self.peek() != &Token::Eof {
          if self.match_token(&Token::LBracket) {
            let key = self.parse_expr()?;
            self.expect(&Token::RBracket)?;
            self.expect(&Token::Eq)?;
            let val = self.parse_expr()?;
            fields.push((Some(key), val));
          } else if let (Token::Ident(_), Token::Eq) = (
            self.peek(),
            self.tokens.get(self.pos + 1).unwrap_or(&Token::Eof),
          ) {
            let key_name = match self.advance() {
              Token::Ident(n) => n,
              _ => unreachable!(),
            };
            self.advance(); // skip Eq
            let val = self.parse_expr()?;
            fields.push((Some(Expr::Literal(LuaVal::String(key_name))), val));
          } else {
            let val = self.parse_expr()?;
            fields.push((
              Some(Expr::Literal(LuaVal::Number(implicit_idx as f64))),
              val,
            ));
            implicit_idx += 1;
          }
          if !self.match_token(&Token::Comma) && !self.match_token(&Token::Semicolon) {
            break;
          }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::TableConstructor(fields))
      }
      Token::Function => {
        self.advance();
        let (params, body) = self.parse_function_body()?;
        Ok(Expr::FunctionDef(params, body))
      }
      _ => Err(format!("Unexpected token in expression: {:?}", self.peek())),
    }
  }
}

// Runtime Environment
pub struct LuaEnv {
  vars: HashMap<String, LuaVal>,
  parent: Option<Rc<RefCell<LuaEnv>>>,
}

impl LuaEnv {
  pub fn new() -> Self {
    let mut env = Self {
      vars: HashMap::new(),
      parent: None,
    };
    env.init_builtins();
    env
  }

  pub fn new_child(parent: Rc<RefCell<LuaEnv>>) -> Self {
    Self {
      vars: HashMap::new(),
      parent: Some(parent),
    }
  }

  fn init_builtins(&mut self) {
    self.set_local("tonumber", LuaVal::NativeFn("tonumber".to_string()));
    self.set_local("tostring", LuaVal::NativeFn("tostring".to_string()));
    self.set_local("print", LuaVal::NativeFn("print".to_string()));
    self.set_local("type", LuaVal::NativeFn("type".to_string()));
    self.set_local("pairs", LuaVal::NativeFn("pairs".to_string()));
    self.set_local("ipairs", LuaVal::NativeFn("ipairs".to_string()));

    // string library
    let mut str_lib = HashMap::new();
    str_lib.insert(
      "format".to_string(),
      LuaVal::NativeFn("string.format".to_string()),
    );
    str_lib.insert(
      "lower".to_string(),
      LuaVal::NativeFn("string.lower".to_string()),
    );
    str_lib.insert(
      "upper".to_string(),
      LuaVal::NativeFn("string.upper".to_string()),
    );
    str_lib.insert(
      "sub".to_string(),
      LuaVal::NativeFn("string.sub".to_string()),
    );
    str_lib.insert(
      "find".to_string(),
      LuaVal::NativeFn("string.find".to_string()),
    );
    self.set_local("string", LuaVal::Table(Rc::new(RefCell::new(str_lib))));

    // math library
    let mut math_lib = HashMap::new();
    math_lib.insert("min".to_string(), LuaVal::NativeFn("math.min".to_string()));
    math_lib.insert("max".to_string(), LuaVal::NativeFn("math.max".to_string()));
    math_lib.insert(
      "floor".to_string(),
      LuaVal::NativeFn("math.floor".to_string()),
    );
    math_lib.insert(
      "ceil".to_string(),
      LuaVal::NativeFn("math.ceil".to_string()),
    );
    math_lib.insert("abs".to_string(), LuaVal::NativeFn("math.abs".to_string()));
    self.set_local("math", LuaVal::Table(Rc::new(RefCell::new(math_lib))));

    // table library
    let mut tbl_lib = HashMap::new();
    tbl_lib.insert(
      "insert".to_string(),
      LuaVal::NativeFn("table.insert".to_string()),
    );
    self.set_local("table", LuaVal::Table(Rc::new(RefCell::new(tbl_lib))));
  }

  pub fn get(&self, name: &str) -> LuaVal {
    if let Some(val) = self.vars.get(name) {
      val.clone()
    } else if let Some(parent) = &self.parent {
      parent.borrow().get(name)
    } else {
      LuaVal::Nil
    }
  }

  pub fn set(&mut self, name: &str, val: LuaVal) {
    if self.vars.contains_key(name) {
      self.vars.insert(name.to_string(), val);
    } else if let Some(parent) = &self.parent {
      let mut p = parent.borrow_mut();
      if p.contains_key(name) {
        p.set(name, val);
      } else {
        self.vars.insert(name.to_string(), val);
      }
    } else {
      self.vars.insert(name.to_string(), val);
    }
  }

  pub fn set_local(&mut self, name: &str, val: LuaVal) {
    self.vars.insert(name.to_string(), val);
  }

  pub fn contains_key(&self, name: &str) -> bool {
    if self.vars.contains_key(name) {
      true
    } else if let Some(parent) = &self.parent {
      parent.borrow().contains_key(name)
    } else {
      false
    }
  }
}

// Interpreter with Gas / Instruction metering, Recursion / Fork-Bomb Protection & Memory Quotas
pub struct Interpreter {
  pub stdout: Vec<String>,
  pub gas_remaining: usize,
  pub call_depth: usize,
  pub expr_depth: usize,
  pub modified_keys: Vec<(String, String, String)>, // (param, old_val, new_val)
}

impl Interpreter {
  pub fn new() -> Self {
    Self {
      stdout: Vec::new(),
      gas_remaining: MAX_LUA_GAS,
      call_depth: 0,
      expr_depth: 0,
      modified_keys: Vec::new(),
    }
  }

  fn check_gas(&mut self) -> Result<(), String> {
    if self.gas_remaining == 0 {
      Err(format!(
        "[Lua Sandbox Security Violation] Execution step limit ({} steps) exceeded — infinite loop or runaway compute terminated.",
        MAX_LUA_GAS
      ))
    } else {
      self.gas_remaining -= 1;
      Ok(())
    }
  }

  pub fn eval_chunk(
    &mut self,
    stmts: &[Stmt],
    env: Rc<RefCell<LuaEnv>>,
  ) -> Result<Option<LuaVal>, String> {
    for stmt in stmts {
      self.check_gas()?;
      if let Some(ret) = self.eval_stmt(stmt, env.clone())? {
        return Ok(Some(ret));
      }
    }
    Ok(None)
  }

  pub fn eval_stmt(
    &mut self,
    stmt: &Stmt,
    env: Rc<RefCell<LuaEnv>>,
  ) -> Result<Option<LuaVal>, String> {
    self.check_gas()?;
    match stmt {
      Stmt::Assign(targets, values) => {
        let mut evaluated_vals = Vec::new();
        for v in values {
          evaluated_vals.push(self.eval_expr(v, env.clone())?);
        }
        for (idx, target) in targets.iter().enumerate() {
          let val = evaluated_vals.get(idx).cloned().unwrap_or(LuaVal::Nil);
          self.assign_to_target(target, val, env.clone())?;
        }
        Ok(None)
      }
      Stmt::LocalAssign(names, values) => {
        let mut evaluated_vals = Vec::new();
        for v in values {
          evaluated_vals.push(self.eval_expr(v, env.clone())?);
        }
        for (idx, name) in names.iter().enumerate() {
          let val = evaluated_vals.get(idx).cloned().unwrap_or(LuaVal::Nil);
          env.borrow_mut().set_local(name, val);
        }
        Ok(None)
      }
      Stmt::FunctionDef(name, params, body) => {
        let func = LuaVal::Function(name.clone(), params.clone(), body.clone());
        env.borrow_mut().set(name, func);
        Ok(None)
      }
      Stmt::FunctionCall(expr) => {
        self.eval_expr(expr, env)?;
        Ok(None)
      }
      Stmt::If {
        cond,
        then_block,
        elseif_blocks,
        else_block,
      } => {
        let c = self.eval_expr(cond, env.clone())?;
        if c.is_truthy() {
          let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env)));
          return self.eval_chunk(then_block, child_env);
        }
        for (elif_c, elif_body) in elseif_blocks {
          let res = self.eval_expr(elif_c, env.clone())?;
          if res.is_truthy() {
            let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env)));
            return self.eval_chunk(elif_body, child_env);
          }
        }
        if let Some(else_b) = else_block {
          let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env)));
          return self.eval_chunk(else_b, child_env);
        }
        Ok(None)
      }
      Stmt::While { cond, body } => {
        while self.eval_expr(cond, env.clone())?.is_truthy() {
          self.check_gas()?;
          let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env.clone())));
          if let Some(ret) = self.eval_chunk(body, child_env)? {
            return Ok(Some(ret));
          }
        }
        Ok(None)
      }
      Stmt::ForNumeric {
        var,
        start,
        stop,
        step,
        body,
      } => {
        let start_val = match self.eval_expr(start, env.clone())? {
          LuaVal::Number(n) => n,
          _ => return Err("Numeric for start must be a number".to_string()),
        };
        let stop_val = match self.eval_expr(stop, env.clone())? {
          LuaVal::Number(n) => n,
          _ => return Err("Numeric for stop must be a number".to_string()),
        };
        let step_val = if let Some(s) = step {
          match self.eval_expr(s, env.clone())? {
            LuaVal::Number(n) => n,
            _ => return Err("Numeric for step must be a number".to_string()),
          }
        } else {
          1.0
        };

        if !start_val.is_finite() || !stop_val.is_finite() || !step_val.is_finite() {
          return Err(
            "[Lua Sandbox Security Violation] Numeric 'for' loop bounds and step must be finite numbers."
              .to_string(),
          );
        }
        if step_val == 0.0 {
          return Err(
            "[Lua Sandbox Security Violation] Numeric 'for' loop step cannot be 0 (infinite loop protection)."
              .to_string(),
          );
        }

        let mut current = start_val;
        let forward = step_val > 0.0;
        while (forward && current <= stop_val) || (!forward && current >= stop_val) {
          self.check_gas()?;
          let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env.clone())));
          child_env
            .borrow_mut()
            .set_local(var, LuaVal::Number(current));
          if let Some(ret) = self.eval_chunk(body, child_env)? {
            return Ok(Some(ret));
          }
          current += step_val;
        }
        Ok(None)
      }
      Stmt::ForGeneric {
        vars,
        iter_expr,
        body,
      } => {
        let table_val = self.eval_expr(iter_expr, env.clone())?;
        if let LuaVal::Table(t) = table_val {
          let map = t.borrow().clone();
          for (k, v) in map {
            self.check_gas()?;
            let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env.clone())));
            if let Some(var0) = vars.get(0) {
              child_env.borrow_mut().set_local(var0, LuaVal::String(k));
            }
            if let Some(var1) = vars.get(1) {
              child_env.borrow_mut().set_local(var1, v);
            }
            if let Some(ret) = self.eval_chunk(body, child_env)? {
              return Ok(Some(ret));
            }
          }
        }
        Ok(None)
      }
      Stmt::Return(exprs) => {
        if exprs.len() >= 2 {
          let first = self.eval_expr(&exprs[0], env.clone())?;
          let second = self.eval_expr(&exprs[1], env)?;
          if first == LuaVal::Boolean(false) {
            let mut map = HashMap::new();
            map.insert("ok".to_string(), LuaVal::Boolean(false));
            map.insert("error".to_string(), LuaVal::String(second.to_lua_string()));
            map.insert("1".to_string(), first);
            map.insert("2".to_string(), second);
            return Ok(Some(LuaVal::Table(Rc::new(RefCell::new(map)))));
          }
          Ok(Some(first))
        } else if let Some(first) = exprs.get(0) {
          let val = self.eval_expr(first, env)?;
          Ok(Some(val))
        } else {
          Ok(Some(LuaVal::Nil))
        }
      }
    }
  }

  fn assign_to_target(
    &mut self,
    target: &Expr,
    val: LuaVal,
    env: Rc<RefCell<LuaEnv>>,
  ) -> Result<(), String> {
    match target {
      Expr::Variable(name) => {
        env.borrow_mut().set(name, val);
        Ok(())
      }
      Expr::TableField(tbl_expr, field) => {
        let tbl_val = self.eval_expr(tbl_expr, env)?;
        if let LuaVal::Table(t) = tbl_val {
          let is_new_key = !t.borrow().contains_key(field);
          if is_new_key && t.borrow().len() >= MAX_LUA_TABLE_ENTRIES {
            return Err(format!(
              "[Lua Sandbox Security Violation] Table size limit ({} entries) exceeded — memory exhaustion bomb prevented.",
              MAX_LUA_TABLE_ENTRIES
            ));
          }
          let old_val = t
            .borrow()
            .get(field)
            .map(|v| v.to_lua_string())
            .unwrap_or_else(|| "nil".to_string());
          t.borrow_mut().insert(field.clone(), val.clone());
          self
            .modified_keys
            .push((field.clone(), old_val, val.to_lua_string()));
          Ok(())
        } else {
          Err(format!("Attempt to index a non-table: {:?}", tbl_val))
        }
      }
      Expr::TableIndex(tbl_expr, idx_expr) => {
        let tbl_val = self.eval_expr(tbl_expr, env.clone())?;
        let idx_val = self.eval_expr(idx_expr, env)?;
        let key_str = match idx_val {
          LuaVal::String(s) => s,
          LuaVal::Number(n) => {
            if n.fract() == 0.0 {
              format!("{:.0}", n)
            } else {
              format!("{}", n)
            }
          }
          _ => return Err(format!("Invalid table key: {:?}", idx_val)),
        };
        if let LuaVal::Table(t) = tbl_val {
          let is_new_key = !t.borrow().contains_key(&key_str);
          if is_new_key && t.borrow().len() >= MAX_LUA_TABLE_ENTRIES {
            return Err(format!(
              "[Lua Sandbox Security Violation] Table size limit ({} entries) exceeded — memory exhaustion bomb prevented.",
              MAX_LUA_TABLE_ENTRIES
            ));
          }
          let old_val = t
            .borrow()
            .get(&key_str)
            .map(|v| v.to_lua_string())
            .unwrap_or_else(|| "nil".to_string());
          t.borrow_mut().insert(key_str.clone(), val.clone());
          self
            .modified_keys
            .push((key_str, old_val, val.to_lua_string()));
          Ok(())
        } else {
          Err(format!("Attempt to index a non-table: {:?}", tbl_val))
        }
      }
      _ => Err("Invalid assignment target".to_string()),
    }
  }

  pub fn eval_expr(&mut self, expr: &Expr, env: Rc<RefCell<LuaEnv>>) -> Result<LuaVal, String> {
    self.check_gas()?;
    if self.expr_depth >= MAX_LUA_EXPR_DEPTH {
      return Err(format!(
        "[Lua Sandbox Security Violation] Maximum expression nesting depth ({}) exceeded.",
        MAX_LUA_EXPR_DEPTH
      ));
    }
    self.expr_depth += 1;
    let res = self.eval_expr_inner(expr, env);
    self.expr_depth = self.expr_depth.saturating_sub(1);
    res
  }

  fn eval_expr_inner(&mut self, expr: &Expr, env: Rc<RefCell<LuaEnv>>) -> Result<LuaVal, String> {
    match expr {
      Expr::Literal(v) => Ok(v.clone()),
      Expr::Variable(name) => Ok(env.borrow().get(name)),
      Expr::TableField(tbl_expr, field) => {
        let tbl = self.eval_expr(tbl_expr, env)?;
        if let LuaVal::Table(t) = tbl {
          Ok(t.borrow().get(field).cloned().unwrap_or(LuaVal::Nil))
        } else {
          Ok(LuaVal::Nil)
        }
      }
      Expr::TableIndex(tbl_expr, idx_expr) => {
        let tbl = self.eval_expr(tbl_expr, env.clone())?;
        let idx = self.eval_expr(idx_expr, env)?;
        let key = match idx {
          LuaVal::String(s) => s,
          LuaVal::Number(n) => {
            if n.fract() == 0.0 {
              format!("{:.0}", n)
            } else {
              format!("{}", n)
            }
          }
          _ => return Ok(LuaVal::Nil),
        };
        if let LuaVal::Table(t) = tbl {
          Ok(t.borrow().get(&key).cloned().unwrap_or(LuaVal::Nil))
        } else {
          Ok(LuaVal::Nil)
        }
      }
      Expr::TableConstructor(fields) => {
        if fields.len() > MAX_LUA_TABLE_ENTRIES {
          return Err(format!(
            "[Lua Sandbox Security Violation] Table constructor size ({}) exceeds {} entry limit.",
            fields.len(),
            MAX_LUA_TABLE_ENTRIES
          ));
        }
        let mut map = HashMap::new();
        for (key_opt, val_expr) in fields {
          let val = self.eval_expr(val_expr, env.clone())?;
          let key = if let Some(k_expr) = key_opt {
            match self.eval_expr(k_expr, env.clone())? {
              LuaVal::String(s) => s,
              LuaVal::Number(n) => {
                if n.fract() == 0.0 {
                  format!("{:.0}", n)
                } else {
                  format!("{}", n)
                }
              }
              _ => return Err("Invalid key in table constructor".to_string()),
            }
          } else {
            (map.len() + 1).to_string()
          };
          map.insert(key, val);
        }
        Ok(LuaVal::Table(Rc::new(RefCell::new(map))))
      }
      Expr::UnaryOp(op, operand) => {
        let val = self.eval_expr(operand, env)?;
        match op {
          UnaryOp::Not => Ok(LuaVal::Boolean(!val.is_truthy())),
          UnaryOp::Negate => match val {
            LuaVal::Number(n) => Ok(LuaVal::Number(-n)),
            _ => Err("Attempt to negate non-number".to_string()),
          },
          UnaryOp::Length => match val {
            LuaVal::String(s) => Ok(LuaVal::Number(s.len() as f64)),
            LuaVal::Table(t) => {
              let map = t.borrow();
              let mut count = 0usize;
              while map.contains_key(&(count + 1).to_string()) {
                count += 1;
              }
              if count == 0 && !map.is_empty() {
                count = map.len();
              }
              Ok(LuaVal::Number(count as f64))
            }
            _ => Err("Attempt to get length of invalid type".to_string()),
          },
        }
      }
      Expr::BinaryOp(op, left_expr, right_expr) => {
        if *op == BinaryOp::And {
          let left = self.eval_expr(left_expr, env.clone())?;
          if !left.is_truthy() {
            return Ok(left);
          }
          return self.eval_expr(right_expr, env);
        }
        if *op == BinaryOp::Or {
          let left = self.eval_expr(left_expr, env.clone())?;
          if left.is_truthy() {
            return Ok(left);
          }
          return self.eval_expr(right_expr, env);
        }

        let left = self.eval_expr(left_expr, env.clone())?;
        let right = self.eval_expr(right_expr, env)?;

        match op {
          BinaryOp::Add => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Number(a + b)),
            _ => Err(format!(
              "Arithmetic '+' on non-numbers: {} and {}",
              left, right
            )),
          },
          BinaryOp::Sub => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Number(a - b)),
            _ => Err(format!(
              "Arithmetic '-' on non-numbers: {} and {}",
              left, right
            )),
          },
          BinaryOp::Mul => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Number(a * b)),
            _ => Err(format!(
              "Arithmetic '*' on non-numbers: {} and {}",
              left, right
            )),
          },
          BinaryOp::Div => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => {
              if *b == 0.0 {
                Err("Division by zero".to_string())
              } else {
                Ok(LuaVal::Number(a / b))
              }
            }
            _ => Err(format!(
              "Arithmetic '/' on non-numbers: {} and {}",
              left, right
            )),
          },
          BinaryOp::Mod => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Number(a % b)),
            _ => Err("Arithmetic '%' on non-numbers".to_string()),
          },
          BinaryOp::Concat => {
            let s1 = left.to_lua_string();
            let s2 = right.to_lua_string();
            if s1.len().saturating_add(s2.len()) > MAX_LUA_STRING_BYTES {
              return Err(format!(
                "[Lua Sandbox Security Violation] String concatenation ({} bytes) exceeds {} byte limit — exponential string memory bomb prevented.",
                s1.len().saturating_add(s2.len()),
                MAX_LUA_STRING_BYTES
              ));
            }
            Ok(LuaVal::String(format!("{}{}", s1, s2)))
          }
          BinaryOp::Eq => Ok(LuaVal::Boolean(left == right)),
          BinaryOp::Neq => Ok(LuaVal::Boolean(left != right)),
          BinaryOp::Lt => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Boolean(a < b)),
            (LuaVal::String(a), LuaVal::String(b)) => Ok(LuaVal::Boolean(a < b)),
            _ => Err("Invalid comparison '<'".to_string()),
          },
          BinaryOp::Lte => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Boolean(a <= b)),
            (LuaVal::String(a), LuaVal::String(b)) => Ok(LuaVal::Boolean(a <= b)),
            _ => Err("Invalid comparison '<='".to_string()),
          },
          BinaryOp::Gt => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Boolean(a > b)),
            (LuaVal::String(a), LuaVal::String(b)) => Ok(LuaVal::Boolean(a > b)),
            _ => Err("Invalid comparison '>'".to_string()),
          },
          BinaryOp::Gte => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Boolean(a >= b)),
            (LuaVal::String(a), LuaVal::String(b)) => Ok(LuaVal::Boolean(a >= b)),
            _ => Err("Invalid comparison '>='".to_string()),
          },
          BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
      }
      Expr::FunctionCall(func_expr, arg_exprs) => {
        let func_val = self.eval_expr(func_expr, env.clone())?;
        let mut evaluated_args = Vec::new();
        for arg in arg_exprs {
          evaluated_args.push(self.eval_expr(arg, env.clone())?);
        }
        self.call_function(&func_val, evaluated_args, env)
      }
      Expr::FunctionDef(params, body) => Ok(LuaVal::Function(
        "anonymous".to_string(),
        params.clone(),
        body.clone(),
      )),
    }
  }

  fn call_function(
    &mut self,
    func: &LuaVal,
    args: Vec<LuaVal>,
    env: Rc<RefCell<LuaEnv>>,
  ) -> Result<LuaVal, String> {
    self.check_gas()?;
    match func {
      LuaVal::NativeFn(name) => self.call_native(name, args),
      LuaVal::Function(_name, params, body) => {
        if self.call_depth >= MAX_LUA_CALL_DEPTH {
          return Err(format!(
            "[Lua Sandbox Security Violation] Maximum function call / recursion depth ({}) exceeded — fork-bomb & stack-overflow protection triggered.",
            MAX_LUA_CALL_DEPTH
          ));
        }
        self.call_depth += 1;
        let child_env = Rc::new(RefCell::new(LuaEnv::new_child(env)));
        for (idx, param) in params.iter().enumerate() {
          let arg_val = args.get(idx).cloned().unwrap_or(LuaVal::Nil);
          child_env.borrow_mut().set_local(param, arg_val);
        }
        let ret = self.eval_chunk(body, child_env);
        self.call_depth = self.call_depth.saturating_sub(1);
        Ok(ret?.unwrap_or(LuaVal::Nil))
      }
      _ => Err(format!("Attempt to call a non-function value: {}", func)),
    }
  }

  fn call_native(&mut self, name: &str, args: Vec<LuaVal>) -> Result<LuaVal, String> {
    match name {
      "tonumber" => {
        if let Some(arg) = args.get(0) {
          let base = args.get(1).and_then(|b| match b {
            LuaVal::Number(n) => Some(*n as u32),
            _ => None,
          });
          let parse_str_num = |s: &str, base: Option<u32>| -> LuaVal {
            let trimmed = s.trim();
            if let Some(radix) = base {
              if (2..=36).contains(&radix) {
                let clean = trimmed
                  .strip_prefix("0x")
                  .or_else(|| trimmed.strip_prefix("0X"))
                  .unwrap_or(trimmed);
                match i64::from_str_radix(clean, radix) {
                  Ok(n) => LuaVal::Number(n as f64),
                  Err(_) => LuaVal::Nil,
                }
              } else {
                LuaVal::Nil
              }
            } else if let Ok(n) = trimmed.trim_start_matches('+').parse::<f64>() {
              LuaVal::Number(n)
            } else {
              let stripped = trimmed
                .trim_start_matches('+')
                .trim_end_matches("RH")
                .trim()
                .trim_end_matches('%')
                .trim_end_matches("mm³/s/mm")
                .trim_end_matches("mm³/s")
                .trim_end_matches("mm/s")
                .trim_end_matches("mm")
                .trim_end_matches("°C")
                .trim_end_matches('s')
                .trim();
              match stripped.parse::<f64>() {
                Ok(n) => LuaVal::Number(n),
                Err(_) => LuaVal::Nil,
              }
            }
          };
          match arg {
            LuaVal::Number(n) => Ok(LuaVal::Number(*n)),
            LuaVal::String(s) => Ok(parse_str_num(s, base)),
            LuaVal::Table(t) => {
              let first = t.borrow().get("1").cloned();
              match first {
                Some(LuaVal::Number(n)) => Ok(LuaVal::Number(n)),
                Some(LuaVal::String(s)) => Ok(parse_str_num(&s, base)),
                _ => Ok(LuaVal::Nil),
              }
            }
            _ => Ok(LuaVal::Nil),
          }
        } else {
          Ok(LuaVal::Nil)
        }
      }
      "tostring" => {
        if let Some(arg) = args.get(0) {
          Ok(LuaVal::String(arg.to_lua_string()))
        } else {
          Ok(LuaVal::String("nil".to_string()))
        }
      }
      "print" => {
        if self.stdout.len() >= MAX_LUA_STDOUT_LINES {
          return Err(format!(
            "[Lua Sandbox Security Violation] Maximum print log output ({} lines) exceeded.",
            MAX_LUA_STDOUT_LINES
          ));
        }
        let parts: Vec<String> = args.iter().map(|a| a.to_lua_string()).collect();
        let line = parts.join("\t");
        self.stdout.push(line);
        Ok(LuaVal::Nil)
      }
      "type" => {
        if let Some(arg) = args.get(0) {
          let type_name = match arg {
            LuaVal::Nil => "nil",
            LuaVal::Boolean(_) => "boolean",
            LuaVal::Number(_) => "number",
            LuaVal::String(_) => "string",
            LuaVal::Table(_) => "table",
            LuaVal::Function(_, _, _) | LuaVal::NativeFn(_) => "function",
          };
          Ok(LuaVal::String(type_name.to_string()))
        } else {
          Ok(LuaVal::String("nil".to_string()))
        }
      }
      "pairs" | "ipairs" => {
        if let Some(arg) = args.get(0) {
          Ok(arg.clone())
        } else {
          Ok(LuaVal::Nil)
        }
      }
      "string.format" => {
        if let Some(fmt_val) = args.get(0) {
          let fmt_str = fmt_val.to_lua_string();
          let mut result = String::new();
          let mut arg_idx = 1;
          let mut chars = fmt_str.chars().peekable();
          let coerce_num = |val_opt: Option<&LuaVal>| -> Option<f64> {
            match val_opt {
              Some(LuaVal::Number(n)) => Some(*n),
              Some(LuaVal::String(s)) => s
                .trim()
                .trim_start_matches('+')
                .trim_end_matches('%')
                .trim_end_matches("mm/s")
                .trim_end_matches("mm")
                .trim_end_matches("°C")
                .trim_end_matches('s')
                .trim()
                .parse::<f64>()
                .ok(),
              _ => None,
            }
          };
          while let Some(c) = chars.next() {
            if c == '%' {
              let force_plus = if chars.peek() == Some(&'+') {
                chars.next();
                true
              } else {
                false
              };
              if let Some(spec) = chars.next() {
                match spec {
                  '%' => result.push('%'),
                  's' => {
                    let a = args
                      .get(arg_idx)
                      .map(|v| v.to_lua_string())
                      .unwrap_or_default();
                    result.push_str(&a);
                    arg_idx += 1;
                  }
                  'd' | 'i' => {
                    if let Some(n) = coerce_num(args.get(arg_idx)) {
                      let iv = n.round() as i64;
                      if force_plus {
                        result.push_str(&format!("{:+}", iv));
                      } else {
                        result.push_str(&format!("{}", iv));
                      }
                    } else if force_plus {
                      result.push_str("+0");
                    } else {
                      result.push_str("0");
                    }
                    arg_idx += 1;
                  }
                  '.' => {
                    // e.g. %.2f or %+.2f
                    let mut prec = String::new();
                    while let Some(&p) = chars.peek() {
                      if p.is_ascii_digit() {
                        prec.push(chars.next().unwrap());
                      } else {
                        break;
                      }
                    }
                    let spec_end = chars.next().unwrap_or('f');
                    let p: usize = prec.parse::<usize>().unwrap_or(2).min(12);
                    if spec_end == 'f' {
                      if let Some(n) = coerce_num(args.get(arg_idx)) {
                        if force_plus {
                          result.push_str(&format!("{:+.*}", p, n));
                        } else {
                          result.push_str(&format!("{:.*}", p, n));
                        }
                      } else {
                        result.push_str("0.00");
                      }
                    }
                    arg_idx += 1;
                  }
                  'f' => {
                    if let Some(n) = coerce_num(args.get(arg_idx)) {
                      if force_plus {
                        result.push_str(&format!("{:+.2}", n));
                      } else {
                        result.push_str(&format!("{:.2}", n));
                      }
                    } else {
                      result.push_str("0.00");
                    }
                    arg_idx += 1;
                  }
                  _ => {
                    result.push('%');
                    if force_plus {
                      result.push('+');
                    }
                    result.push(spec);
                  }
                }
              } else {
                result.push('%');
              }
            } else {
              result.push(c);
            }
            if result.len() > MAX_LUA_STRING_BYTES {
              return Err(format!(
                "[Lua Sandbox Security Violation] string.format output exceeds {} byte limit.",
                MAX_LUA_STRING_BYTES
              ));
            }
          }
          Ok(LuaVal::String(result))
        } else {
          Ok(LuaVal::String(String::new()))
        }
      }
      "string.lower" => {
        if let Some(LuaVal::String(s)) = args.get(0) {
          Ok(LuaVal::String(s.to_lowercase()))
        } else {
          Ok(LuaVal::String(String::new()))
        }
      }
      "string.upper" => {
        if let Some(LuaVal::String(s)) = args.get(0) {
          Ok(LuaVal::String(s.to_uppercase()))
        } else {
          Ok(LuaVal::String(String::new()))
        }
      }
      "string.sub" => {
        if let Some(LuaVal::String(s)) = args.get(0) {
          let chars: Vec<char> = s.chars().collect();
          let len = chars.len() as isize;
          if len == 0 {
            return Ok(LuaVal::String(String::new()));
          }
          let raw_start = match args.get(1) {
            Some(LuaVal::Number(n)) => *n as isize,
            _ => 1,
          };
          let raw_end = match args.get(2) {
            Some(LuaVal::Number(n)) => *n as isize,
            _ => len,
          };
          let start_idx = if raw_start < 0 {
            (len + raw_start + 1).max(1)
          } else {
            raw_start.max(1)
          };
          let end_idx = if raw_end < 0 {
            len + raw_end + 1
          } else {
            raw_end.min(len)
          };
          if start_idx > end_idx || start_idx > len {
            Ok(LuaVal::String(String::new()))
          } else {
            let slice: String = chars[(start_idx as usize - 1)..(end_idx as usize)]
              .iter()
              .collect();
            Ok(LuaVal::String(slice))
          }
        } else {
          Ok(LuaVal::String(String::new()))
        }
      }
      "string.find" => {
        if let (Some(haystack_val), Some(needle_val)) = (args.get(0), args.get(1)) {
          let haystack = haystack_val.to_lua_string();
          let needle = needle_val.to_lua_string().replace("%%", "%");
          if let Some(pos) = haystack.find(&needle) {
            Ok(LuaVal::Number((pos + 1) as f64))
          } else {
            Ok(LuaVal::Nil)
          }
        } else {
          Ok(LuaVal::Nil)
        }
      }
      "math.min" => {
        let mut min_val = f64::INFINITY;
        for a in args {
          if let LuaVal::Number(n) = a {
            if n < min_val {
              min_val = n;
            }
          }
        }
        Ok(LuaVal::Number(if min_val.is_infinite() {
          0.0
        } else {
          min_val
        }))
      }
      "math.max" => {
        let mut max_val = f64::NEG_INFINITY;
        for a in args {
          if let LuaVal::Number(n) = a {
            if n > max_val {
              max_val = n;
            }
          }
        }
        Ok(LuaVal::Number(if max_val.is_infinite() {
          0.0
        } else {
          max_val
        }))
      }
      "math.floor" => {
        if let Some(LuaVal::Number(n)) = args.get(0) {
          Ok(LuaVal::Number(n.floor()))
        } else {
          Ok(LuaVal::Number(0.0))
        }
      }
      "math.ceil" => {
        if let Some(LuaVal::Number(n)) = args.get(0) {
          Ok(LuaVal::Number(n.ceil()))
        } else {
          Ok(LuaVal::Number(0.0))
        }
      }
      "math.abs" => {
        if let Some(LuaVal::Number(n)) = args.get(0) {
          Ok(LuaVal::Number(n.abs()))
        } else {
          Ok(LuaVal::Number(0.0))
        }
      }
      "table.insert" => {
        if let Some(LuaVal::Table(t)) = args.get(0) {
          if t.borrow().len() >= MAX_LUA_TABLE_ENTRIES {
            return Err(format!(
              "[Lua Sandbox Security Violation] table.insert exceeded maximum table size ({} entries) — memory exhaustion bomb prevented.",
              MAX_LUA_TABLE_ENTRIES
            ));
          }
          if let Some(val) = args.get(1) {
            let idx = (t.borrow().len() + 1).to_string();
            t.borrow_mut().insert(idx, val.clone());
          }
        }
        Ok(LuaVal::Nil)
      }
      _ => Err(format!("Unknown native function: {}", name)),
    }
  }
}

/// Generalized parameter type specifier declared inside a Lua transformation rule (`params = { ... }`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LuaParamKind {
  /// Numeric value constrained within `[min, max]` with an optional UI step.
  Range {
    min: f64,
    max: f64,
    step: Option<f64>,
  },
  /// Discrete choice from an explicit set of `(value, label)` options.
  Enum,
  /// Boolean flag (`"true"` / `"false"`).
  Boolean,
  /// Freeform string parameter.
  Text,
}

/// A single parameter specification extracted from a Lua script's `params` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LuaParamSpec {
  pub id: String,
  pub label: String,
  pub description: String,
  pub kind: LuaParamKind,
  pub default_value: String,
  pub unit: String,
  /// Allowed `(value, label)` choices for `Enum`, or quick-select presets for `Range`.
  pub options: Vec<(String, String)>,
  /// Optional linked parameter assignments (`option_value -> [(target_param_id, target_value)]`)
  /// declared via `sets = { ... }` on an option entry (e.g. a climate preset snapping temp & humidity dials).
  #[serde(default)]
  pub option_sets: HashMap<String, Vec<(String, String)>>,
}

/// Optional metadata declared inside a Lua rule (`meta = { ... }`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LuaRuleMeta {
  pub id: Option<String>,
  pub name: Option<String>,
  pub icon: Option<String>,
  pub category: Option<String>,
  pub tree_category: Option<String>,
  pub tree_subcategory: Option<String>,
  pub slicer_menu_path: Option<String>,
  pub when_to_run: Option<String>,
  pub formula_summary: Option<String>,
  pub default_condition_expr: Option<String>,
  pub how_to_read_print: Vec<String>,
  pub hardware_checklist: Vec<String>,
}

/// Full introspection result for a parameterized Lua transformation script.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LuaRuleSpec {
  pub meta: LuaRuleMeta,
  pub params: Vec<LuaParamSpec>,
  pub configured_params: HashMap<String, String>,
  pub has_validate_fn: bool,
  pub has_describe_fn: bool,
}

/// Dynamic rule description returned by a Lua script's `describe(params, ctx)` function.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LuaRuleDescription {
  pub title: Option<String>,
  pub description: Option<String>,
  pub badge: Option<String>,
}

fn lua_table_get_str(map: &HashMap<String, LuaVal>, key: &str) -> Option<String> {
  match map.get(key) {
    Some(LuaVal::Nil) | None => None,
    Some(v) => Some(v.to_lua_string()),
  }
}

fn lua_table_get_num(map: &HashMap<String, LuaVal>, key: &str) -> Option<f64> {
  match map.get(key) {
    Some(LuaVal::Number(n)) => Some(*n),
    Some(LuaVal::String(s)) => s.trim().parse::<f64>().ok(),
    _ => None,
  }
}

fn lua_table_get_string_list(map: &HashMap<String, LuaVal>, key: &str) -> Vec<String> {
  if let Some(LuaVal::Table(t)) = map.get(key) {
    let borrowed = t.borrow();
    let mut idx = 1usize;
    let mut out = Vec::new();
    while let Some(v) = borrowed.get(&idx.to_string()) {
      if !matches!(v, LuaVal::Nil) {
        out.push(v.to_lua_string());
      }
      idx += 1;
    }
    out
  } else {
    Vec::new()
  }
}

fn parse_lua_param_spec_entry(
  entry_map: &HashMap<String, LuaVal>,
  fallback_idx: usize,
) -> LuaParamSpec {
  let id = lua_table_get_str(entry_map, "id")
    .or_else(|| lua_table_get_str(entry_map, "key"))
    .unwrap_or_else(|| format!("param_{}", fallback_idx));
  let label = lua_table_get_str(entry_map, "label")
    .or_else(|| lua_table_get_str(entry_map, "question"))
    .or_else(|| lua_table_get_str(entry_map, "name"))
    .unwrap_or_else(|| id.clone());
  let description = lua_table_get_str(entry_map, "description")
    .or_else(|| lua_table_get_str(entry_map, "hint"))
    .unwrap_or_default();
  let unit = lua_table_get_str(entry_map, "unit").unwrap_or_default();
  let default_value = lua_table_get_str(entry_map, "default")
    .or_else(|| lua_table_get_str(entry_map, "default_value"))
    .unwrap_or_default();

  let mut options = Vec::new();
  let mut option_sets = HashMap::new();
  if let Some(LuaVal::Table(opts_tbl)) = entry_map.get("options") {
    let opts_borrowed = opts_tbl.borrow();
    let mut i = 1usize;
    while let Some(opt_val) = opts_borrowed.get(&i.to_string()) {
      match opt_val {
        LuaVal::Table(opt_entry) => {
          let ob = opt_entry.borrow();
          let val = lua_table_get_str(&ob, "value")
            .or_else(|| lua_table_get_str(&ob, "1"))
            .unwrap_or_default();
          let lbl = lua_table_get_str(&ob, "label")
            .or_else(|| lua_table_get_str(&ob, "2"))
            .unwrap_or_else(|| val.clone());
          if let Some(LuaVal::Table(sets_tbl)) = ob.get("sets") {
            let sb = sets_tbl.borrow();
            let mut linked = Vec::new();
            let mut keys: Vec<String> = sb.keys().cloned().collect();
            keys.sort();
            for k in keys {
              if let Some(v) = sb.get(&k) {
                if !matches!(v, LuaVal::Nil) {
                  linked.push((k, v.to_lua_string()));
                }
              }
            }
            if !linked.is_empty() {
              option_sets.insert(val.clone(), linked);
            }
          }
          options.push((val, lbl));
        }
        other => {
          let s = other.to_lua_string();
          options.push((s.clone(), s));
        }
      }
      i += 1;
    }
  }

  let raw_type = lua_table_get_str(entry_map, "type")
    .or_else(|| lua_table_get_str(entry_map, "kind"))
    .unwrap_or_default()
    .to_lowercase();
  let min_opt = lua_table_get_num(entry_map, "min");
  let max_opt = lua_table_get_num(entry_map, "max");
  let step_opt = lua_table_get_num(entry_map, "step");

  let kind =
    if raw_type == "range" || raw_type == "number" || min_opt.is_some() || max_opt.is_some() {
      LuaParamKind::Range {
        min: min_opt.unwrap_or(-1_000_000.0),
        max: max_opt.unwrap_or(1_000_000.0),
        step: step_opt,
      }
    } else if raw_type == "enum"
      || raw_type == "select"
      || (!options.is_empty() && raw_type.is_empty())
    {
      LuaParamKind::Enum
    } else if raw_type == "bool" || raw_type == "boolean" {
      LuaParamKind::Boolean
    } else {
      LuaParamKind::Text
    };

  LuaParamSpec {
    id,
    label,
    description,
    kind,
    default_value,
    unit,
    options,
    option_sets,
  }
}

/// Parse and evaluate top-level declarations of a Lua rule script to extract `meta`, `params`,
/// `configured_params`, and whether `validate(params, ctx)` / `describe(params, ctx)` exist.
pub fn extract_lua_rule_spec(script: &str) -> Result<LuaRuleSpec, String> {
  validate_lua_sandbox_security(script)?;

  let lexer = Lexer::new(script);
  let mut parser = Parser::new(lexer)?;
  let stmts = parser.parse_chunk()?;

  let mut interpreter = Interpreter::new();
  let env = Rc::new(RefCell::new(LuaEnv::new()));

  let dummy_cfg = LuaVal::from_json(&serde_json::json!({}));
  let dummy_ctx = LuaVal::from_json(&serde_json::json!({}));
  env.borrow_mut().set_local("config", dummy_cfg);
  env.borrow_mut().set_local("ctx", dummy_ctx.clone());
  env.borrow_mut().set_local("context", dummy_ctx);

  interpreter.eval_chunk(&stmts, env.clone())?;

  let mut spec = LuaRuleSpec::default();

  if let LuaVal::Table(meta_tbl) = env.borrow().get("meta") {
    let mb = meta_tbl.borrow();
    spec.meta = LuaRuleMeta {
      id: lua_table_get_str(&mb, "id"),
      name: lua_table_get_str(&mb, "name"),
      icon: lua_table_get_str(&mb, "icon"),
      category: lua_table_get_str(&mb, "category"),
      tree_category: lua_table_get_str(&mb, "tree_category"),
      tree_subcategory: lua_table_get_str(&mb, "tree_subcategory"),
      slicer_menu_path: lua_table_get_str(&mb, "slicer_menu_path"),
      when_to_run: lua_table_get_str(&mb, "when_to_run"),
      formula_summary: lua_table_get_str(&mb, "formula_summary"),
      default_condition_expr: lua_table_get_str(&mb, "default_condition_expr"),
      how_to_read_print: lua_table_get_string_list(&mb, "how_to_read_print"),
      hardware_checklist: lua_table_get_string_list(&mb, "hardware_checklist"),
    };
  }

  if let LuaVal::Table(params_tbl) = env.borrow().get("params") {
    let pb = params_tbl.borrow();
    let mut idx = 1usize;
    if pb.contains_key("1") {
      while let Some(LuaVal::Table(entry_tbl)) = pb.get(&idx.to_string()) {
        let eb = entry_tbl.borrow();
        spec.params.push(parse_lua_param_spec_entry(&eb, idx));
        idx += 1;
      }
    } else {
      let mut keys: Vec<String> = pb.keys().cloned().collect();
      keys.sort();
      for (i, k) in keys.into_iter().enumerate() {
        if let Some(LuaVal::Table(entry_tbl)) = pb.get(&k) {
          let mut eb = entry_tbl.borrow().clone();
          eb.entry("id".to_string()).or_insert(LuaVal::String(k));
          spec.params.push(parse_lua_param_spec_entry(&eb, i + 1));
        }
      }
    }
  }

  if let LuaVal::Table(cfg_params_tbl) = env.borrow().get("configured_params") {
    let cb = cfg_params_tbl.borrow();
    for (k, v) in cb.iter() {
      if !matches!(v, LuaVal::Nil) {
        spec.configured_params.insert(k.clone(), v.to_lua_string());
      }
    }
  }

  spec.has_validate_fn = matches!(env.borrow().get("validate"), LuaVal::Function(_, _, _));
  spec.has_describe_fn = matches!(env.borrow().get("describe"), LuaVal::Function(_, _, _));

  Ok(spec)
}

/// Resolve the effective parameter map by merging `params` defaults, `configured_params` in the script,
/// and caller-supplied `param_overrides`.
pub fn resolve_lua_rule_params(
  spec: &LuaRuleSpec,
  param_overrides: &HashMap<String, String>,
) -> HashMap<String, String> {
  let mut resolved = HashMap::new();
  for p in &spec.params {
    resolved.insert(p.id.clone(), p.default_value.clone());
  }
  for (k, v) in &spec.configured_params {
    resolved.insert(k.clone(), v.clone());
  }
  for (k, v) in param_overrides {
    resolved.insert(k.clone(), v.clone());
  }
  for p in &spec.params {
    if let Some(cur_val) = resolved.get(&p.id).cloned() {
      if let Some(linked_sets) = p.option_sets.get(&cur_val) {
        for (target_k, target_v) in linked_sets {
          let explicitly_set = param_overrides.contains_key(target_k)
            || (!param_overrides.contains_key(&p.id)
              && spec.configured_params.contains_key(target_k));
          if !explicitly_set {
            resolved.insert(target_k.clone(), target_v.clone());
          }
        }
      }
    }
  }
  resolved
}

/// Apply a single parameter change to `map`, automatically propagating any `sets = { ... }`
/// defined on an enum preset option (e.g., selecting `climate_preset = "tropical"` snaps
/// `ambient_temp_c = "30"` and `humidity_pct = "80"`), OR switching a linked preset parameter
/// to `"custom"` (or a matching preset) when one of its target dials is adjusted directly.
pub fn apply_lua_param_change_with_sets(
  params: &[LuaParamSpec],
  map: &mut HashMap<String, String>,
  changed_id: &str,
  new_val: String,
) {
  map.insert(changed_id.to_string(), new_val.clone());

  // 1. If `changed_id` has `option_sets` for `new_val`, snap all linked target parameters
  if let Some(changed_spec) = params.iter().find(|p| p.id == changed_id) {
    if let Some(linked) = changed_spec.option_sets.get(&new_val) {
      for (target_k, target_v) in linked {
        map.insert(target_k.clone(), target_v.clone());
      }
      return;
    }
  }

  // 2. Otherwise, check if any other parameter has `option_sets` referencing `changed_id`
  for p in params {
    if p.id == changed_id || p.option_sets.is_empty() {
      continue;
    }
    let references_changed = p
      .option_sets
      .values()
      .any(|pairs| pairs.iter().any(|(tk, _)| tk == changed_id));
    if !references_changed {
      continue;
    }

    // Check if the updated map matches one of `p`'s presets
    let mut matched_preset: Option<String> = None;
    for (opt_val, _) in &p.options {
      if let Some(pairs) = p.option_sets.get(opt_val) {
        if !pairs.is_empty()
          && pairs.iter().all(|(tk, tv)| {
            map
              .get(tk)
              .map(|cur| cur.trim() == tv.trim())
              .unwrap_or(false)
          })
        {
          matched_preset = Some(opt_val.clone());
          break;
        }
      }
    }

    if let Some(preset_val) = matched_preset {
      map.insert(p.id.clone(), preset_val);
    } else if p.options.iter().any(|(ov, _)| ov == "custom") {
      map.insert(p.id.clone(), "custom".to_string());
    }
  }
}

fn parse_numeric_param(raw: &str) -> Option<f64> {
  let trimmed = raw
    .trim()
    .trim_start_matches('+')
    .trim_end_matches("RH")
    .trim()
    .trim_end_matches('%')
    .trim_end_matches("mm³/s/mm")
    .trim_end_matches("mm³/s")
    .trim_end_matches("mm/s")
    .trim_end_matches("mm")
    .trim_end_matches("°C")
    .trim_end_matches('s')
    .trim();
  trimmed.parse::<f64>().ok()
}

/// Validate resolved parameter values against both:
/// 1. The declarative `params` schema (`Range` min/max bounds, `Enum` allowed options, `Boolean` format)
/// 2. The Lua script's `validate(params, ctx)` function (for cross-parameter combinations).
///
/// Returns `Ok(resolved_params)` if valid, or `Err(human_readable_reason)` if invalid so invalid
/// combinations cannot be saved or executed.
pub fn validate_lua_rule_params(
  script: &str,
  param_overrides: &HashMap<String, String>,
  context: &serde_json::Value,
) -> Result<HashMap<String, String>, String> {
  let spec = extract_lua_rule_spec(script)?;
  let resolved = resolve_lua_rule_params(&spec, param_overrides);

  // 1. Declarative schema validation (range / enum / boolean)
  for param_spec in &spec.params {
    let val_str = resolved
      .get(&param_spec.id)
      .cloned()
      .unwrap_or_else(|| param_spec.default_value.clone());

    match &param_spec.kind {
      LuaParamKind::Range { min, max, .. } => {
        let num = parse_numeric_param(&val_str).ok_or_else(|| {
          format!(
            "Parameter '{}' ({}) must be a valid number, got '{}'.",
            param_spec.label, param_spec.id, val_str
          )
        })?;
        if num < *min - 1e-9 || num > *max + 1e-9 {
          let unit_suffix = if param_spec.unit.is_empty() {
            String::new()
          } else {
            format!(" {}", param_spec.unit)
          };
          return Err(format!(
            "Parameter '{}' ({}) value {}{} is outside the allowed range [{}{}, {}{}].",
            param_spec.label, param_spec.id, num, unit_suffix, min, unit_suffix, max, unit_suffix
          ));
        }
      }
      LuaParamKind::Enum => {
        if !param_spec.options.is_empty()
          && !param_spec
            .options
            .iter()
            .any(|(allowed, _)| allowed == &val_str)
        {
          let allowed_list = param_spec
            .options
            .iter()
            .map(|(v, _)| format!("'{}'", v))
            .collect::<Vec<_>>()
            .join(", ");
          return Err(format!(
            "Parameter '{}' ({}) value '{}' is not one of the allowed enum options: [{}].",
            param_spec.label, param_spec.id, val_str, allowed_list
          ));
        }
      }
      LuaParamKind::Boolean => {
        let lower = val_str.trim().to_lowercase();
        if !matches!(lower.as_str(), "true" | "false" | "1" | "0" | "yes" | "no") {
          return Err(format!(
            "Parameter '{}' ({}) must be a boolean ('true' or 'false'), got '{}'.",
            param_spec.label, param_spec.id, val_str
          ));
        }
      }
      LuaParamKind::Text => {}
    }
  }

  // 2. Execute Lua `validate(params, ctx)` if defined
  if spec.has_validate_fn {
    let lexer = Lexer::new(script);
    let mut parser = Parser::new(lexer)?;
    let stmts = parser.parse_chunk()?;

    let mut interpreter = Interpreter::new();
    let env = Rc::new(RefCell::new(LuaEnv::new()));

    let lua_context = LuaVal::from_json(context);
    let mut params_map = HashMap::new();
    for (k, v) in &resolved {
      params_map.insert(k.clone(), LuaVal::String(v.clone()));
    }
    let lua_params = LuaVal::Table(Rc::new(RefCell::new(params_map)));

    env.borrow_mut().set_local("ctx", lua_context.clone());
    env.borrow_mut().set_local("context", lua_context.clone());
    env.borrow_mut().set_local("params", lua_params.clone());

    interpreter.eval_chunk(&stmts, env.clone())?;

    let validate_fn = env.borrow().get("validate");
    if let LuaVal::Function(_, _, _) = validate_fn {
      let ret =
        interpreter.call_function(&validate_fn, vec![lua_params, lua_context], env.clone())?;
      match ret {
        LuaVal::Boolean(true) | LuaVal::Nil => {}
        LuaVal::Boolean(false) => {
          return Err("Parameter combination rejected by Lua validate(params, ctx).".to_string());
        }
        LuaVal::String(err_msg) => {
          if !err_msg.trim().is_empty() {
            return Err(err_msg);
          }
        }
        LuaVal::Table(tbl) => {
          let tb = tbl.borrow();
          let ok_val = tb
            .get("ok")
            .or_else(|| tb.get("valid"))
            .or_else(|| tb.get("1"));
          let err_val = tb
            .get("error")
            .or_else(|| tb.get("message"))
            .or_else(|| tb.get("reason"))
            .or_else(|| tb.get("2"));
          if let Some(LuaVal::Boolean(false)) = ok_val {
            let msg = err_val
              .map(|v| v.to_lua_string())
              .filter(|s: &String| !s.is_empty())
              .unwrap_or_else(|| "Invalid parameter combination.".to_string());
            return Err(msg);
          }
        }
        _ => {}
      }
    }
  }

  Ok(resolved)
}

/// Evaluate `describe(params, ctx)` in a Lua rule script if present, returning dynamic rule title,
/// description, and badge text.
pub fn describe_lua_rule(
  script: &str,
  param_overrides: &HashMap<String, String>,
  scope: &str,
  context: &serde_json::Value,
) -> Result<LuaRuleDescription, String> {
  let spec = extract_lua_rule_spec(script)?;
  let resolved = resolve_lua_rule_params(&spec, param_overrides);

  let mut out = LuaRuleDescription {
    title: spec.meta.name.clone(),
    description: spec.meta.formula_summary.clone(),
    badge: spec.meta.icon.clone(),
  };

  if !spec.has_describe_fn {
    return Ok(out);
  }

  let lexer = Lexer::new(script);
  let mut parser = Parser::new(lexer)?;
  let stmts = parser.parse_chunk()?;

  let mut interpreter = Interpreter::new();
  let env = Rc::new(RefCell::new(LuaEnv::new()));

  let mut ctx_json = context.clone();
  if let Some(obj) = ctx_json.as_object_mut() {
    obj
      .entry("scope".to_string())
      .or_insert_with(|| serde_json::Value::String(scope.to_string()));
  }
  let lua_context = LuaVal::from_json(&ctx_json);
  let mut params_map = HashMap::new();
  for (k, v) in &resolved {
    params_map.insert(k.clone(), LuaVal::String(v.clone()));
  }
  let lua_params = LuaVal::Table(Rc::new(RefCell::new(params_map)));

  env.borrow_mut().set_local("ctx", lua_context.clone());
  env.borrow_mut().set_local("context", lua_context.clone());
  env.borrow_mut().set_local("params", lua_params.clone());

  interpreter.eval_chunk(&stmts, env.clone())?;

  let describe_fn = env.borrow().get("describe");
  if let LuaVal::Function(_, _, _) = describe_fn {
    let ret =
      interpreter.call_function(&describe_fn, vec![lua_params, lua_context], env.clone())?;
    match ret {
      LuaVal::Table(tbl) => {
        let tb = tbl.borrow();
        if let Some(t) = lua_table_get_str(&tb, "title").or_else(|| lua_table_get_str(&tb, "name"))
        {
          out.title = Some(t);
        }
        if let Some(d) = lua_table_get_str(&tb, "description") {
          out.description = Some(d);
        }
        if let Some(b) = lua_table_get_str(&tb, "badge") {
          out.badge = Some(b);
        }
      }
      LuaVal::String(s) => {
        out.description = Some(s);
      }
      _ => {}
    }
  }

  Ok(out)
}

fn escape_lua_string_literal(s: &str) -> String {
  s.replace('\\', "\\\\")
    .replace('"', "\\\"")
    .replace('\n', "\\n")
    .replace('\r', "")
}

/// Embed or update a `configured_params = { ... }` block at the top of a Lua rule script so
/// user-chosen parameter values travel with the script while keeping `params = { ... }` and
/// `validate(params, ctx)` intact.
pub fn embed_param_defaults_in_lua(script: &str, param_values: &HashMap<String, String>) -> String {
  let start_marker = "-- BEGIN_CONFIGURED_PARAMS";
  let end_marker = "-- END_CONFIGURED_PARAMS";

  let cleaned =
    if let (Some(s_idx), Some(e_idx)) = (script.find(start_marker), script.find(end_marker)) {
      if e_idx >= s_idx {
        let after = script[e_idx + end_marker.len()..].trim_start_matches('\n');
        format!("{}{}", &script[..s_idx], after)
      } else {
        script.to_string()
      }
    } else {
      script.to_string()
    };

  if param_values.is_empty() {
    return cleaned;
  }

  let mut keys: Vec<&String> = param_values.keys().collect();
  keys.sort();
  let mut block = String::from("-- BEGIN_CONFIGURED_PARAMS\nconfigured_params = {\n");
  for k in keys {
    if let Some(v) = param_values.get(k) {
      block.push_str(&format!(
        "  {} = \"{}\",\n",
        k,
        escape_lua_string_literal(v)
      ));
    }
  }
  block.push_str("}\n-- END_CONFIGURED_PARAMS\n\n");
  format!("{}{}", block, cleaned)
}

/// Execute a Lua script that contains an `apply(config, context, params)` function,
/// or runs directly at top level. Validates parameters before execution.
pub fn execute_transformation_lua(
  script: &str,
  config: &serde_json::Value,
  context: &serde_json::Value,
) -> Result<
  (
    serde_json::Value,
    Vec<String>,
    Vec<(String, String, String)>,
  ),
  String,
> {
  execute_transformation_lua_with_params(script, config, context, &HashMap::new())
}

/// Execute a Lua transformation script with explicit parameter overrides.
/// Automatically validates parameters against `params = { ... }` and `validate(params, ctx)` first.
pub fn execute_transformation_lua_with_params(
  script: &str,
  config: &serde_json::Value,
  context: &serde_json::Value,
  param_overrides: &HashMap<String, String>,
) -> Result<
  (
    serde_json::Value,
    Vec<String>,
    Vec<(String, String, String)>,
  ),
  String,
> {
  // 1. Pre-flight sandbox security validation (blocks io.*, os.*, dofile, require, debug, coroutine, etc.)
  validate_lua_sandbox_security(script)?;

  // 2. Extract and validate parameter specifications and Lua `validate(params, ctx)`
  let mut merged_overrides = HashMap::new();
  if let Some(ctx_params) = context.get("params").and_then(|v| v.as_object()) {
    for (k, v) in ctx_params {
      if let Some(s) = v.as_str() {
        merged_overrides.insert(k.clone(), s.to_string());
      } else {
        merged_overrides.insert(k.clone(), v.to_string());
      }
    }
  }
  for (k, v) in param_overrides {
    merged_overrides.insert(k.clone(), v.clone());
  }
  let validated_params = validate_lua_rule_params(script, &merged_overrides, context)?;

  let lexer = Lexer::new(script);
  let mut parser = Parser::new(lexer)?;
  let stmts = parser.parse_chunk()?;

  let mut interpreter = Interpreter::new();
  let env = Rc::new(RefCell::new(LuaEnv::new()));

  let lua_config = LuaVal::from_json(config);
  let lua_context = LuaVal::from_json(context);

  let mut params_map = HashMap::new();
  for (k, v) in &validated_params {
    params_map.insert(k.clone(), LuaVal::String(v.clone()));
  }
  let lua_params = LuaVal::Table(Rc::new(RefCell::new(params_map)));
  if let LuaVal::Table(ctx_tbl) = &lua_context {
    ctx_tbl
      .borrow_mut()
      .insert("params".to_string(), lua_params.clone());
  }

  env.borrow_mut().set_local("config", lua_config.clone());
  env.borrow_mut().set_local("ctx", lua_context.clone());
  env.borrow_mut().set_local("context", lua_context.clone());
  env.borrow_mut().set_local("params", lua_params.clone());

  // Execute the script
  interpreter.eval_chunk(&stmts, env.clone())?;

  // Re-bind `params` in case top-level `params = { ... }` schema declaration overwrote `params`
  env.borrow_mut().set_local("params", lua_params.clone());

  // If an `apply` function was defined, invoke it: apply(config, ctx, params)
  let apply_fn = env.borrow().get("apply");
  if let LuaVal::Function(_, _, _) = apply_fn {
    let _ = interpreter.call_function(
      &apply_fn,
      vec![lua_config.clone(), lua_context, lua_params],
      env.clone(),
    )?;
  }

  let mut resulting_config = lua_config.to_json();
  if let Some(obj) = resulting_config.as_object_mut() {
    for key in ["z_hop_types", "filament_z_hop_types"] {
      if let Some(val) = obj.get_mut(key) {
        let normalize = |s: &str| -> String {
          match s.trim() {
            "Normal" | "normal" => "Normal Lift".to_string(),
            "Spiral" | "spiral" => "Spiral Lift".to_string(),
            "Slope" | "slope" | "z-hop-slope" => "Slope Lift".to_string(),
            "Auto" | "auto" => "Auto Lift".to_string(),
            other => other.to_string(),
          }
        };
        match val {
          serde_json::Value::String(s) => {
            *s = normalize(s);
          }
          serde_json::Value::Array(arr) => {
            for elem in arr.iter_mut() {
              if let serde_json::Value::String(s) = elem {
                *s = normalize(s);
              }
            }
          }
          _ => {}
        }
      }
    }
  }
  Ok((
    resulting_config,
    interpreter.stdout,
    interpreter.modified_keys,
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_lua_petg_stringing_fix() {
    let initial_json = serde_json::json!({
        "nozzle_temperature": ["240"],
        "filament_retraction_length": ["0.8"],
        "wipe_on_loops": "0"
    });
    let context = serde_json::json!({
        "filament_type": "PETG",
        "nozzle_diameter": "0.4"
    });

    let script = r#"
            function apply(config, ctx)
                if ctx.filament_type == "PETG" then
                    local cur = tonumber(config["nozzle_temperature"][1])
                    config["nozzle_temperature"] = { tostring(cur - 5) }
                    local cur_retract = tonumber(config["filament_retraction_length"][1])
                    config["filament_retraction_length"] = { string.format("%.2f", cur_retract + 0.4) }
                    config["wipe_on_loops"] = "1"
                    print("Applied PETG stringing fix!")
                end
            end
        "#;

    let (result, stdout, modified) =
      execute_transformation_lua(script, &initial_json, &context).unwrap();
    assert_eq!(result["nozzle_temperature"][0], "235");
    assert_eq!(result["filament_retraction_length"][0], "1.20");
    assert_eq!(result["wipe_on_loops"], "1");
    assert_eq!(stdout, vec!["Applied PETG stringing fix!"]);
    assert!(modified.iter().any(|(k, _, _)| k == "nozzle_temperature"));
  }

  #[test]
  fn test_lua_diamondback_thermal_compensation() {
    let initial_json = serde_json::json!({
        "nozzle_temperature": ["210"],
        "filament_max_volumetric_speed": ["15"]
    });
    let context = serde_json::json!({
        "nozzle_type": "diamondback"
    });

    let script = r#"
            function apply(config, ctx)
                if ctx.nozzle_type == "diamondback" then
                    local temp = tonumber(config["nozzle_temperature"][1])
                    config["nozzle_temperature"] = { tostring(temp - 10) }
                    local mvs = tonumber(config["filament_max_volumetric_speed"][1])
                    config["filament_max_volumetric_speed"] = { string.format("%.1f", mvs * 1.2) }
                end
            end
        "#;

    let (result, _, _) = execute_transformation_lua(script, &initial_json, &context).unwrap();
    assert_eq!(result["nozzle_temperature"][0], "200");
    assert_eq!(result["filament_max_volumetric_speed"][0], "18.0");
  }

  #[test]
  fn test_all_embedded_lua_transformations() {
    let thermal_lua = include_str!("../transformations/thermal_conductivity.lua");
    let speed_lua = include_str!("../transformations/volumetric_speed_ceiling.lua");
    let flush_lua = include_str!("../transformations/flush_volumes_matrix.lua");
    let stringing_lua = include_str!("../transformations/symptom_stringing_diamondback.lua");
    let clog_lua = include_str!("../transformations/symptom_clog_ace_pro.lua");
    let adhesion_lua = include_str!("../transformations/symptom_bed_adhesion_petg.lua");
    let spaghetti_lua = include_str!("../transformations/symptom_spaghetti_pla.lua");

    let config = serde_json::json!({
        "nozzle_temperature": ["240"],
        "nozzle_temperature_initial_layer": ["245"],
        "hot_plate_temp_initial_layer": ["80"],
        "filament_max_volumetric_speed": ["15.0"],
        "layer_height": "0.20",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
        "inner_wall_speed": "300",
        "sparse_infill_speed": "350",
        "flush_multiplier": "1.0"
    });

    // 8-slot Double ACE Pro 2 context
    let ctx = serde_json::json!({
        "nozzle_material": "Diamondback PCD",
        "nozzle_type": "diamondback",
        "hotend_max_flow": 22.0,
        "filament_max_flow": 15.0,
        "melt_zone_multiplier": 1.25,
        "slots": [
            { "material_type": "PLA",  "color_hex": "#1A1A1A", "opacity": 1.0, "pigment_stickiness": 1.6 },
            { "material_type": "PLA",  "color_hex": "#FFFFFF", "opacity": 0.9, "pigment_stickiness": 0.6 },
            { "material_type": "PETG", "color_hex": "#F97316", "opacity": 0.8, "pigment_stickiness": 1.1 },
            { "material_type": "PLA",  "color_hex": "#E60000", "opacity": 1.0, "pigment_stickiness": 1.8 },
            { "material_type": "PLA",  "color_hex": "#06B6D4", "opacity": 0.4, "pigment_stickiness": 0.9 },
            { "material_type": "PETG", "color_hex": "#10B981", "opacity": 0.7, "pigment_stickiness": 1.0 },
            { "material_type": "ASA",  "color_hex": "#334155", "opacity": 1.0, "pigment_stickiness": 1.3 },
            { "material_type": "PLA",  "color_hex": "#FACC15", "opacity": 0.85, "pigment_stickiness": 0.8 }
        ]
    });

    let (c1, _, _) = execute_transformation_lua(thermal_lua, &config, &ctx).unwrap();
    assert_eq!(c1["nozzle_temperature"][0], "230");
    assert_eq!(c1["filament_flow_ratio"][0], "0.94");

    let (c2, _, _) = execute_transformation_lua(speed_lua, &c1, &ctx).unwrap();
    // min(22.0, 15.0) / (0.20 * 0.42) = 15.0 / 0.084 = 178 mm/s
    assert_eq!(c2["computed_safe_speed_ceiling"], "178");
    assert_eq!(c2["outer_wall_speed"], "178");

    let (c3, _, _) = execute_transformation_lua(flush_lua, &c2, &ctx).unwrap();
    let matrix = c3["flush_volumes_matrix"].as_array().unwrap();
    assert_eq!(matrix.len(), 64); // 8x8 Double ACE Pro 2 matrix!
    assert_eq!(matrix[0], "0"); // Diagonal 1->1
                                // Slot 1 (Black PLA, stickiness 1.6) -> Slot 2 (White PLA, opacity 0.9) should be high purge (>400)
    let purge_black_to_white: i32 = matrix[1].as_str().unwrap().parse().unwrap();
    assert!(purge_black_to_white >= 400);
    // Slot 1 (PLA, stickiness 1.6 -> capped 1.25x) -> Slot 3 (PETG) polymer cross-contamination -> 300 * 1.25 = 375 at 1.0x (563 at 1.5x)
    assert_eq!(matrix[2], "375");

    let (c4, _, _) = execute_transformation_lua(stringing_lua, &c3, &ctx).unwrap();
    assert_eq!(c4["filament_retraction_length"][0], "0.95");
    assert_eq!(c4["z_hop_types"][0], "Spiral Lift");

    let (c5, _, _) = execute_transformation_lua(clog_lua, &c4, &ctx).unwrap();
    assert_eq!(c5["flush_multiplier"], "1.45");

    let (c6, _, _) = execute_transformation_lua(adhesion_lua, &c5, &ctx).unwrap();
    assert_eq!(c6["initial_layer_line_width"], "0.55");
    assert_eq!(c6["hot_plate_temp_initial_layer"][0], "85");

    let (c7, _, _) = execute_transformation_lua(spaghetti_lua, &c6, &ctx).unwrap();
    assert_eq!(c7["sparse_infill_pattern"], "gyroid");
    assert_eq!(c7["reduce_crossing_wall"], "1");
  }

  #[test]
  fn test_lua_sandbox_protections_against_fork_bomb_fs_and_infinite_compute() {
    let cfg = serde_json::json!({"nozzle_temperature": "230"});
    let ctx = serde_json::json!({"filament_type": "PLA"});

    // 1. Filesystem access (io.open, dofile, loadfile) blocked
    let fs_script = r#"
      function apply(config, ctx)
        local f = io.open("/etc/passwd", "r")
      end
    "#;
    let err_fs = execute_transformation_lua(fs_script, &cfg, &ctx).unwrap_err();
    assert!(err_fs.contains("Filesystem access"));

    let dofile_script = r#"dofile("/tmp/evil.lua")"#;
    let err_dofile = execute_transformation_lua(dofile_script, &cfg, &ctx).unwrap_err();
    assert!(err_dofile.contains("External file execution"));

    // 2. OS / Process / Fork bomb via os.execute or os.fork blocked
    let os_script = r#"
      function apply(config, ctx)
        os.execute(":(){ :|:& };:")
      end
    "#;
    let err_os = execute_transformation_lua(os_script, &cfg, &ctx).unwrap_err();
    assert!(err_os.contains("Operating System / Process"));

    // 3. Recursive function fork bomb blocked by call_depth limit
    let fork_bomb_lua = r#"
      function bomb()
        bomb()
        bomb()
      end
      function apply(config, ctx)
        bomb()
      end
    "#;
    let err_bomb = execute_transformation_lua(fork_bomb_lua, &cfg, &ctx).unwrap_err();
    assert!(err_bomb.contains("recursion depth"));

    // 4. Infinite while loop blocked by gas budget
    let inf_while = r#"
      function apply(config, ctx)
        while true do
          config["nozzle_temperature"] = "235"
        end
      end
    "#;
    let err_while = execute_transformation_lua(inf_while, &cfg, &ctx).unwrap_err();
    assert!(err_while.contains("Execution step limit"));

    // 5. Numeric for loop with step = 0 blocked immediately
    let zero_step_for = r#"
      function apply(config, ctx)
        for i = 1, 10, 0 do
          config["nozzle_temperature"] = "235"
        end
      end
    "#;
    let err_for = execute_transformation_lua(zero_step_for, &cfg, &ctx).unwrap_err();
    assert!(err_for.contains("step cannot be 0"));

    // 6. Exponential string doubling memory bomb (s = s .. s) blocked
    let str_bomb = r#"
      function apply(config, ctx)
        local s = "aaaaaaaaaaaaaaaa"
        for i = 1, 25 do
          s = s .. s
        end
      end
    "#;
    let err_str = execute_transformation_lua(str_bomb, &cfg, &ctx).unwrap_err();
    assert!(err_str.contains("String concatenation"));

    // 7. Table allocation memory bomb blocked
    let tbl_bomb = r#"
      function apply(config, ctx)
        local t = {}
        for i = 1, 2000 do
          table.insert(t, i)
        end
      end
    "#;
    let err_tbl = execute_transformation_lua(tbl_bomb, &cfg, &ctx).unwrap_err();
    assert!(err_tbl.contains("maximum table size"));
  }
}
