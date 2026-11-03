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

/// Rust-owned helper prelude evaluated into every rule's environment before the rule script.
/// Rules never redefine these; keeping them here gives all `transformations/*.lua` files one
/// shared, versioned implementation of the table-shape plumbing (single `["225"]`-slot arrays
/// vs bare scalars) while the numeric curve helpers (`pwl`, `clamp`, `math.sqrt`, `^`) are
/// native Rust builtins.
const LUA_RULE_PRELUDE: &str = r#"
-- scalar_num(val, default): read a possibly array-wrapped numeric config slot.
-- Slicer keys arrive either as "235" or {"235"}; this returns 235.0 either way,
-- or `default` when absent / non-numeric.
function scalar_num(val, default)
  if type(val) == "table" then
    return tonumber(val[1]) or default
  end
  return tonumber(val) or default
end

-- map_num(tbl, fn, fmt): apply `fn` to every numeric slot of a config key (either an
-- array-valued key like {"235"} or a bare scalar "235"), re-formatting each result with
-- `string.format(fmt)` (default "%.3f") unless `config["_defer_quantization"] == "1"` is
-- active during Phase 2 of `TransformationEngine::evaluate_stack` (in which case full
-- IEEE-754 float precision `tostring(r)` is preserved and quantized once in Phase 3 Epilogue);
-- non-numeric slots pass through untouched and the key's shape (array vs scalar) is preserved.
-- A callback returning a non-number passes the ORIGINAL slot through (never the string.format
-- "0.00" placeholder) so a buggy future callback cannot silently corrupt a config value.
function map_num(tbl, fn, fmt)
  local defer_q = (type(config) == "table" and config["_defer_quantization"] == "1")
  if type(tbl) ~= "table" then
    local n = tonumber(tbl)
    if n then
      local r = fn(n)
      if type(r) == "number" then
        if defer_q then
          return tostring(r)
        end
        return string.format(fmt or "%.3f", r)
      end
      return tbl
    end
    return tbl
  end
  local out = {}
  for k, v in pairs(tbl) do
    local n = tonumber(v)
    if n then
      local r = fn(n)
      if type(r) == "number" then
        if defer_q then
          out[k] = tostring(r)
        else
          out[k] = string.format(fmt or "%.3f", r)
        end
      else
        out[k] = v
      end
    else
      out[k] = v
    end
  end
  return out
end

-- fmt_num(val, fmt): format a scalar numeric value with `string.format(fmt)` when running
-- standalone, or keep full IEEE-754 float precision `tostring(n)` during Phase 2 of
-- `TransformationEngine::evaluate_stack` (`config["_defer_quantization"] == "1"`) so all
-- quantization happens once at the very end of the pipeline.
function fmt_num(val, fmt)
  local n = tonumber(val)
  if not n then
    return tostring(val)
  end
  if type(config) == "table" and config["_defer_quantization"] == "1" then
    return tostring(n)
  end
  local f = fmt or "%.3f"
  if f == "%.0f" then
    if n >= 0 then
      return tostring(math.floor(n + 0.5))
    else
      return tostring(math.ceil(n - 0.5))
    end
  end
  return string.format(f, n)
end
"#;

/// Evaluate `LUA_RULE_PRELUDE` into `env`. Called by every code path that executes rule Lua
/// (transformation, `validate(params, ctx)`, `describe(params, ctx)`) so the helper surface is
/// identical no matter which entry point runs the script.
fn eval_rule_prelude(
  interpreter: &mut Interpreter,
  env: &Rc<RefCell<LuaEnv>>,
) -> Result<(), String> {
  let prelude_lexer = Lexer::new(LUA_RULE_PRELUDE);
  let mut prelude_parser = Parser::new(prelude_lexer)?;
  let prelude_stmts = prelude_parser.parse_chunk()?;
  interpreter.eval_chunk(&prelude_stmts, env.clone())?;
  Ok(())
}

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
        let snapped = (n * 1e9).round() / 1e9;
        if (snapped - snapped.round()).abs() < 1e-9 {
          format!("{:.0}", snapped.round())
        } else {
          let mut s = format!("{:.9}", snapped);
          while s.ends_with('0') {
            s.pop();
          }
          if s.ends_with('.') {
            s.pop();
          }
          s
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
  Pow,
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
  Caret,
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
      '^' => Ok(Token::Caret),
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
      self.parse_power()
    }
  }

  /// `^` power operator: binds tighter than unary minus (`-x^2` = `-(x^2)`),
  /// right-associative, and accepts a unary expression on the right (`2^-2`).
  fn parse_power(&mut self) -> Result<Expr, String> {
    let mut expr = self.parse_suffixed_expr()?;
    if self.match_token(&Token::Caret) {
      let right = self.parse_unary()?;
      expr = Expr::BinaryOp(BinaryOp::Pow, Box::new(expr), Box::new(right));
    }
    Ok(expr)
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
    // Curve helpers implemented natively in Rust (single deterministic evaluator, shared by
    // every rule — see `pwl` / `clamp` / `constrain` arms in `call_native`).
    self.set_local("pwl", LuaVal::NativeFn("pwl".to_string()));
    self.set_local("clamp", LuaVal::NativeFn("clamp".to_string()));
    self.set_local("constrain", LuaVal::NativeFn("constrain".to_string()));
    self.set_local(
      "capture_coupled_baseline",
      LuaVal::NativeFn("capture_coupled_baseline".to_string()),
    );
    self.set_local(
      "sync_coupled_physics",
      LuaVal::NativeFn("sync_coupled_physics".to_string()),
    );
    self.set_local(
      "coupled_purge_factor",
      LuaVal::NativeFn("coupled_purge_factor".to_string()),
    );
    self.set_local("bounds", LuaVal::Table(Rc::new(RefCell::new(HashMap::new()))));

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
    math_lib.insert(
      "sqrt".to_string(),
      LuaVal::NativeFn("math.sqrt".to_string()),
    );
    math_lib.insert("huge".to_string(), LuaVal::Number(f64::INFINITY));
    self.set_local("math", LuaVal::Table(Rc::new(RefCell::new(math_lib))));

    // table library
    let mut tbl_lib = HashMap::new();
    tbl_lib.insert(
      "insert".to_string(),
      LuaVal::NativeFn("table.insert".to_string()),
    );
    tbl_lib.insert(
      "concat".to_string(),
      LuaVal::NativeFn("table.concat".to_string()),
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
  pub config_table: Option<Rc<RefCell<HashMap<String, LuaVal>>>>,
  pub base_table: Option<Rc<RefCell<HashMap<String, LuaVal>>>>,
}

impl Interpreter {
  pub fn new() -> Self {
    Self {
      stdout: Vec::new(),
      gas_remaining: MAX_LUA_GAS,
      call_depth: 0,
      expr_depth: 0,
      modified_keys: Vec::new(),
      config_table: None,
      base_table: None,
    }
  }

  fn is_expr_rooted_in_base(expr: &Expr) -> bool {
    match expr {
      Expr::Variable(v) => v == "base",
      Expr::TableField(inner, _) | Expr::TableIndex(inner, _) => Self::is_expr_rooted_in_base(inner),
      _ => false,
    }
  }

  fn is_base_table_or_child(&self, target_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>) -> bool {
    let Some(base_rc) = &self.base_table else {
      return false;
    };
    if Rc::ptr_eq(target_tbl, base_rc) {
      return true;
    }
    let bb = base_rc.borrow();
    for v in bb.values() {
      if let LuaVal::Table(sub_rc) = v {
        if Rc::ptr_eq(target_tbl, sub_rc) {
          return true;
        }
      }
    }
    false
  }

  fn resolve_config_modification_key(
    &self,
    target_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
    key: &str,
  ) -> Option<String> {
    if key == "_bounds" || key == "_base" {
      return None;
    }
    match &self.config_table {
      Some(cfg_rc) => {
        if Rc::ptr_eq(target_tbl, cfg_rc) {
          Some(key.to_string())
        } else {
          let cb = cfg_rc.borrow();
          for (parent_k, parent_v) in cb.iter() {
            if parent_k == "_bounds" || parent_k == "_coupled_state" || parent_k == "_base" {
              continue;
            }
            if let LuaVal::Table(sub_rc) = parent_v {
              if Rc::ptr_eq(target_tbl, sub_rc) {
                return Some(parent_k.clone());
              }
            }
          }
          None
        }
      }
      None => Some(key.to_string()),
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
        if name == "base" {
          return Err(
            "[Lua Immutability Violation] Cannot reassign read-only 'base' (x0) snapshot table — mutate 'config' instead."
              .to_string(),
          );
        }
        env.borrow_mut().set(name, val);
        Ok(())
      }
      Expr::TableField(tbl_expr, field) => {
        if Self::is_expr_rooted_in_base(tbl_expr) {
          return Err(format!(
            "[Lua Immutability Violation] Cannot modify read-only 'base' (x0) snapshot table (key '{}') — mutate 'config' instead.",
            field
          ));
        }
        let is_bounds_target =
          matches!(tbl_expr.as_ref(), Expr::Variable(v) if v == "bounds") || field == "_bounds";
        let tbl_val = self.eval_expr(tbl_expr, env)?;
        if let LuaVal::Table(t) = tbl_val {
          if self.is_base_table_or_child(&t) {
            return Err(format!(
              "[Lua Immutability Violation] Cannot modify read-only 'base' (x0) snapshot table (key '{}') — mutate 'config' instead.",
              field
            ));
          }
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
          if !is_bounds_target {
            if let Some(cfg_key) = self.resolve_config_modification_key(&t, field) {
              self
                .modified_keys
                .push((cfg_key, old_val, val.to_lua_string()));
            }
          }
          Ok(())
        } else {
          Err(format!("Attempt to index a non-table: {:?}", tbl_val))
        }
      }
      Expr::TableIndex(tbl_expr, idx_expr) => {
        if Self::is_expr_rooted_in_base(tbl_expr) {
          return Err(
            "[Lua Immutability Violation] Cannot modify read-only 'base' (x0) snapshot table — mutate 'config' instead."
              .to_string(),
          );
        }
        let is_bounds_target = matches!(tbl_expr.as_ref(), Expr::Variable(v) if v == "bounds");
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
          if self.is_base_table_or_child(&t) {
            return Err(format!(
              "[Lua Immutability Violation] Cannot modify read-only 'base' (x0) snapshot table (key '{}') — mutate 'config' instead.",
              key_str
            ));
          }
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
          if !is_bounds_target && key_str != "_bounds" {
            if let Some(cfg_key) = self.resolve_config_modification_key(&t, &key_str) {
              self
                .modified_keys
                .push((cfg_key, old_val, val.to_lua_string()));
            }
          }
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
          BinaryOp::Pow => match (&left, &right) {
            (LuaVal::Number(a), LuaVal::Number(b)) => Ok(LuaVal::Number(a.powf(*b))),
            _ => Err("Arithmetic '^' on non-numbers".to_string()),
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
      LuaVal::NativeFn(name) => self.call_native(name, args, &env),
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

  fn call_native(
    &mut self,
    name: &str,
    args: Vec<LuaVal>,
    env: &Rc<RefCell<LuaEnv>>,
  ) -> Result<LuaVal, String> {
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
      "math.sqrt" => {
        if let Some(LuaVal::Number(n)) = args.get(0) {
          if *n < 0.0 {
            Err("math.sqrt domain error (negative argument)".to_string())
          } else {
            Ok(LuaVal::Number(n.sqrt()))
          }
        } else {
          Err("math.sqrt expects a number".to_string())
        }
      }
      // pwl(x, points): piecewise-linear curve evaluation with flat extrapolation.
      // `points` is an array of {x, y} pairs in any order: pwl(12, {{0,0},{10,10},{20,30}}) == 12.
      // Implemented natively in Rust so every rule shares one deterministic, sorted evaluator
      // (Lua-side `pairs` iteration is unordered in this interpreter).
      "pwl" => {
        let coerce = |v: Option<&LuaVal>| -> Option<f64> {
          match v {
            Some(LuaVal::Number(n)) => Some(*n),
            Some(LuaVal::String(s)) => s.trim().parse::<f64>().ok(),
            _ => None,
          }
        };
        let x = match coerce(args.get(0)) {
          Some(x) => x,
          None => return Err("pwl expects a numeric x value".to_string()),
        };
        let pts_tbl = match args.get(1) {
          Some(LuaVal::Table(t)) => t.clone(),
          _ => return Err("pwl expects a table of {x, y} points".to_string()),
        };
        let pts_map = pts_tbl.borrow();
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for i in 1..=pts_map.len() {
          let pair = match pts_map.get(&i.to_string()) {
            Some(LuaVal::Table(p)) => p.clone(),
            _ => return Err(format!("pwl point #{} is not a {{x, y}} pair table", i)),
          };
          let borrowed = pair.borrow();
          let px = coerce(borrowed.get(&"1".to_string()))
            .ok_or_else(|| format!("pwl point #{} is missing a numeric x (index 1)", i))?;
          let py = coerce(borrowed.get(&"2".to_string()))
            .ok_or_else(|| format!("pwl point #{} is missing a numeric y (index 2)", i))?;
          pts.push((px, py));
        }
        if pts.is_empty() {
          return Err("pwl expects at least one {x, y} point".to_string());
        }
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if x <= pts[0].0 {
          return Ok(LuaVal::Number(pts[0].1));
        }
        let last = pts[pts.len() - 1];
        if x >= last.0 {
          return Ok(LuaVal::Number(last.1));
        }
        for w in pts.windows(2) {
          let (x0, y0) = w[0];
          let (x1, y1) = w[1];
          if x >= x0 && x <= x1 {
            let t = if (x1 - x0).abs() < f64::EPSILON {
              0.0
            } else {
              (x - x0) / (x1 - x0)
            };
            return Ok(LuaVal::Number(y0 + t * (y1 - y0)));
          }
        }
        Ok(LuaVal::Number(last.1))
      }
      // clamp(x, lo, hi): min(max(x, lo), hi) with max applied first, so inverted bounds
      // (lo > hi) resolve to hi — matching the Lua-side math.min(math.max(x, lo), hi) idiom.
      "clamp" => {
        let coerce = |v: Option<&LuaVal>| -> Option<f64> {
          match v {
            Some(LuaVal::Number(n)) => Some(*n),
            Some(LuaVal::String(s)) => s.trim().parse::<f64>().ok(),
            _ => None,
          }
        };
        match (
          coerce(args.get(0)),
          coerce(args.get(1)),
          coerce(args.get(2)),
        ) {
          (Some(x), Some(lo), Some(hi)) => {
            let maxed = if x < lo { lo } else { x };
            Ok(LuaVal::Number(if maxed > hi { hi } else { maxed }))
          }
          _ => Err("clamp expects numeric (x, lo, hi)".to_string()),
        }
      }
      // constrain(key, min_val, max_val, reason): monotonically narrows the feasible interval
      // I_k = [L_k, U_k] via interval intersection [max(L_old, min_val), min(U_old, max_val)],
      // recording provenance attribution (min_by / max_by, rule_id, step_num) in `bounds[key]`
      // (which is shared with `config["_bounds"]` across stack steps).
      "constrain" => {
        let key = match args.get(0) {
          Some(LuaVal::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
          Some(other) if !matches!(other, LuaVal::Nil) => {
            let s = other.to_lua_string();
            if s.trim().is_empty() {
              return Ok(LuaVal::Nil);
            }
            s.trim().to_string()
          }
          _ => return Ok(LuaVal::Nil),
        };
        let coerce_opt = |v: Option<&LuaVal>| -> Option<f64> {
          match v {
            Some(LuaVal::Number(n)) if n.is_finite() => Some(*n),
            Some(LuaVal::String(s)) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
            _ => None,
          }
        };
        let min_v = coerce_opt(args.get(1));
        let max_v = coerce_opt(args.get(2));
        let reason_arg = match args.get(3) {
          Some(LuaVal::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
          _ => None,
        };

        let (ctx_rule_name, ctx_rule_id, ctx_step_num) = {
          let mut r_name = None;
          let mut r_id = String::new();
          let mut s_num = 0usize;
          if let LuaVal::String(s) = env.borrow().get("current_rule_name") {
            if !s.trim().is_empty() {
              r_name = Some(s.trim().to_string());
            }
          }
          if let LuaVal::Table(ctx_tbl) = env.borrow().get("ctx") {
            let cb = ctx_tbl.borrow();
            if r_name.is_none() {
              if let Some(LuaVal::String(s)) = cb.get("_current_rule_name") {
                if !s.trim().is_empty() {
                  r_name = Some(s.trim().to_string());
                }
              }
            }
            if let Some(LuaVal::String(s)) = cb.get("_current_rule_id") {
              r_id = s.trim().to_string();
            }
            if let Some(LuaVal::Number(n)) = cb.get("_current_step_num") {
              if *n > 0.0 {
                s_num = *n as usize;
              }
            }
          }
          if r_name.is_none() || r_id.is_empty() {
            if let LuaVal::Table(meta_tbl) = env.borrow().get("meta") {
              let mb = meta_tbl.borrow();
              if r_name.is_none() {
                if let Some(LuaVal::String(s)) = mb.get("name") {
                  if !s.trim().is_empty() {
                    r_name = Some(s.trim().to_string());
                  }
                }
              }
              if r_id.is_empty() {
                if let Some(LuaVal::String(s)) = mb.get("id") {
                  r_id = s.trim().to_string();
                }
              }
            }
          }
          (
            r_name.unwrap_or_else(|| "default".to_string()),
            r_id,
            s_num,
          )
        };
        let reason = reason_arg.unwrap_or(ctx_rule_name);

        let bounds_tbl = match env.borrow().get("bounds") {
          LuaVal::Table(t) => t,
          _ => {
            let t = Rc::new(RefCell::new(HashMap::new()));
            env.borrow_mut().set("bounds", LuaVal::Table(t.clone()));
            t
          }
        };
        if bounds_tbl.borrow().len() >= MAX_LUA_TABLE_ENTRIES
          && !bounds_tbl.borrow().contains_key(&key)
        {
          return Err(format!(
            "[Lua Sandbox Security Violation] bounds table exceeded maximum size ({} entries).",
            MAX_LUA_TABLE_ENTRIES
          ));
        }

        let entry_tbl = {
          let existing = bounds_tbl.borrow().get(&key).cloned();
          match existing {
            Some(LuaVal::Table(t)) => t,
            _ => {
              let mut init_map = HashMap::new();
              init_map.insert("min".to_string(), LuaVal::Number(f64::NEG_INFINITY));
              init_map.insert("max".to_string(), LuaVal::Number(f64::INFINITY));
              init_map.insert("min_by".to_string(), LuaVal::String("default".to_string()));
              init_map.insert("max_by".to_string(), LuaVal::String("default".to_string()));
              let t = Rc::new(RefCell::new(init_map));
              bounds_tbl
                .borrow_mut()
                .insert(key.clone(), LuaVal::Table(t.clone()));
              t
            }
          }
        };

        {
          let mut eb = entry_tbl.borrow_mut();
          let cur_min = match eb.get("min") {
            Some(LuaVal::Number(n)) if !n.is_nan() => *n,
            Some(LuaVal::String(s)) => s.trim().parse::<f64>().unwrap_or(f64::NEG_INFINITY),
            _ => f64::NEG_INFINITY,
          };
          let cur_max = match eb.get("max") {
            Some(LuaVal::Number(n)) if !n.is_nan() => *n,
            Some(LuaVal::String(s)) => s.trim().parse::<f64>().unwrap_or(f64::INFINITY),
            _ => f64::INFINITY,
          };
          if !eb.contains_key("min") || matches!(eb.get("min"), Some(LuaVal::Nil)) {
            eb.insert("min".to_string(), LuaVal::Number(cur_min));
          }
          if !eb.contains_key("max") || matches!(eb.get("max"), Some(LuaVal::Nil)) {
            eb.insert("max".to_string(), LuaVal::Number(cur_max));
          }

          if let Some(mv) = min_v {
            if mv > cur_min {
              eb.insert("min".to_string(), LuaVal::Number(mv));
              eb.insert("min_by".to_string(), LuaVal::String(reason.clone()));
              if !ctx_rule_id.is_empty() {
                eb.insert(
                  "min_rule_id".to_string(),
                  LuaVal::String(ctx_rule_id.clone()),
                );
              }
              if ctx_step_num > 0 {
                eb.insert("min_step".to_string(), LuaVal::Number(ctx_step_num as f64));
              }
            }
          }
          if let Some(xv) = max_v {
            if xv < cur_max {
              eb.insert("max".to_string(), LuaVal::Number(xv));
              eb.insert("max_by".to_string(), LuaVal::String(reason));
              if !ctx_rule_id.is_empty() {
                eb.insert("max_rule_id".to_string(), LuaVal::String(ctx_rule_id));
              }
              if ctx_step_num > 0 {
                eb.insert("max_step".to_string(), LuaVal::Number(ctx_step_num as f64));
              }
            }
          }
        }

        if let LuaVal::Table(cfg_tbl) = env.borrow().get("config") {
          cfg_tbl
            .borrow_mut()
            .insert("_bounds".to_string(), LuaVal::Table(bounds_tbl));
        }

        Ok(LuaVal::Table(entry_tbl))
      }
      "capture_coupled_baseline" => {
        let cfg_tbl = match args.get(0).cloned().unwrap_or_else(|| env.borrow().get("config")) {
          LuaVal::Table(t) => t,
          _ => return Ok(LuaVal::Nil),
        };
        let state_tbl = ensure_coupled_state_table(&cfg_tbl);
        if !state_tbl.borrow().contains_key("base") {
          if let Some(ref base_rc) = self.base_table {
            state_tbl
              .borrow_mut()
              .insert("base".to_string(), LuaVal::Table(base_rc.clone()));
          }
        }
        let cur_t_opt = lua_refcell_get_scalar_f64(&cfg_tbl, "nozzle_temperature");
        if let Some(cur_t) = cur_t_opt {
          state_tbl
            .borrow_mut()
            .insert("t0".to_string(), LuaVal::Number(cur_t));
        }
        state_tbl
          .borrow_mut()
          .insert("awaiting_p10_eq".to_string(), LuaVal::Boolean(true));
        Ok(LuaVal::Nil)
      }
      "sync_coupled_physics" => {
        let cfg_tbl = match args.get(0).cloned().unwrap_or_else(|| env.borrow().get("config")) {
          LuaVal::Table(t) => t,
          _ => return Ok(LuaVal::Nil),
        };
        let ctx_tbl = match args.get(1).cloned().unwrap_or_else(|| env.borrow().get("ctx")) {
          LuaVal::Table(t) => t,
          _ => Rc::new(RefCell::new(HashMap::new())),
        };
        self.apply_sync_coupled_physics(&cfg_tbl, &ctx_tbl);
        Ok(LuaVal::Nil)
      }
      "coupled_purge_factor" => {
        let cfg_tbl = match args.get(0).cloned().unwrap_or_else(|| env.borrow().get("config")) {
          LuaVal::Table(t) => t,
          _ => Rc::new(RefCell::new(HashMap::new())),
        };
        let ctx_tbl = match args.get(1).cloned().unwrap_or_else(|| env.borrow().get("ctx")) {
          LuaVal::Table(t) => t,
          _ => Rc::new(RefCell::new(HashMap::new())),
        };
        let factor = compute_coupled_purge_factor(&cfg_tbl, &ctx_tbl);
        Ok(LuaVal::Number(factor))
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
      "table.concat" => {
        if let Some(LuaVal::Table(t)) = args.get(0) {
          let sep = args
            .get(1)
            .map(|v| match v {
              LuaVal::Nil => String::new(),
              other => other.to_lua_string(),
            })
            .unwrap_or_default();
          let map = t.borrow();
          let mut parts = Vec::with_capacity(map.len());
          let mut idx = 1usize;
          while let Some(v) = map.get(&idx.to_string()) {
            parts.push(v.to_lua_string());
            idx += 1;
          }
          let joined = parts.join(&sep);
          if joined.len() > MAX_LUA_STRING_BYTES {
            return Err(format!(
              "[Lua Sandbox Security Violation] table.concat result ({} bytes) exceeds {} byte limit.",
              joined.len(),
              MAX_LUA_STRING_BYTES
            ));
          }
          Ok(LuaVal::String(joined))
        } else {
          Ok(LuaVal::String(String::new()))
        }
      }
      _ => Err(format!("Unknown native function: {}", name)),
    }
  }

  fn update_coupled_config_key(
    &mut self,
    cfg_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
    key: &str,
    new_str: String,
  ) {
    let existing = cfg_tbl.borrow().get(key).cloned();
    match existing {
      Some(LuaVal::Table(arr_tbl)) => {
        let old_str = arr_tbl
          .borrow()
          .get("1")
          .map(|v| v.to_lua_string())
          .unwrap_or_default();
        if old_str != new_str {
          let mut new_arr = HashMap::new();
          new_arr.insert("1".to_string(), LuaVal::String(new_str.clone()));
          cfg_tbl
            .borrow_mut()
            .insert(key.to_string(), LuaVal::Table(Rc::new(RefCell::new(new_arr))));
          self
            .modified_keys
            .push((key.to_string(), old_val_or_nil(&old_str), new_str));
        }
      }
      Some(val) if !matches!(val, LuaVal::Nil) => {
        let old_str = val.to_lua_string();
        if old_str != new_str {
          cfg_tbl
            .borrow_mut()
            .insert(key.to_string(), LuaVal::String(new_str.clone()));
          self
            .modified_keys
            .push((key.to_string(), old_val_or_nil(&old_str), new_str));
        }
      }
      _ => {}
    }
  }

  fn apply_sync_coupled_physics(
    &mut self,
    cfg_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
    ctx_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
  ) {
    let state_tbl = ensure_coupled_state_table(cfg_tbl);
    let cur_temp_opt = lua_refcell_get_scalar_f64(cfg_tbl, "nozzle_temperature");

    {
      let mut sb = state_tbl.borrow_mut();
      let awaiting = matches!(sb.get("awaiting_p10_eq"), Some(LuaVal::Boolean(true)));
      if awaiting {
        if let Some(t_cur) = cur_temp_opt {
          sb.insert("t_eq".to_string(), LuaVal::Number(t_cur));
        }
        sb.insert("awaiting_p10_eq".to_string(), LuaVal::Boolean(false));
      } else if !sb.contains_key("t_eq") {
        let pre_mut_t = self
          .modified_keys
          .iter()
          .find(|(k, _, _)| k == "nozzle_temperature")
          .and_then(|(_, old_v, _)| old_v.trim().parse::<f64>().ok());
        if let Some(t_init) = pre_mut_t.or(cur_temp_opt) {
          sb.insert("t_eq".to_string(), LuaVal::Number(t_init));
        }
      }
    }

    let already_modified_in_rule =
      |modified: &[(String, String, String)], key: &str| modified.iter().any(|(k, _, _)| k == key);
    let defer_quant = lua_refcell_get_scalar_f64(cfg_tbl, "_defer_quantization") == Some(1.0);

    let props = extract_coupled_physics_props(cfg_tbl, ctx_tbl, &state_tbl);

    // 1. Coupled Pressure Advance: PA = PA_0 * c_trib * (0.40 / d)^1.35 * exp(-alpha_eta * dt_net)
    let c_trib = if props.is_diamond {
      0.85
    } else if props.is_cht {
      0.90
    } else if props.is_cm2 || props.is_twinclad {
      0.95
    } else {
      1.00
    };
    let alpha_eta = if props.is_copolyester {
      0.014
    } else if props.is_elastomer {
      0.010
    } else {
      0.012
    };
    let target_pa_factor = (c_trib
      * (0.40 / props.nozzle_diameter).powf(1.35)
      * (-alpha_eta * props.dt_net).exp())
    .clamp(0.25, 2.50);

    let prev_pa_factor = lua_refcell_get_scalar_f64(&state_tbl, "prev_pa_factor")
      .filter(|v| *v > 0.0)
      .unwrap_or(1.0);
    let step_pa_ratio = target_pa_factor / prev_pa_factor;
    if (step_pa_ratio - 1.0).abs() > 1e-6 {
      if !already_modified_in_rule(&self.modified_keys, "pressure_advance") {
        if let Some(cur_pa) = lua_refcell_get_scalar_f64(cfg_tbl, "pressure_advance") {
          if cur_pa > 0.0 {
            let new_pa = (cur_pa * step_pa_ratio).clamp(0.005, 0.120);
            let pa_str = if defer_quant {
              LuaVal::Number(new_pa).to_lua_string()
            } else {
              format!("{:.3}", new_pa)
            };
            self.update_coupled_config_key(cfg_tbl, "pressure_advance", pa_str);
          }
        }
      }
      state_tbl
        .borrow_mut()
        .insert("prev_pa_factor".to_string(), LuaVal::Number(target_pa_factor));
    }

    // 2. Coupled Max Volumetric Speed: MVS = MVS_0 * c_bore * (d / 0.40)^beta_Q * c_T(dt_net)
    let c_bore = if props.is_cht { 1.25 } else { 1.00 };
    let beta_q = if props.nozzle_diameter < 0.40 {
      0.85
    } else if props.is_diamond {
      0.42
    } else if props.is_cht || props.is_cm2 {
      0.35
    } else if props.is_hardened {
      0.22
    } else {
      0.30
    };
    let c_t = if props.dt_net < 0.0 {
      1.0 + 0.012 * props.dt_net
    } else {
      1.0 + 0.008 * props.dt_net
    };
    let target_mvs_factor = (c_bore * (props.nozzle_diameter / 0.40).powf(beta_q) * c_t)
      .clamp(0.35, 1.85);

    let prev_mvs_factor = lua_refcell_get_scalar_f64(&state_tbl, "prev_mvs_factor")
      .filter(|v| *v > 0.0)
      .unwrap_or(1.0);
    let step_mvs_ratio = target_mvs_factor / prev_mvs_factor;
    if (step_mvs_ratio - 1.0).abs() > 1e-6 {
      let skip_machine_placeholder = {
        let cb = cfg_tbl.borrow();
        let is_machine = lua_table_get_str(&cb, "type")
          .map(|s| s.eq_ignore_ascii_case("machine"))
          .unwrap_or(false);
        let base_mvs = self
          .base_table
          .as_ref()
          .and_then(|b| lua_table_get_scalar_f64(&b.borrow(), "filament_max_volumetric_speed"))
          .or_else(|| lua_table_get_scalar_f64(&cb, "filament_max_volumetric_speed"))
          .unwrap_or(0.0);
        let ctx_mvs = lua_refcell_get_scalar_f64(ctx_tbl, "filament_max_flow");
        is_machine
          && (base_mvs - 12.0).abs() < 0.01
          && ctx_mvs.map(|cm| (cm - 12.0).abs() > 0.01).unwrap_or(false)
      };
      if !skip_machine_placeholder
        && !already_modified_in_rule(&self.modified_keys, "filament_max_volumetric_speed")
      {
        if let Some(cur_mvs) = lua_refcell_get_scalar_f64(cfg_tbl, "filament_max_volumetric_speed")
        {
          if cur_mvs > 0.0 {
            let hotend_limit = lua_refcell_get_scalar_f64(ctx_tbl, "hotend_max_flow")
              .filter(|v| *v > 0.0)
              .unwrap_or(35.0);
            let new_mvs = (cur_mvs * step_mvs_ratio).clamp(1.5, hotend_limit);
            let mvs_str = if defer_quant {
              LuaVal::Number(new_mvs).to_lua_string()
            } else {
              format!("{:.1}", new_mvs)
            };
            self.update_coupled_config_key(cfg_tbl, "filament_max_volumetric_speed", mvs_str);
          }
        }
      }
      state_tbl.borrow_mut().insert(
        "prev_mvs_factor".to_string(),
        LuaVal::Number(target_mvs_factor),
      );
    }

    // 3. Coupled Cooling Fan Curves: fan_min_speed, fan_max_speed, overhang_fan_speed
    let dfan_rad = if props.is_warp_prone {
      0.0
    } else if props.is_diamond {
      6.0 * (props.nozzle_diameter / 0.40).sqrt()
    } else if props.is_cm2 || props.is_cht || props.is_twinclad {
      2.0 * (props.nozzle_diameter / 0.40).sqrt()
    } else {
      0.0
    };
    let k_t = if props.is_warp_prone {
      0.0
    } else if props.is_copolyester || props.is_elastomer {
      1.0
    } else {
      1.2
    };
    let dt_fan = k_t * props.dt_net;
    let structural_fan_cut =
      lua_refcell_get_scalar_f64(cfg_tbl, "_structural_fan_cut_pct").unwrap_or(0.0);
    let dt_fan_part = if structural_fan_cut > 0.0 && dt_fan > 0.0 {
      0.0
    } else {
      dt_fan
    };

    let target_fan_min_delta = if defer_quant {
      0.75 * dfan_rad + dt_fan_part
    } else {
      (0.75 * dfan_rad + dt_fan_part).round()
    };
    let target_fan_max_delta = if defer_quant {
      0.50 * dfan_rad + dt_fan_part
    } else {
      (0.50 * dfan_rad + dt_fan_part).round()
    };
    let target_ovh_fan_delta = if defer_quant {
      dfan_rad + 0.50 * dt_fan
    } else {
      (dfan_rad + 0.50 * dt_fan).round()
    };

    let prev_fan_min_delta =
      lua_refcell_get_scalar_f64(&state_tbl, "prev_fan_min_delta").unwrap_or(0.0);
    let prev_fan_max_delta =
      lua_refcell_get_scalar_f64(&state_tbl, "prev_fan_max_delta").unwrap_or(0.0);
    let prev_ovh_fan_delta =
      lua_refcell_get_scalar_f64(&state_tbl, "prev_ovh_fan_delta").unwrap_or(0.0);

    let step_fan_min = target_fan_min_delta - prev_fan_min_delta;
    if step_fan_min.abs() > 1e-6 {
      for fan_key in ["fan_min_speed", "fan_min_speed_BRASS", "fan_min_speed_HS"] {
        if !already_modified_in_rule(&self.modified_keys, fan_key) {
          if let Some(cur_fan) = lua_refcell_get_scalar_f64(cfg_tbl, fan_key) {
            let new_fan = (cur_fan + step_fan_min).clamp(0.0, 100.0);
            let fan_str = if defer_quant {
              LuaVal::Number(new_fan).to_lua_string()
            } else {
              (new_fan.round() as i32).to_string()
            };
            self.update_coupled_config_key(cfg_tbl, fan_key, fan_str);
          }
        }
      }
      state_tbl.borrow_mut().insert(
        "prev_fan_min_delta".to_string(),
        LuaVal::Number(target_fan_min_delta),
      );
    }

    let step_fan_max = target_fan_max_delta - prev_fan_max_delta;
    if step_fan_max.abs() > 1e-6 {
      let cur_min_floor = lua_refcell_get_scalar_f64(cfg_tbl, "fan_min_speed").unwrap_or(0.0);
      for fan_key in ["fan_max_speed", "fan_max_speed_BRASS", "fan_max_speed_HS"] {
        if !already_modified_in_rule(&self.modified_keys, fan_key) {
          if let Some(cur_fan) = lua_refcell_get_scalar_f64(cfg_tbl, fan_key) {
            let new_fan = (cur_fan + step_fan_max)
              .max(cur_min_floor)
              .clamp(0.0, 100.0);
            let fan_str = if defer_quant {
              LuaVal::Number(new_fan).to_lua_string()
            } else {
              (new_fan.round() as i32).to_string()
            };
            self.update_coupled_config_key(cfg_tbl, fan_key, fan_str);
          }
        }
      }
      state_tbl.borrow_mut().insert(
        "prev_fan_max_delta".to_string(),
        LuaVal::Number(target_fan_max_delta),
      );
    }

    let step_ovh_fan = target_ovh_fan_delta - prev_ovh_fan_delta;
    if step_ovh_fan.abs() > 1e-6 {
      if !already_modified_in_rule(&self.modified_keys, "overhang_fan_speed") {
        if let Some(cur_ovh) = lua_refcell_get_scalar_f64(cfg_tbl, "overhang_fan_speed") {
          let new_ovh = (cur_ovh + step_ovh_fan).clamp(0.0, 100.0);
          let ovh_str = if defer_quant {
            LuaVal::Number(new_ovh).to_lua_string()
          } else {
            (new_ovh.round() as i32).to_string()
          };
          self.update_coupled_config_key(cfg_tbl, "overhang_fan_speed", ovh_str);
        }
      }
      state_tbl.borrow_mut().insert(
        "prev_ovh_fan_delta".to_string(),
        LuaVal::Number(target_ovh_fan_delta),
      );
    }
  }
}

fn old_val_or_nil(s: &str) -> String {
  if s.is_empty() {
    "nil".to_string()
  } else {
    s.to_string()
  }
}

fn lua_val_scalar_f64(val: &LuaVal) -> Option<f64> {
  match val {
    LuaVal::Number(n) if !n.is_nan() => Some(*n),
    LuaVal::String(s) => s.trim().trim_end_matches('%').trim().parse::<f64>().ok(),
    LuaVal::Table(t) => t.borrow().get("1").and_then(lua_val_scalar_f64),
    _ => None,
  }
}

fn lua_table_get_scalar_f64(map: &HashMap<String, LuaVal>, key: &str) -> Option<f64> {
  map.get(key).and_then(lua_val_scalar_f64)
}

fn lua_refcell_get_scalar_f64(
  tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
  key: &str,
) -> Option<f64> {
  let borrow = tbl.borrow();
  lua_table_get_scalar_f64(&borrow, key)
}

fn ensure_coupled_state_table(
  cfg_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
) -> Rc<RefCell<HashMap<String, LuaVal>>> {
  if let Some(LuaVal::Table(existing)) = cfg_tbl.borrow().get("_coupled_state").cloned() {
    return existing;
  }
  let mut init_map = HashMap::new();
  if let Some(t_cur) = lua_refcell_get_scalar_f64(cfg_tbl, "nozzle_temperature") {
    init_map.insert("t0".to_string(), LuaVal::Number(t_cur));
  }
  init_map.insert("prev_pa_factor".to_string(), LuaVal::Number(1.0));
  init_map.insert("prev_mvs_factor".to_string(), LuaVal::Number(1.0));
  init_map.insert("prev_fan_min_delta".to_string(), LuaVal::Number(0.0));
  init_map.insert("prev_fan_max_delta".to_string(), LuaVal::Number(0.0));
  init_map.insert("prev_ovh_fan_delta".to_string(), LuaVal::Number(0.0));
  init_map.insert("awaiting_p10_eq".to_string(), LuaVal::Boolean(false));
  let tbl = Rc::new(RefCell::new(init_map));
  cfg_tbl
    .borrow_mut()
    .insert("_coupled_state".to_string(), LuaVal::Table(tbl.clone()));
  tbl
}

struct CoupledPhysicsProps {
  is_diamond: bool,
  is_cht: bool,
  is_cm2: bool,
  is_twinclad: bool,
  is_hardened: bool,
  nozzle_diameter: f64,
  is_elastomer: bool,
  is_copolyester: bool,
  is_warp_prone: bool,
  dt_net: f64,
  melt_zone_multiplier: f64,
}

fn extract_coupled_physics_props(
  cfg_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
  ctx_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
  state_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
) -> CoupledPhysicsProps {
  let cb = cfg_tbl.borrow();
  let xb = ctx_tbl.borrow();
  let sb = state_tbl.borrow();

  let n_type = lua_table_get_str(&xb, "nozzle_type")
    .or_else(|| lua_table_get_str(&cb, "nozzle_type"))
    .unwrap_or_else(|| "brass".to_string())
    .to_lowercase();
  let n_mat = lua_table_get_str(&xb, "nozzle_material")
    .unwrap_or_default()
    .to_lowercase();

  let is_diamond = n_type.contains("diamondback")
    || n_type.contains("diamond")
    || n_mat.contains("diamondback")
    || n_mat.contains("diamond");
  let is_cht = n_type.contains("cht") || n_mat.contains("cht");
  let is_cm2 = n_type.contains("cm2") || n_mat.contains("cm2");
  let is_twinclad = n_type.contains("microswiss_brass")
    || n_type.contains("twinclad")
    || n_mat.contains("twinclad");
  let is_hardened = !is_diamond
    && !is_cht
    && !is_cm2
    && !is_twinclad
    && (n_type.contains("hardened")
      || n_type.contains("stainless")
      || n_mat.contains("hardened")
      || n_mat.contains("stainless"));

  let nozzle_diameter = lua_table_get_scalar_f64(&xb, "nozzle_diameter")
    .or_else(|| lua_table_get_scalar_f64(&cb, "nozzle_diameter"))
    .unwrap_or(0.40)
    .clamp(0.20, 1.20);

  let fil_type = lua_table_get_str(&xb, "filament_type")
    .or_else(|| {
      cb.get("filament_type").and_then(|v| match v {
        LuaVal::Table(t) => t.borrow().get("1").map(|x| x.to_lua_string()),
        other => Some(other.to_lua_string()),
      })
    })
    .unwrap_or_else(|| "PLA".to_string())
    .to_uppercase();

  let is_elastomer = matches!(xb.get("filament_is_elastomer"), Some(LuaVal::Boolean(true)))
    || fil_type.contains("TPU")
    || fil_type.contains("TPE")
    || fil_type.contains("PEBA")
    || fil_type.contains("FLEX");
  let is_copolyester = fil_type.contains("PETG") || fil_type.contains("PCTG");
  let is_warp_prone = matches!(xb.get("filament_is_warp_prone"), Some(LuaVal::Boolean(true)))
    || fil_type.contains("ABS")
    || fil_type.contains("ASA")
    || fil_type == "PA"
    || fil_type.starts_with("PA-")
    || fil_type.starts_with("PA6")
    || fil_type.starts_with("PA12")
    || fil_type.starts_with("PPA")
    || fil_type.starts_with("PAHT")
    || fil_type.contains("NYLON")
    || fil_type == "PC"
    || fil_type.starts_with("PC-")
    || fil_type == "PET-CF"
    || fil_type == "PET-GF"
    || fil_type.contains("HIPS")
    || fil_type.contains("PEEK")
    || fil_type.contains("PEI")
    || fil_type.contains("ULTEM")
    || fil_type.contains("PSU")
    || fil_type.contains("PPSU")
    || fil_type.contains("PPS")
    || fil_type == "PP"
    || fil_type.starts_with("PP-");

  let mut cur_temp = lua_table_get_scalar_f64(&cb, "nozzle_temperature");
  if let (Some(tc), Some(LuaVal::Table(b_tbl))) = (cur_temp, cb.get("_bounds")) {
    if let Some(LuaVal::Table(noz_b)) = b_tbl.borrow().get("nozzle_temperature") {
      let nb = noz_b.borrow();
      let b_min = nb.get("min").and_then(lua_val_scalar_f64).unwrap_or(f64::NEG_INFINITY);
      let b_max = nb.get("max").and_then(lua_val_scalar_f64).unwrap_or(f64::INFINITY);
      if b_min <= b_max {
        cur_temp = Some(tc.clamp(b_min, b_max));
      }
    }
  }
  let t_eq = lua_table_get_scalar_f64(&sb, "t_eq").or(cur_temp);
  let pre_delta = lua_table_get_scalar_f64(&xb, "pre_temp_delta_c").unwrap_or(0.0);
  let dt_net = match (cur_temp, t_eq) {
    (Some(tc), Some(te)) => ((tc - te) + pre_delta).clamp(-15.0, 15.0),
    _ => pre_delta.clamp(-15.0, 15.0),
  };

  let melt_zone_multiplier = lua_table_get_scalar_f64(&xb, "melt_zone_multiplier")
    .unwrap_or(1.00)
    .clamp(0.80, 1.50);

  CoupledPhysicsProps {
    is_diamond,
    is_cht,
    is_cm2,
    is_twinclad,
    is_hardened,
    nozzle_diameter,
    is_elastomer,
    is_copolyester,
    is_warp_prone,
    dt_net,
    melt_zone_multiplier,
  }
}

fn compute_coupled_purge_factor(
  cfg_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
  ctx_tbl: &Rc<RefCell<HashMap<String, LuaVal>>>,
) -> f64 {
  let state_tbl = ensure_coupled_state_table(cfg_tbl);
  let props = extract_coupled_physics_props(cfg_tbl, ctx_tbl, &state_tbl);

  let c_vol = props.melt_zone_multiplier;
  let c_wall_slip = if props.is_diamond {
    0.80
  } else if props.is_cm2 || props.is_twinclad {
    0.94
  } else if props.is_cht {
    1.08
  } else if props.is_hardened {
    1.04
  } else {
    1.00
  };
  let beta_eta = if props.is_copolyester { 0.015 } else { 0.012 };
  let c_visc = (-beta_eta * props.dt_net).exp();
  (c_vol * c_wall_slip * c_visc).clamp(0.70, 1.65)
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
  eval_rule_prelude(&mut interpreter, &env)?;

  let dummy_cfg = LuaVal::from_json(&serde_json::json!({}));
  let dummy_base = LuaVal::from_json(&serde_json::json!({}));
  let dummy_ctx = LuaVal::from_json(&serde_json::json!({}));
  if let LuaVal::Table(ref t) = dummy_base {
    interpreter.base_table = Some(t.clone());
  }
  env.borrow_mut().set_local("config", dummy_cfg);
  env.borrow_mut().set_local("base", dummy_base);
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
    eval_rule_prelude(&mut interpreter, &env)?;

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
  eval_rule_prelude(&mut interpreter, &env)?;

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

  // Rust-owned helper prelude (scalar_num / map_num) so rules share one implementation.
  eval_rule_prelude(&mut interpreter, &env)?;

  let had_explicit_cfg_base = config.get("_base").map(|v| v.is_object()).unwrap_or(false);
  let base_json = if let Some(ctx_base) = context.get("_base_config").filter(|v| v.is_object()) {
    ctx_base.clone()
  } else if let Some(cfg_base) = config.get("_base").filter(|v| v.is_object()) {
    cfg_base.clone()
  } else if let Some(cs_base) = config
    .get("_coupled_state")
    .and_then(|cs| cs.get("base"))
    .filter(|v| v.is_object())
  {
    cs_base.clone()
  } else if let Some(obj) = config.as_object() {
    let mut b_map = serde_json::Map::new();
    for (k, v) in obj {
      if k != "_bounds" && k != "_coupled_state" && k != "_base" {
        b_map.insert(k.clone(), v.clone());
      }
    }
    serde_json::Value::Object(b_map)
  } else {
    serde_json::json!({})
  };
  let lua_base = LuaVal::from_json(&base_json);
  if let LuaVal::Table(ref base_tbl) = lua_base {
    interpreter.base_table = Some(base_tbl.clone());
  }

  let lua_config = LuaVal::from_json(config);
  let lua_context = LuaVal::from_json(context);
  if let LuaVal::Table(ref cfg_tbl) = lua_config {
    cfg_tbl.borrow_mut().remove("_base");
    interpreter.config_table = Some(cfg_tbl.clone());
  }

  // Link `bounds` global to `config["_bounds"]` (normalizing unbounded nil endpoints back to ±math.huge).
  if let LuaVal::Table(cfg_tbl) = &lua_config {
    let existing_bounds = cfg_tbl.borrow().get("_bounds").cloned();
    if let Some(LuaVal::Table(b_tbl)) = existing_bounds {
      for v in b_tbl.borrow().values() {
        if let LuaVal::Table(eb) = v {
          let mut eb_mut = eb.borrow_mut();
          if !eb_mut.contains_key("min") || matches!(eb_mut.get("min"), Some(LuaVal::Nil)) {
            eb_mut.insert("min".to_string(), LuaVal::Number(f64::NEG_INFINITY));
          }
          if !eb_mut.contains_key("max") || matches!(eb_mut.get("max"), Some(LuaVal::Nil)) {
            eb_mut.insert("max".to_string(), LuaVal::Number(f64::INFINITY));
          }
        }
      }
      env
        .borrow_mut()
        .set_local("bounds", LuaVal::Table(b_tbl.clone()));
    }
  }

  let mut params_map = HashMap::new();
  for (k, v) in &validated_params {
    params_map.insert(k.clone(), LuaVal::String(v.clone()));
  }
  let lua_params = LuaVal::Table(Rc::new(RefCell::new(params_map)));
  if let LuaVal::Table(ctx_tbl) = &lua_context {
    ctx_tbl.borrow_mut().remove("_base_config");
    ctx_tbl
      .borrow_mut()
      .insert("params".to_string(), lua_params.clone());
  }

  env.borrow_mut().set_local("config", lua_config.clone());
  env.borrow_mut().set_local("base", lua_base.clone());
  env.borrow_mut().set_local("ctx", lua_context.clone());
  env.borrow_mut().set_local("context", lua_context.clone());
  env.borrow_mut().set_local("params", lua_params.clone());

  // Execute the script
  interpreter.eval_chunk(&stmts, env.clone())?;

  // Re-bind `params` and `base` in case top-level declarations overwrote them
  env.borrow_mut().set_local("params", lua_params.clone());
  env.borrow_mut().set_local("base", lua_base.clone());

  // If an `apply` function was defined, invoke it: apply(config, ctx, params, base)
  let apply_fn = env.borrow().get("apply");
  if let LuaVal::Function(_, _, _) = apply_fn {
    let _ = interpreter.call_function(
      &apply_fn,
      vec![lua_config.clone(), lua_context, lua_params, lua_base],
      env.clone(),
    )?;
  }

  if let (LuaVal::Table(cfg_tbl), LuaVal::Table(b_tbl)) = (&lua_config, env.borrow().get("bounds"))
  {
    if !b_tbl.borrow().is_empty() {
      cfg_tbl
        .borrow_mut()
        .insert("_bounds".to_string(), LuaVal::Table(b_tbl));
    }
  }

  let mut resulting_config = lua_config.to_json();
  if let Some(obj) = resulting_config.as_object_mut() {
    if had_explicit_cfg_base {
      obj.insert("_base".to_string(), base_json);
    }
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
    for key in ["retract_lift_enforce", "filament_retract_lift_enforce"] {
      if let Some(val) = obj.get_mut(key) {
        let normalize = |s: &str| -> String {
          match s.trim() {
            "Top Surfaces Only" | "top_only" | "top" => "Top Only".to_string(),
            "Bottom Surfaces Only" | "bottom_only" | "bottom" => "Bottom Only".to_string(),
            "Top and Bottom Surfaces" | "top_and_bottom" => "Top and Bottom".to_string(),
            "All" | "all" | "all_surfaces" => "All Surfaces".to_string(),
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
    if let Some(serde_json::Value::String(s)) = obj.get_mut("wall_sequence") {
      *s = match s.trim() {
        "inner-outer-inner" => "inner-outer-inner wall".to_string(),
        "outer-inner" => "outer wall/inner wall".to_string(),
        "inner-outer" => "inner wall/outer wall".to_string(),
        other => other.to_string(),
      };
    }
    if let Some(serde_json::Value::String(s)) = obj.get_mut("ironing_type") {
      *s = match s.trim() {
        "all solid" | "all_solid" | "All solid layers" => "solid".to_string(),
        "Top surfaces" => "top".to_string(),
        "Topmost surface" => "topmost".to_string(),
        "No ironing" | "none" => "no ironing".to_string(),
        other => other.to_string(),
      };
    }
    if let Some(serde_json::Value::String(s)) = obj.get_mut("brim_type") {
      *s = match s.trim() {
        "ears" | "mouse_ears" => "brim_ears".to_string(),
        other => other.to_string(),
      };
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
  fn test_rust_native_pwl_clamp_sqrt_and_power_operator() {
    let cfg = serde_json::json!({});
    let ctx = serde_json::json!({});

    // pwl: interpolation, flat extrapolation on both ends, unordered input points.
    // Also covers clamp, math.sqrt, and the `^` operator (right-assoc, binds tighter
    // than unary minus so -2^2 == -4, and accepts a unary right side: 2^-2 == 0.25).
    let script = r#"
      function apply(config, ctx)
        local curve = { {265, -7.5}, {190, -8.0}, {240, -10.0}, {215, -11.0}, {300, -5.0} }
        print(string.format("%.2f", pwl(150, curve)))   -- below first x -> flat -8.0
        print(string.format("%.2f", pwl(235, curve)))   -- 215..240 segment -> -10.20
        print(string.format("%.2f", pwl(290, curve)))   -- 265..300 segment -> -5.71
        print(string.format("%.2f", pwl(400, curve)))   -- beyond last x -> flat -5.0
        print(string.format("%.2f", pwl(215, curve)))   -- exactly on a knot -> -11.0
        print(string.format("%.3f", clamp(0.96 * 0.99, 0.85, 1.1)))
        print(string.format("%.3f", clamp(5.0, 0.0, 1.0)))
        print(string.format("%.4f", math.sqrt(0.4 / 0.25)))
        print(string.format("%.4f", (0.5 / 0.4) ^ 1.15))
        print(string.format("%.2f", -2 ^ 2))
        print(string.format("%.2f", 2 ^ -2))
      end
    "#;
    let (_, stdout, _) = execute_transformation_lua(script, &cfg, &ctx).unwrap();
    let expected = [
      "-8.00", "-10.20", "-5.71", "-5.00", "-11.00", "0.950", "1.000", "1.2649", "1.2925", "-4.00",
      "0.25",
    ];
    assert_eq!(stdout, expected.to_vec());
  }

  #[test]
  fn test_rust_prelude_scalar_num_and_map_num() {
    let cfg = serde_json::json!({
      "nozzle_temperature": ["235"],
      "filament_flow_ratio": ["0.96", "0.90"],
      "filament_retraction_length": "0.8"
    });
    let ctx = serde_json::json!({});
    let script = r#"
      function apply(config, ctx)
        -- scalar_num unwraps single-slot arrays and bare strings, falls back to default.
        print(string.format("%.1f", scalar_num(config["nozzle_temperature"], 220)))
        print(string.format("%.2f", scalar_num(config["filament_retraction_length"], 1.5)))
        print(string.format("%.1f", scalar_num(config["missing_key"], 42)))
        -- map_num maps every numeric slot, preserves array shape & non-numeric slots.
        local mapped = map_num(config["filament_flow_ratio"], function(f)
          return f * 0.99
        end, "%.3f")
        config["filament_flow_ratio"] = mapped
        config["nozzle_temperature"] = map_num(config["nozzle_temperature"], function(t)
          return t - 10
        end, "%.0f")
        -- Scalar-shaped keys map too (shape preserved: scalar in, scalar out).
        config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r)
          return r * 0.8
        end, "%.2f")
      end
    "#;
    let (result, stdout, _) = execute_transformation_lua(script, &cfg, &ctx).unwrap();
    assert_eq!(stdout, vec!["235.0", "0.80", "42.0"]);
    assert_eq!(result["filament_flow_ratio"][0], "0.950");
    assert_eq!(result["filament_flow_ratio"][1], "0.891");
    assert_eq!(result["nozzle_temperature"][0], "225");
    assert_eq!(result["filament_retraction_length"], "0.64");
  }

  #[test]
  fn test_all_embedded_lua_transformations() {
    let thermal_lua = include_str!("../transformations/thermal_conductivity.lua");
    let addons_lua = include_str!("../transformations/rig_addons_cooling.lua");
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
        "filament_flow_ratio": ["0.96"],
        "filament_retraction_length": ["0.80"],
        "filament_retraction_speed": ["40"],
        "filament_z_hop": ["0.15"],
        "filament_max_volumetric_speed": ["15.0"],
        "layer_height": "0.20",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
        "inner_wall_speed": "300",
        "sparse_infill_speed": "350",
        "flush_multiplier": "1.0"
    });

    // 8-slot Double ACE Pro 2 context (FlowTech hotend installed — always true for MicroSwiss nozzles)
    let ctx = serde_json::json!({
        "nozzle_material": "Diamondback PCD",
        "nozzle_type": "diamondback",
        "nozzle_diameter": "0.4",
        "hotend_max_flow": 22.0,
        "filament_max_flow": 15.0,
        "melt_zone_multiplier": 1.25,
        "addons": ["flowtech_hotend"],
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

    // P10 thermal: 240 sits exactly on the PWL knot (240, -10) -> 230; init layer 245 ->
    // dT -9.5 -> 235.5 -> "%.0f" rounds half-to-even -> 236; flow 0.96 -> 0.954
    // (u = 0.733, trim = 0.65%); retraction untouched here (owned by P12/P25).
    let (c1, _, _) = execute_transformation_lua(thermal_lua, &config, &ctx).unwrap();
    assert_eq!(c1["nozzle_temperature"][0], "230");
    assert_eq!(c1["nozzle_temperature_initial_layer"][0], "236");
    assert_eq!(c1["filament_flow_ratio"][0], "0.954");

    // P12 FlowTech: 0.80 -> pwl(0.80)=0.70 (d_scale=1.0 at 0.4mm); 40 -> 44 mm/s.
    let (c1b, _, _) = execute_transformation_lua(addons_lua, &c1, &ctx).unwrap();
    assert_eq!(c1b["filament_retraction_length"][0], "0.70");
    assert_eq!(c1b["filament_retraction_speed"][0], "44");

    let (c2, _, _) = execute_transformation_lua(speed_lua, &c1b, &ctx).unwrap();
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

    // P25 stringing: 230 -> 228 (gated -1.6); retract 0.70 -> 0.75 (+0.051 recovery);
    // speed 44 -> 38 (quadratic damp); z_hop 0.15 -> 0.22 Spiral; travel accel 10000 -> 10700.
    let (c4, _, _) = execute_transformation_lua(stringing_lua, &c3, &ctx).unwrap();
    assert_eq!(c4["nozzle_temperature"][0], "228");
    assert_eq!(c4["filament_retraction_length"][0], "0.75");
    assert_eq!(c4["filament_retraction_speed"][0], "38");
    assert_eq!(c4["filament_z_hop"][0], "0.22");
    assert_eq!(c4["filament_z_hop_types"][0], "Spiral Lift");
    assert_eq!(c4["travel_acceleration"], "10700");
    assert_eq!(c4["wipe_on_loops"], "1");

    let (c5, _, _) = execute_transformation_lua(clog_lua, &c4, &ctx).unwrap();
    assert_eq!(c5["flush_multiplier"], "1.45");

    let (c6, _, _) = execute_transformation_lua(adhesion_lua, &c5, &ctx).unwrap();
    assert_eq!(c6["initial_layer_line_width"], "0.55");
    assert_eq!(c6["hot_plate_temp_initial_layer"][0], "85");

    let (c7, _, _) = execute_transformation_lua(spaghetti_lua, &c6, &ctx).unwrap();
    assert_eq!(c7["sparse_infill_pattern"], "gyroid");
    assert_eq!(c7["reduce_crossing_wall"], "1");
  }

  /// End-to-end verification matrix for the Diamondback PWL compensation chain
  /// (P10 thermal_conductivity -> P12 rig_addons_cooling/flowtech -> P25
  /// symptom_stringing_diamondback), mirroring §5 of
  /// `doc/pwl-nozzle-compensation-math.md`. Every expected value is a literal so the
  /// engineering report and the engine can never silently drift apart.
  #[test]
  fn test_diamondback_pwl_compensation_chain_matrix() {
    let thermal_lua = include_str!("../transformations/thermal_conductivity.lua");
    let addons_lua = include_str!("../transformations/rig_addons_cooling.lua");
    let stringing_lua = include_str!("../transformations/symptom_stringing_diamondback.lua");

    // (label, nozzle_diameter, base config keys): T0, r0, s0, f0, z0
    struct Case {
      label: &'static str,
      diam: &'static str,
      t0: f64,
      init0: f64,
      r0: f64,
      s0: f64,
      f0: f64,
      z0: f64,
      elastomer: bool,
    }
    let cases = [
      Case {
        label: "PETG 0.4 (audit config_007)",
        diam: "0.4",
        t0: 235.0,
        init0: 235.0,
        r0: 0.80,
        s0: 40.0,
        f0: 0.960,
        z0: 0.15,
        elastomer: false,
      },
      Case {
        label: "PLA 0.4",
        diam: "0.4",
        t0: 215.0,
        init0: 220.0,
        r0: 0.60,
        s0: 35.0,
        f0: 0.980,
        z0: 0.10,
        elastomer: false,
      },
      Case {
        label: "ASA 0.4 (structural)",
        diam: "0.4",
        t0: 265.0,
        init0: 265.0,
        r0: 0.80,
        s0: 40.0,
        f0: 0.930,
        z0: 0.10,
        elastomer: false,
      },
      Case {
        label: "TPU 95A 0.4 (elastomer)",
        diam: "0.4",
        t0: 225.0,
        init0: 230.0,
        r0: 1.30,
        s0: 25.0,
        f0: 1.000,
        z0: 0.20,
        elastomer: true,
      },
      Case {
        label: "PA-CF 0.6 (high-temp)",
        diam: "0.6",
        t0: 290.0,
        init0: 290.0,
        r0: 0.90,
        s0: 35.0,
        f0: 0.900,
        z0: 0.15,
        elastomer: false,
      },
      Case {
        label: "PLA 0.25 (fine detail)",
        diam: "0.25",
        t0: 210.0,
        init0: 215.0,
        r0: 0.60,
        s0: 35.0,
        f0: 0.980,
        z0: 0.12,
        elastomer: false,
      },
      Case {
        label: "PETG 1.0 (draft)",
        diam: "1.0",
        t0: 240.0,
        init0: 240.0,
        r0: 0.80,
        s0: 40.0,
        f0: 0.960,
        z0: 0.20,
        elastomer: false,
      },
      Case {
        label: "TPU 95A 0.4 (app defaults: 0.60 @ 35)",
        diam: "0.4",
        t0: 225.0,
        init0: 230.0,
        r0: 0.60,
        s0: 35.0,
        f0: 0.980,
        z0: 0.20,
        elastomer: true,
      },
    ];

    for case in &cases {
      let config = serde_json::json!({
        "nozzle_temperature": [format!("{}", case.t0)],
        "nozzle_temperature_initial_layer": [format!("{}", case.init0)],
        "filament_flow_ratio": [format!("{:.3}", case.f0)],
        "filament_retraction_length": [format!("{:.2}", case.r0)],
        "filament_retraction_speed": [format!("{}", case.s0)],
        "filament_z_hop": [format!("{:.2}", case.z0)],
        "travel_acceleration": "10000",
      });
      let ctx = serde_json::json!({
        "nozzle_material": "Diamondback PCD",
        "nozzle_type": "diamondback",
        "nozzle_diameter": case.diam,
        "addons": ["flowtech_hotend"],
        "filament_is_elastomer": case.elastomer,
      });

      let get1 = |v: &serde_json::Value, k: &str| -> String {
        v[k][0].as_str().unwrap_or("<missing>").to_string()
      };

      let (p10, _, _) = execute_transformation_lua(thermal_lua, &config, &ctx)
        .unwrap_or_else(|e| panic!("{}: P10 failed: {}", case.label, e));
      let (p12, _, _) = execute_transformation_lua(addons_lua, &p10, &ctx)
        .unwrap_or_else(|e| panic!("{}: P12 failed: {}", case.label, e));
      let (p25, _, _) = execute_transformation_lua(stringing_lua, &p12, &ctx)
        .unwrap_or_else(|e| panic!("{}: P25 failed: {}", case.label, e));

      println!(
        "[{}] T {}->{}->{}->{} | r {}->{}->{}->{} | s {}->{}->{}->{} | f {}->{} | z {}->{}",
        case.label,
        case.t0,
        get1(&p10, "nozzle_temperature"),
        get1(&p12, "nozzle_temperature"),
        get1(&p25, "nozzle_temperature"),
        format!("{:.2}", case.r0),
        get1(&p10, "filament_retraction_length"),
        get1(&p12, "filament_retraction_length"),
        get1(&p25, "filament_retraction_length"),
        case.s0,
        get1(&p10, "filament_retraction_speed"),
        get1(&p12, "filament_retraction_speed"),
        get1(&p25, "filament_retraction_speed"),
        format!("{:.3}", case.f0),
        get1(&p10, "filament_flow_ratio"),
        format!("{:.2}", case.z0),
        get1(&p25, "filament_z_hop"),
      );

      // Global invariants that must hold for EVERY case (the specific per-case numbers are
      // printed above and tabulated in the doc; these asserts are the physics guards).
      let t_final: f64 = get1(&p25, "nozzle_temperature").parse().unwrap();
      let t_base: f64 = case.t0;
      // Elastomers get NO P25 trim at all: final temp == P10's curve output exactly.
      if case.elastomer {
        let t_p10: f64 = get1(&p10, "nozzle_temperature").parse().unwrap();
        assert!(
          (t_final - t_p10).abs() < 1e-9,
          "{}: elastomer temp trimmed by P25 ({} -> {})",
          case.label,
          t_p10,
          t_final
        );
      }
      assert!(
        t_base - t_final >= 4.0 && t_base - t_final <= 13.0,
        "{}: total temp drop {} out of [-13, -4] band",
        case.label,
        t_base - t_final
      );
      assert!(
        t_final >= 175.0 && t_final <= 300.0,
        "{}: temp out of bounds",
        case.label
      );

      let r_final: f64 = get1(&p25, "filament_retraction_length").parse().unwrap();
      let s_final: f64 = get1(&p25, "filament_retraction_speed").parse().unwrap();
      let f_final: f64 = get1(&p10, "filament_flow_ratio").parse().unwrap();
      let z_final: f64 = get1(&p25, "filament_z_hop").parse().unwrap();
      let d: f64 = case.diam.parse().unwrap();

      // Rigid filaments never end ABOVE their brass base retraction; elastomers never lose
      // more than 15% of their spring travel (the designed P12 elastomer-PWL compression is
      // ~12% at 1.30 mm; short app-default TPU retraction round-trips back to its base).
      if !case.elastomer {
        assert!(
          r_final <= case.r0 + 1e-9,
          "{}: rigid retraction {} exceeded base {}",
          case.label,
          r_final,
          case.r0
        );
      } else {
        assert!(
          r_final >= case.r0 * 0.85 - 1e-9,
          "{}: elastomer retraction {} crushed below 85% of base {}",
          case.label,
          r_final,
          case.r0
        );
        // Elastomers keep their calibrated speed through BOTH the P12 skip and the P25
        // damper (even above the 25 mm/s soft zone — TPU for AMS ships 35 and must stay 35).
        assert!(
          (s_final - case.s0).abs() < 1e-9,
          "{}: elastomer retraction speed changed {} -> {}",
          case.label,
          case.s0,
          s_final
        );
      }
      // PCD temp drop must taper (never more than 12 C below base; high-temp never below -9).
      if case.t0 >= 265.0 {
        assert!(
          t_base - t_final <= 8.0,
          "{}: high-temp polymer over-cooled by {}",
          case.label,
          t_base - t_final
        );
      }
      // Flow trim is a pure reduction bounded by 1.2% and floored at 0.85.
      assert!(
        f_final <= case.f0 + 1e-9,
        "{}: flow trim must never increase flow",
        case.label
      );
      assert!(f_final >= 0.85, "{}: flow floor", case.label);
      // Z-hop lift respects the small-orifice ceiling and never reduces the spool base z0.
      assert!(
        z_final >= case.z0 - 1e-9 && z_final <= (case.z0 + 0.04).max(d * 0.65) + 1e-9,
        "{}: z-hop {} out of [{}, {}]",
        case.label,
        z_final,
        case.z0,
        (case.z0 + 0.04).max(d * 0.65)
      );
      // Travel accel boost always clears the <8000 -> 10000 export clamp.
      let a_final: f64 = p25["travel_acceleration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
      assert!(
        a_final >= 9000.0,
        "{}: travel accel {} below 9000 floor",
        case.label,
        a_final
      );
    }

    // Spot-check the headline per-case numbers from the engineering report (PETG 0.4).
    let config = serde_json::json!({
      "nozzle_temperature": ["235"],
      "nozzle_temperature_initial_layer": ["235"],
      "filament_flow_ratio": ["0.960"],
      "filament_retraction_length": ["0.80"],
      "filament_retraction_speed": ["40"],
      "filament_z_hop": ["0.15"],
      "travel_acceleration": "10000",
    });
    let ctx = serde_json::json!({
      "nozzle_material": "Diamondback PCD",
      "nozzle_type": "diamondback",
      "nozzle_diameter": "0.4",
      "addons": ["flowtech_hotend"],
    });
    let (p10, _, _) = execute_transformation_lua(thermal_lua, &config, &ctx).unwrap();
    let (p12, _, _) = execute_transformation_lua(addons_lua, &p10, &ctx).unwrap();
    let (p25, _, _) = execute_transformation_lua(stringing_lua, &p12, &ctx).unwrap();
    assert_eq!(p10["nozzle_temperature"][0], "225"); // -10.2 -> 225
    assert_eq!(p10["filament_flow_ratio"][0], "0.954");
    assert_eq!(p12["filament_retraction_length"][0], "0.70");
    assert_eq!(p12["filament_retraction_speed"][0], "44");
    assert_eq!(p25["nozzle_temperature"][0], "223"); // gated -2.0
    assert_eq!(p25["filament_retraction_length"][0], "0.75");
    assert_eq!(p25["filament_retraction_speed"][0], "38");
    assert_eq!(p25["filament_z_hop"][0], "0.22");
    assert_eq!(p25["travel_acceleration"], "10700");
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
