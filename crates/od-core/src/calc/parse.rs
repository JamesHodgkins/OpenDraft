//! The expression language: numbers, names, `+ - * / ^`, brackets and
//! functions. Strict by design (no implicit multiplication, no units), so an
//! expression means one thing. Parsed to a tree and evaluated against the
//! names defined so far.

use super::CalcError;

/// Deepest nesting of brackets and operators accepted.
const DEPTH_MAX: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Expr {
    Number(f64),
    Name(String),
    Negate(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(char),
}

fn tokens(text: &str) -> Result<Vec<(usize, Token)>, CalcError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // An exponent: e or E, an optional sign, digits.
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let word: String = chars[start..i].iter().collect();
            let value = word
                .parse::<f64>()
                .map_err(|_| CalcError(format!("{word:?} is not a number (at {})", start + 1)))?;
            out.push((start, Token::Number(value)));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push((start, Token::Name(chars[start..i].iter().collect())));
        } else if "+-*/^(),".contains(c) {
            out.push((i, Token::Op(c)));
            i += 1;
        } else {
            return Err(CalcError(format!("unexpected {c:?} (at {})", i + 1)));
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<(usize, Token)>,
    at: usize,
    depth: usize,
    end: usize,
}

pub(super) fn parse(text: &str) -> Result<Expr, CalcError> {
    let tokens = tokens(text)?;
    if tokens.is_empty() {
        return Err(CalcError("the expression is empty".to_owned()));
    }
    let mut parser = Parser {
        tokens,
        at: 0,
        depth: 0,
        end: text.chars().count() + 1,
    };
    let expr = parser.expr()?;
    if let Some((pos, token)) = parser.tokens.get(parser.at) {
        return Err(CalcError(format!(
            "unexpected {} (at {})",
            describe(token),
            pos + 1
        )));
    }
    Ok(expr)
}

fn describe(token: &Token) -> String {
    match token {
        Token::Number(n) => n.to_string(),
        Token::Name(n) => format!("{n:?}"),
        Token::Op(c) => format!("{c:?}"),
    }
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(_, t)| t)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).map(|(_, t)| t.clone());
        self.at += 1;
        token
    }

    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Token::Op(op)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn position(&self) -> usize {
        self.tokens.get(self.at).map_or(self.end, |(p, _)| p + 1)
    }

    fn enter(&mut self) -> Result<(), CalcError> {
        self.depth += 1;
        if self.depth > DEPTH_MAX {
            return Err(CalcError("the expression is nested too deeply".to_owned()));
        }
        Ok(())
    }

    fn expr(&mut self) -> Result<Expr, CalcError> {
        self.enter()?;
        let mut left = self.term()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.at += 1;
            let right = self.term()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        self.depth -= 1;
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, CalcError> {
        let mut left = self.unary()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.peek().cloned() {
            self.at += 1;
            let right = self.unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `-x` binds looser than `^`, so `-2^2` is `-(2^2)`.
    fn unary(&mut self) -> Result<Expr, CalcError> {
        self.enter()?;
        let result = if self.eat('-') {
            Expr::Negate(Box::new(self.unary()?))
        } else if self.eat('+') {
            self.unary()?
        } else {
            self.power()?
        };
        self.depth -= 1;
        Ok(result)
    }

    /// `a^b^c` is `a^(b^c)`, and the exponent may be negative: `2^-1`.
    fn power(&mut self) -> Result<Expr, CalcError> {
        let base = self.atom()?;
        if self.eat('^') {
            let exponent = self.unary()?;
            return Ok(Expr::Binary('^', Box::new(base), Box::new(exponent)));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<Expr, CalcError> {
        let at = self.position();
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::Name(name)) => {
                if !self.eat('(') {
                    return Ok(Expr::Name(name));
                }
                let mut args = Vec::new();
                if !self.eat(')') {
                    loop {
                        args.push(self.expr()?);
                        if self.eat(')') {
                            break;
                        }
                        if !self.eat(',') {
                            return Err(CalcError(format!(
                                "expected ',' or ')' in the call of {name} (at {})",
                                self.position()
                            )));
                        }
                    }
                }
                Ok(Expr::Call(name, args))
            }
            Some(Token::Op('(')) => {
                let inner = self.expr()?;
                if !self.eat(')') {
                    return Err(CalcError(format!("expected ')' (at {})", self.position())));
                }
                Ok(inner)
            }
            Some(other) => Err(CalcError(format!(
                "unexpected {} (at {at})",
                describe(&other)
            ))),
            None => Err(CalcError(format!(
                "the expression ends too soon (at {})",
                self.end
            ))),
        }
    }
}
