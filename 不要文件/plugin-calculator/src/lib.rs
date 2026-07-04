// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Calculator plugin: evaluates mathematical expressions.

use easysearch_core::{Action, Plugin, PluginResult};

pub struct CalculatorPlugin;

impl Plugin for CalculatorPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // auto-detect math expressions
    }

    fn matches(&self, query: &str) -> bool {
        // Simple heuristic: starts with digit or open-paren, contains operator or is a function call
        let q = query.trim();
        if q.is_empty() {
            return false;
        }
        let first = q.chars().next().unwrap();
        let starts_ok = first.is_ascii_digit() || first == '(' || first == '-' || first == '.';
        let has_op = q.chars().any(|c| "+-*/^%".contains(c));
        let has_fn = FUNCTIONS.iter().any(|f| q.to_lowercase().contains(f.0));
        (starts_ok && has_op) || has_fn
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        match evaluate(query) {
            Some(result) => {
                let display = format_number(result);
                vec![PluginResult {
                    title: display.clone(),
                    subtitle: format!("{query} ="),
                    icon: String::from("calculator"),
                    action: Action::Copy(display),
                    score: 1000,
                }]
            }
            None => Vec::new(),
        }
    }

    fn name(&self) -> &str {
        "Calculator"
    }
}

/// Simple recursive descent expression evaluator.
/// Supports: + - * / ^ % ( ) and functions (sqrt, sin, cos, tan, abs, log, ln, floor, ceil, round)
fn evaluate(expr: &str) -> Option<f64> {
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos == tokens.len() {
        if result.is_nan() || result.is_infinite() {
            return None;
        }
        Some(result)
    } else {
        None
    }
}

/// Supported math functions.
const FUNCTIONS: &[(&str, fn(f64) -> f64)] = &[
    ("sqrt", |x| x.sqrt()),
    ("abs", |x| x.abs()),
    ("sin", |x| x.sin()),
    ("cos", |x| x.cos()),
    ("tan", |x| x.tan()),
    ("asin", |x| x.asin()),
    ("acos", |x| x.acos()),
    ("atan", |x| x.atan()),
    ("log", |x| x.log10()),
    ("log2", |x| x.log2()),
    ("ln", |x| x.ln()),
    ("exp", |x| x.exp()),
    ("floor", |x| x.floor()),
    ("ceil", |x| x.ceil()),
    ("round", |x| x.round()),
];

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Op(char),
    LParen,
    RParen,
    Func(fn(f64) -> f64),
}

fn tokenize(s: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            let num = num_str.parse::<f64>().ok()?;
            tokens.push(Token::Num(num));
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            let ident_lower = ident.to_lowercase();

            match ident_lower.as_str() {
                "pi" => tokens.push(Token::Num(std::f64::consts::PI)),
                "e" => tokens.push(Token::Num(std::f64::consts::E)),
                _ => {
                    if let Some((_name, func)) =
                        FUNCTIONS.iter().find(|(name, _)| *name == ident_lower.as_str())
                    {
                        tokens.push(Token::Func(*func));
                    } else {
                        return None;
                    }
                }
            }
        } else if "+-*/%^".contains(c) {
            if c == '-'
                && (tokens.is_empty()
                    || matches!(tokens.last(), Some(Token::Op(_) | Token::LParen)))
            {
                i += 1;
                if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    let start = i;
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        i += 1;
                    }
                    let num_str: String = chars[start..i].iter().collect();
                    let num = num_str.parse::<f64>().ok()?;
                    tokens.push(Token::Num(-num));
                } else {
                    tokens.push(Token::Num(0.0));
                    tokens.push(Token::Op('-'));
                }
            } else {
                tokens.push(Token::Op(c));
                i += 1;
            }
        } else if c == '(' {
            tokens.push(Token::LParen);
            i += 1;
        } else if c == ')' {
            tokens.push(Token::RParen);
            i += 1;
        } else if c == ',' {
            i += 1;
        } else {
            return None;
        }
    }

    Some(tokens)
}

fn parse_expr(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('+') => {
                *pos += 1;
                left += parse_term(tokens, pos)?;
            }
            Token::Op('-') => {
                *pos += 1;
                left -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }
    Some(left)
}

fn parse_term(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('*') => {
                *pos += 1;
                left *= parse_power(tokens, pos)?;
            }
            Token::Op('/') => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                if right == 0.0 {
                    return None;
                }
                left /= right;
            }
            Token::Op('%') => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                if right == 0.0 {
                    return None;
                }
                left %= right;
            }
            _ => break,
        }
    }
    Some(left)
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    let base = parse_atom(tokens, pos)?;
    if *pos < tokens.len() {
        if let Token::Op('^') = &tokens[*pos] {
            *pos += 1;
            let exp = parse_power(tokens, pos)?;
            return Some(base.powf(exp));
        }
    }
    Some(base)
}

fn parse_atom(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    if *pos >= tokens.len() {
        return None;
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let val = *n;
            *pos += 1;
            Some(val)
        }
        Token::Func(f) => {
            let func = *f;
            *pos += 1;
            if *pos < tokens.len() && matches!(&tokens[*pos], Token::LParen) {
                *pos += 1;
                let val = parse_expr(tokens, pos)?;
                if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) {
                    *pos += 1;
                    Some(func(val))
                } else {
                    None
                }
            } else {
                None
            }
        }
        Token::LParen => {
            *pos += 1;
            let val = parse_expr(tokens, pos)?;
            if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) {
                *pos += 1;
                Some(val)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn format_number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{:.10}", n);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}
