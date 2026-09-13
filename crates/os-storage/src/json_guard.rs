//! Reject ambiguous JSON before Value parsing can collapse duplicate object keys.
use serde::{
    Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::BTreeSet, fmt};

pub(crate) fn validate(raw: &str) -> Result<(), serde_json::Error> {
    check(raw, 0)
}

fn check(raw: &str, depth: usize) -> Result<(), serde_json::Error> {
    if depth > 128 {
        return Err(de::Error::custom("JSON nesting exceeds 128"));
    }
    let mut parser = serde_json::Deserializer::from_str(raw);
    match raw.trim_start().as_bytes().first() {
        Some(b'{') => parser.deserialize_map(Keys(depth))?,
        Some(b'[') => parser.deserialize_seq(Keys(depth))?,
        _ => {
            de::IgnoredAny::deserialize(&mut parser)?;
        }
    }
    parser.end()
}

use serde::Deserialize;
struct Keys(usize);
impl<'de> Visitor<'de> for Keys {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("unambiguous JSON")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            // serde_json's arbitrary-precision Value representation reserves this
            // key. Reject it explicitly instead of reinterpreting an opaque object.
            if key == "$serde_json::private::Number" {
                return Err(de::Error::custom("reserved JSON number representation key"));
            }
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            let value: &RawValue = map.next_value()?;
            check(value.get(), self.0 + 1).map_err(de::Error::custom)?;
        }
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(value) = seq.next_element::<&RawValue>()? {
            check(value.get(), self.0 + 1).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_duplicates_escaped_aliases_reserved_keys_and_trailing_input() {
        for raw in [
            r#"{"x":1,"x":2}"#,
            r#"[{"x":1,"\u0078":2}]"#,
            r#"{"$serde_json::private::Number":"42"}"#,
            "{} {}",
        ] {
            assert!(validate(raw).is_err(), "accepted {raw}");
        }
        validate(r#"{"precise":184467440737095516170,"items":[null,true,{"x":1e400}]}"#).unwrap();
    }
}
