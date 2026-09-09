//! Lightweight, zero-dependency predicate expression parser and evaluator for CSV database queries.
//!
//! Unit tests live in the sibling `predicate_tests.rs` sidecar.

use std::collections::HashMap;

const ERR_INVALID_PRED: &str = "Invalid predicate";
const ERR_EVAL_ERROR: &str = "Evaluation error";

/// Represents a value during predicate evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating-point number.
    Float(f64),
    /// UTF-8 string.
    String(String),
    /// Boolean flag.
    Bool(bool),
}

impl Value {
    /// Attempt to coerce this value to a boolean, returning an error if not boolean.
    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err(format!("{ERR_EVAL_ERROR}: expected boolean, got {self:?}")),
        }
    }
}

/// Binary operators supported in predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// Equality `==`
    Eq,
    /// Inequality `!=`
    Ne,
    /// Less than `<`
    Lt,
    /// Less than or equal `<=`
    Le,
    /// Greater than `>`
    Gt,
    /// Greater than or equal `>=`
    Ge,
    /// Logical AND `&&` or `and`
    And,
    /// Logical OR `||` or `or`
    Or,
    /// Addition `+`
    Add,
    /// Subtraction `-`
    Sub,
    /// Multiplication `*`
    Mul,
    /// Division `/`
    Div,
}

/// Unary operators supported in predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Logical negation `!` or `not`
    Not,
    /// Arithmetic negation `-`
    Neg,
}

/// Abstract Syntax Tree node for predicate expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Literal constant value.
    Literal(Value),
    /// Column or variable identifier.
    Identifier(String),
    /// Unary operator applied to an expression.
    Unary(UnaryOp, Box<Expr>),
    /// Binary operator applied to two expressions.
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}

/// Parsed predicate expression ready for evaluation against row contexts.
#[derive(Debug, Clone, PartialEq)]
pub struct Predicate {
    root: Expr,
}

impl Predicate {
    /// Parse a predicate expression string into a compiled `Predicate`.
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(format!("{ERR_INVALID_PRED}: empty expression"));
        }
        let tokens = tokenize(trimmed)?;
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expr()?;
        if parser.has_more() {
            return Err(format!(
                "{ERR_INVALID_PRED}: unexpected trailing tokens at position {}",
                parser.pos
            ));
        }
        Ok(Predicate { root: expr })
    }

    /// Evaluate the predicate against a variable context, returning a boolean.
    pub fn eval_boolean(&self, context: &HashMap<String, Value>) -> Result<bool, String> {
        let val = eval_expr(&self.root, context)?;
        val.as_bool()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < len {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '=' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Eq);
                    i += 2;
                } else {
                    return Err(format!(
                        "{ERR_INVALID_PRED}: unexpected '=' at position {i}, expected '=='"
                    ));
                }
            }
            '!' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Ne);
                    i += 2;
                } else {
                    tokens.push(Token::Not);
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Le);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '&' => {
                if i + 1 < len && chars[i + 1] == '&' {
                    tokens.push(Token::And);
                    i += 2;
                } else {
                    return Err(format!(
                        "{ERR_INVALID_PRED}: unexpected single '&' at position {i}, expected '&&'"
                    ));
                }
            }
            '|' => {
                if i + 1 < len && chars[i + 1] == '|' {
                    tokens.push(Token::Or);
                    i += 2;
                } else {
                    return Err(format!(
                        "{ERR_INVALID_PRED}: unexpected single '|' at position {i}, expected '||'"
                    ));
                }
            }
            '"' | '\'' => {
                let quote = c;
                let mut s = String::new();
                i += 1;
                let mut closed = false;
                while i < len {
                    let cur = chars[i];
                    if cur == '\\' && i + 1 < len {
                        s.push(chars[i + 1]);
                        i += 2;
                    } else if cur == quote {
                        closed = true;
                        i += 1;
                        break;
                    } else {
                        s.push(cur);
                        i += 1;
                    }
                }
                if !closed {
                    return Err(format!("{ERR_INVALID_PRED}: unclosed string literal"));
                }
                tokens.push(Token::Str(s));
            }
            '0'..='9' => {
                let start = i;
                let mut is_float = false;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    if chars[i] == '.' {
                        if is_float {
                            break;
                        }
                        is_float = true;
                    }
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                if is_float {
                    let val = num_str
                        .parse::<f64>()
                        .map_err(|e| format!("{ERR_INVALID_PRED}: invalid float: {e}"))?;
                    tokens.push(Token::Float(val));
                } else {
                    let val = num_str
                        .parse::<i64>()
                        .map_err(|e| format!("{ERR_INVALID_PRED}: invalid int: {e}"))?;
                    tokens.push(Token::Int(val));
                }
            }
            _ if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                match ident.to_lowercase().as_str() {
                    "true" => tokens.push(Token::Bool(true)),
                    "false" => tokens.push(Token::Bool(false)),
                    "and" => tokens.push(Token::And),
                    "or" => tokens.push(Token::Or),
                    "not" => tokens.push(Token::Not),
                    _ => tokens.push(Token::Ident(ident)),
                }
            }
            _ => {
                return Err(format!(
                    "{ERR_INVALID_PRED}: unexpected character '{c}' at position {i}"
                ));
            }
        }
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next_token(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn has_more(&self) -> bool {
        self.pos < self.tokens.len()
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while let Some(Token::Or) = self.peek() {
            self.next_token();
            let right = self.parse_and()?;
            left = Expr::Binary(BinaryOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_comparison()?;
        while let Some(Token::And) = self.peek() {
            self.next_token();
            let right = self.parse_comparison()?;
            left = Expr::Binary(BinaryOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Some(Token::Eq) => BinaryOp::Eq,
                Some(Token::Ne) => BinaryOp::Ne,
                Some(Token::Lt) => BinaryOp::Lt,
                Some(Token::Le) => BinaryOp::Le,
                Some(Token::Gt) => BinaryOp::Gt,
                Some(Token::Ge) => BinaryOp::Ge,
                _ => break,
            };
            self.next_token();
            let right = self.parse_additive()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Sub,
                _ => break,
            };
            self.next_token();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinaryOp::Mul,
                Some(Token::Slash) => BinaryOp::Div,
                _ => break,
            };
            self.next_token();
            let right = self.parse_unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::Not) => {
                self.next_token();
                let inner = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(inner)))
            }
            Some(Token::Minus) => {
                self.next_token();
                let inner = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(inner)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.next_token() {
            Some(Token::Int(i)) => Ok(Expr::Literal(Value::Int(i))),
            Some(Token::Float(f)) => Ok(Expr::Literal(Value::Float(f))),
            Some(Token::Str(s)) => Ok(Expr::Literal(Value::String(s))),
            Some(Token::Bool(b)) => Ok(Expr::Literal(Value::Bool(b))),
            Some(Token::Ident(id)) => Ok(Expr::Identifier(id)),
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                if let Some(Token::RParen) = self.next_token() {
                    Ok(inner)
                } else {
                    Err(format!(
                        "{ERR_INVALID_PRED}: missing closing parenthesis ')'"
                    ))
                }
            }
            Some(tok) => Err(format!("{ERR_INVALID_PRED}: unexpected token {tok:?}")),
            None => Err(format!("{ERR_INVALID_PRED}: unexpected end of expression")),
        }
    }
}

fn eval_expr(expr: &Expr, context: &HashMap<String, Value>) -> Result<Value, String> {
    match expr {
        Expr::Literal(val) => Ok(val.clone()),
        Expr::Identifier(id) => context
            .get(id)
            .cloned()
            .ok_or_else(|| format!("{ERR_EVAL_ERROR}: variable '{id}' not found")),
        Expr::Unary(op, inner) => {
            let val = eval_expr(inner, context)?;
            match op {
                UnaryOp::Not => {
                    let b = val.as_bool()?;
                    Ok(Value::Bool(!b))
                }
                UnaryOp::Neg => match val {
                    Value::Int(i) => Ok(Value::Int(-i)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(format!(
                        "{ERR_EVAL_ERROR}: cannot apply unary minus to {val:?}"
                    )),
                },
            }
        }
        Expr::Binary(op, left_expr, right_expr) => match op {
            BinaryOp::And => {
                let left_val = eval_expr(left_expr, context)?;
                let left_b = left_val.as_bool()?;
                if !left_b {
                    return Ok(Value::Bool(false));
                }
                let right_val = eval_expr(right_expr, context)?;
                let right_b = right_val.as_bool()?;
                Ok(Value::Bool(right_b))
            }
            BinaryOp::Or => {
                let left_val = eval_expr(left_expr, context)?;
                let left_b = left_val.as_bool()?;
                if left_b {
                    return Ok(Value::Bool(true));
                }
                let right_val = eval_expr(right_expr, context)?;
                let right_b = right_val.as_bool()?;
                Ok(Value::Bool(right_b))
            }
            _ => {
                let left = eval_expr(left_expr, context)?;
                let right = eval_expr(right_expr, context)?;
                eval_binary_op(*op, left, right)
            }
        },
    }
}

fn eval_binary_op(op: BinaryOp, left: Value, right: Value) -> Result<Value, String> {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
        BinaryOp::Ne => Ok(Value::Bool(!values_equal(&left, &right))),
        BinaryOp::Lt => compare_values(&left, &right).map(|c| Value::Bool(c < 0)),
        BinaryOp::Le => compare_values(&left, &right).map(|c| Value::Bool(c <= 0)),
        BinaryOp::Gt => compare_values(&left, &right).map(|c| Value::Bool(c > 0)),
        BinaryOp::Ge => compare_values(&left, &right).map(|c| Value::Bool(c >= 0)),
        BinaryOp::Add => arithmetic_op(&left, &right, |a, b| a + b, |a, b| a + b),
        BinaryOp::Sub => arithmetic_op(&left, &right, |a, b| a - b, |a, b| a - b),
        BinaryOp::Mul => arithmetic_op(&left, &right, |a, b| a * b, |a, b| a * b),
        BinaryOp::Div => {
            if is_zero(&right) {
                return Err(format!("{ERR_EVAL_ERROR}: division by zero"));
            }
            arithmetic_op(&left, &right, |a, b| a / b, |a, b| a / b)
        }
        BinaryOp::And | BinaryOp::Or => unreachable!("Handled in short-circuit evaluation"),
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
        (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        _ => false,
    }
}

fn compare_values(left: &Value, right: &Value) -> Result<i8, String> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }),
        (Value::Float(a), Value::Float(b)) => Ok(match a.partial_cmp(b) {
            Some(std::cmp::Ordering::Less) => -1,
            Some(std::cmp::Ordering::Equal) => 0,
            Some(std::cmp::Ordering::Greater) => 1,
            None => -1,
        }),
        (Value::Int(a), Value::Float(b)) => {
            let af = *a as f64;
            Ok(match af.partial_cmp(b) {
                Some(std::cmp::Ordering::Less) => -1,
                Some(std::cmp::Ordering::Equal) => 0,
                Some(std::cmp::Ordering::Greater) => 1,
                None => -1,
            })
        }
        (Value::Float(a), Value::Int(b)) => {
            let bf = *b as f64;
            Ok(match a.partial_cmp(&bf) {
                Some(std::cmp::Ordering::Less) => -1,
                Some(std::cmp::Ordering::Equal) => 0,
                Some(std::cmp::Ordering::Greater) => 1,
                None => -1,
            })
        }
        (Value::String(a), Value::String(b)) => Ok(match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }),
        _ => Err(format!(
            "{ERR_EVAL_ERROR}: cannot compare incompatible types {left:?} and {right:?}"
        )),
    }
}

fn arithmetic_op<FI, FF>(
    left: &Value,
    right: &Value,
    int_op: FI,
    float_op: FF,
) -> Result<Value, String>
where
    FI: Fn(i64, i64) -> i64,
    FF: Fn(f64, f64) -> f64,
{
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(int_op(*a, *b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(*a, *b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(float_op(*a as f64, *b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(float_op(*a, *b as f64))),
        _ => Err(format!(
            "{ERR_EVAL_ERROR}: cannot perform arithmetic on {left:?} and {right:?}"
        )),
    }
}

fn is_zero(val: &Value) -> bool {
    match val {
        Value::Int(i) => *i == 0,
        Value::Float(f) => *f == 0.0,
        _ => false,
    }
}

#[cfg(test)]
#[path = "predicate_tests.rs"]
mod tests;
