//! A small JSON reader and writer, so the crate needs no dependency. Objects keep their key
//! order, which the 26.3 recipes written from 1.20 ones keep too.

use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }
    /// An integer: a number with no fraction that fits an `i64`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Some(*n as i64),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn str(s: &str) -> Value {
        Value::String(s.to_owned())
    }
    pub fn obj(entries: Vec<(&str, Value)>) -> Value {
        Value::Object(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    }

    /// Compact JSON text.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        write_value(self, &mut out);
        out
    }
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 9.0e15 {
                let _ = write!(out, "{}", *n as i64);
            } else if n.is_finite() {
                let _ = write!(out, "{n}");
            } else {
                out.push_str("null");
            }
        }
        Value::String(s) => write_string(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out);
            }
            out.push(']');
        }
        Value::Object(o) => {
            out.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(x, out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Reads one JSON document (surrounding whitespace allowed).
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
        depth: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("trailing characters at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.i))
    }
    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }
    fn value(&mut self) -> Result<Value, String> {
        self.ws();
        let Some(&c) = self.s.get(self.i) else {
            return self.err("unexpected end");
        };
        match c {
            b'{' | b'[' => {
                self.depth += 1;
                if self.depth > 128 {
                    return self.err("nested too deep");
                }
                let v = if c == b'{' {
                    self.object()
                } else {
                    self.array()
                };
                self.depth -= 1;
                v
            }
            b'"' => self.string().map(Value::String),
            b't' if self.eat("true") => Ok(Value::Bool(true)),
            b'f' if self.eat("false") => Ok(Value::Bool(false)),
            b'n' if self.eat("null") => Ok(Value::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => self.err("unexpected character"),
        }
    }
    fn object(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut entries = Vec::new();
        self.ws();
        if self.eat("}") {
            return Ok(Value::Object(entries));
        }
        loop {
            self.ws();
            if self.s.get(self.i) != Some(&b'"') {
                return self.err("expected a key");
            }
            let key = self.string()?;
            self.ws();
            if !self.eat(":") {
                return self.err("expected ':'");
            }
            let v = self.value()?;
            entries.push((key, v));
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("}") {
                return Ok(Value::Object(entries));
            }
            return self.err("expected ',' or '}'");
        }
    }
    fn array(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("]") {
                return Ok(Value::Array(items));
            }
            return self.err("expected ',' or ']'");
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let Some(digits) = self.s.get(self.i..self.i + 4) else {
            return self.err("short \\u escape");
        };
        let text = std::str::from_utf8(digits).map_err(|e| e.to_string())?;
        let n = u32::from_str_radix(text, 16).map_err(|_| format!("bad \\u at {}", self.i))?;
        self.i += 4;
        Ok(n)
    }
    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.i < self.s.len() && !matches!(self.s[self.i], b'"' | b'\\') {
                if self.s[self.i] < 0x20 {
                    return self.err("control character in string");
                }
                self.i += 1;
            }
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?);
            match self.s.get(self.i) {
                None => return self.err("unterminated string"),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(_) => {
                    self.i += 1;
                    let Some(&e) = self.s.get(self.i) else {
                        return self.err("unterminated escape");
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat("\\u") {
                                    return self.err("lone surrogate");
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return self.err("bad low surrogate");
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => return self.err("bad escape"),
                    }
                }
            }
        }
    }
    fn number(&mut self) -> Result<Value, String> {
        let start = self.i;
        while self.i < self.s.len()
            && matches!(
                self.s[self.i],
                b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
            )
        {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?;
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| format!("bad number {text:?} at byte {start}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_back() {
        let text = r#"{"type":"botania:mana_infusion","mana":3000,"input":{"item":"minecraft:iron_ingot"},"list":[1,2.5,-3,true,null],"s":"a\"b\\c\u00e9\ud83d\ude00"}"#;
        let v = parse(text).unwrap();
        assert_eq!(v.get("mana").and_then(Value::as_i64), Some(3000));
        assert_eq!(v.get("s").and_then(Value::as_str), Some("a\"b\\cé😀"));
        let again = parse(&v.to_json()).unwrap();
        assert_eq!(again, v);
        assert_eq!(
            parse(r#" { "a" : [ ] , "b" : { } } "#).unwrap().to_json(),
            r#"{"a":[],"b":{}}"#
        );
    }

    #[test]
    fn refuses_broken_documents() {
        for bad in [
            "{",
            "[1,]",
            "{\"a\" 1}",
            "\"\\x\"",
            "tru",
            "{} x",
            "\"\\ud800\"",
        ] {
            assert!(parse(bad).is_err(), "{bad} should fail");
        }
    }
}
