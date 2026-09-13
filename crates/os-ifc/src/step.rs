//! Bounded STEP subset tokenizer. This is not a general ISO 10303 implementation.
use os_core::{Error, Id, Result, ensure};
use std::collections::BTreeMap;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
pub fn guid(id: Id) -> String {
    let mut n = id.0.as_u128();
    let mut out = [b'0'; 22];
    for c in out.iter_mut().rev() {
        *c = ALPHABET[(n & 63) as usize];
        n >>= 6;
    }
    String::from_utf8(out.to_vec()).expect("ASCII alphabet")
}
pub fn identity(s: &str) -> Result<Id> {
    ensure(
        s.len() == 22 && s.as_bytes()[0] <= b'3',
        "invalid IFC GlobalId",
    )?;
    let mut n = 0_u128;
    for c in s.bytes() {
        let i = ALPHABET
            .iter()
            .position(|a| *a == c)
            .ok_or_else(|| bad("invalid GlobalId alphabet"))?;
        n = (n << 6) | i as u128;
    }
    ensure(n != 0, "nil IFC GlobalId")?;
    Ok(Id(uuid::Uuid::from_u128(n)))
}
pub fn quote(s: &str) -> String {
    // Encode all text in the standard UTF-16 escape form; apostrophes/backslashes
    // cannot terminate STEP strings and non-ASCII names round trip correctly.
    if s.is_empty() {
        return "''".into();
    }
    let hex: String = s.encode_utf16().map(|c| format!("{c:04X}")).collect();
    format!("'\\X2\\{hex}\\X0\\'")
}
pub fn bad(s: impl Into<String>) -> Error {
    Error::Invalid(format!("IFC: {}", s.into()))
}
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Derived,
    Ref(u32),
    Number(f64),
    Text(String),
    Enum(String),
    List(Vec<Value>),
}
impl Value {
    pub fn list(&self) -> Result<&[Value]> {
        if let Self::List(v) = self {
            Ok(v)
        } else {
            Err(bad("expected list"))
        }
    }
    pub fn text(&self) -> Result<&str> {
        if let Self::Text(v) = self {
            Ok(v)
        } else {
            Err(bad("expected string"))
        }
    }
    pub fn number(&self) -> Result<f64> {
        if let Self::Number(v) = self {
            Ok(*v)
        } else {
            Err(bad("expected number"))
        }
    }
    pub fn reference(&self) -> Result<u32> {
        if let Self::Ref(v) = self {
            Ok(*v)
        } else {
            Err(bad("expected reference"))
        }
    }
}
#[derive(Debug)]
pub struct Entity {
    pub kind: String,
    pub args: Vec<Value>,
}
impl Entity {
    pub fn at(&self, i: usize) -> Result<&Value> {
        self.args
            .get(i)
            .ok_or_else(|| bad(format!("{} missing argument {i}", self.kind)))
    }
}
pub type Entities = BTreeMap<u32, Entity>;
struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn ws(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn token(&mut self, s: &[u8]) -> Result<()> {
        self.ws();
        ensure(
            self.bytes[self.at..].starts_with(s),
            format!(
                "IFC: expected {} at byte {}",
                String::from_utf8_lossy(s),
                self.at
            ),
        )?;
        self.at += s.len();
        Ok(())
    }
    fn word(&mut self) -> Result<String> {
        self.ws();
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            self.at += 1;
        }
        ensure(self.at > start, "IFC: expected identifier")?;
        Ok(String::from_utf8_lossy(&self.bytes[start..self.at]).into())
    }
    fn string(&mut self) -> Result<String> {
        self.token(b"'")?;
        let mut out = String::new();
        loop {
            let c = *self
                .bytes
                .get(self.at)
                .ok_or_else(|| bad("unterminated string"))?;
            self.at += 1;
            match c {
                b'\'' if self.bytes.get(self.at) == Some(&b'\'') => {
                    out.push('\'');
                    self.at += 1;
                }
                b'\'' => return Ok(out),
                b'\\' => {
                    ensure(
                        self.bytes[self.at..].starts_with(b"X2\\"),
                        "IFC: unsupported string escape",
                    )?;
                    self.at += 3;
                    let start = self.at;
                    while !self.bytes[self.at..].starts_with(b"\\X0\\") {
                        ensure(
                            self.bytes.get(self.at).is_some_and(u8::is_ascii_hexdigit),
                            "IFC: malformed UTF-16 escape",
                        )?;
                        self.at += 1;
                    }
                    let hex = &self.bytes[start..self.at];
                    ensure(hex.len().is_multiple_of(4), "IFC: malformed UTF-16 length")?;
                    let units: Result<Vec<u16>> = hex
                        .chunks(4)
                        .map(|c| {
                            u16::from_str_radix(std::str::from_utf8(c).unwrap_or(""), 16)
                                .map_err(|_| bad("invalid UTF-16"))
                        })
                        .collect();
                    out.push_str(
                        &String::from_utf16(&units?)
                            .map_err(|_| bad("invalid UTF-16 surrogate"))?,
                    );
                    self.at += 4;
                }
                32..=126 => out.push(c as char),
                _ => return Err(bad("STEP text requires escaped non-ASCII characters")),
            }
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        ensure(
            depth < 32 && self.nodes < 1_000_000,
            "IFC: value complexity limit exceeded",
        )?;
        self.nodes += 1;
        self.ws();
        let c = *self
            .bytes
            .get(self.at)
            .ok_or_else(|| bad("unexpected end"))?;
        match c {
            b'$' => {
                self.at += 1;
                Ok(Value::Null)
            }
            b'*' => {
                self.at += 1;
                Ok(Value::Derived)
            }
            b'#' => {
                self.at += 1;
                let n = self
                    .word()?
                    .parse()
                    .map_err(|_| bad("invalid STEP reference"))?;
                Ok(Value::Ref(n))
            }
            b'\'' => Ok(Value::Text(self.string()?)),
            b'(' => {
                self.at += 1;
                let mut items = Vec::new();
                self.ws();
                if self.bytes.get(self.at) != Some(&b')') {
                    loop {
                        items.push(self.value(depth + 1)?);
                        self.ws();
                        if self.bytes.get(self.at) != Some(&b',') {
                            break;
                        }
                        self.at += 1;
                    }
                }
                self.token(b")")?;
                Ok(Value::List(items))
            }
            b'.' => {
                self.at += 1;
                let s = self.word()?;
                self.token(b".")?;
                Ok(Value::Enum(s))
            }
            b'+' | b'-' | b'0'..=b'9' => {
                let start = self.at;
                while self
                    .bytes
                    .get(self.at)
                    .is_some_and(|c| c.is_ascii_digit() || b".+-Ee".contains(c))
                {
                    self.at += 1;
                }
                let n: f64 = std::str::from_utf8(&self.bytes[start..self.at])
                    .map_err(|_| bad("number encoding"))?
                    .parse()
                    .map_err(|_| bad("invalid number"))?;
                ensure(n.is_finite(), "IFC: non-finite number")?;
                Ok(Value::Number(n))
            }
            _ => Err(bad(format!("unsupported STEP value at byte {}", self.at))),
        }
    }
}
pub fn parse(bytes: &[u8]) -> Result<Entities> {
    ensure(
        bytes.len() <= 16 * 1024 * 1024,
        "IFC: file exceeds 16 MiB limit",
    )?;
    let mut p = Parser {
        bytes,
        at: 0,
        nodes: 0,
    };
    p.token(b"ISO-10303-21;")?;
    p.token(b"HEADER;")?;
    for name in ["FILE_DESCRIPTION", "FILE_NAME", "FILE_SCHEMA"] {
        p.token(name.as_bytes())?;
        let args = p.value(0)?;
        p.token(b";")?;
        if name == "FILE_SCHEMA" {
            ensure(
                args == Value::List(vec![Value::List(vec![Value::Text("IFC4".into())])]),
                "IFC: only IFC4 is supported",
            )?;
        }
    }
    p.token(b"ENDSEC;")?;
    p.token(b"DATA;")?;
    let mut entities = BTreeMap::new();
    loop {
        p.ws();
        if bytes[p.at..].starts_with(b"ENDSEC;") {
            break;
        }
        p.token(b"#")?;
        let id: u32 = p.word()?.parse().map_err(|_| bad("invalid entity id"))?;
        ensure(
            id > 0 && entities.len() < 100_000,
            "IFC: invalid id or entity limit",
        )?;
        p.token(b"=")?;
        let kind = p.word()?;
        let args = p.value(0)?.list()?.to_vec();
        p.token(b";")?;
        ensure(
            entities.insert(id, Entity { kind, args }).is_none(),
            "IFC: duplicate STEP id",
        )?;
    }
    p.token(b"ENDSEC;")?;
    p.token(b"END-ISO-10303-21;")?;
    p.ws();
    ensure(p.at == bytes.len(), "IFC: trailing data")?;
    fn refs(v: &Value, es: &Entities) -> Result<()> {
        match v {
            Value::Ref(id) => ensure(es.contains_key(id), "IFC: dangling STEP reference"),
            Value::List(vs) => {
                for v in vs {
                    refs(v, es)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    for e in entities.values() {
        for v in &e.args {
            refs(v, &entities)?;
        }
    }
    Ok(entities)
}
