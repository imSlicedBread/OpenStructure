//! Bounded, unit-aware arithmetic. References use `@{UUID}`, never display names.
use crate::{LengthParameter, LengthUnit};
use os_core::{Id, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_FORMULA_BYTES: usize = 2048;
pub const MAX_FORMULA_NODES: usize = 128;
pub const MAX_FORMULA_DEPTH: usize = 32;
pub const MAX_PARAMETER_EVALUATIONS: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterExpression {
    Literal(f64, LengthUnit),
    Reference(Id),
    Binary(char, Box<Self>, Box<Self>),
    Negate(Box<Self>),
}
fn invalid(message: &str) -> os_core::Error {
    os_core::Error::Invalid(message.into())
}
impl ParameterExpression {
    pub fn parse(text: &str) -> Result<Self> {
        ensure(
            text.len() <= MAX_FORMULA_BYTES && text.is_ascii(),
            "formula must be ASCII and at most 2048 bytes",
        )?;
        let mut p = Parser {
            text: text.as_bytes(),
            at: 0,
            nodes: 0,
        };
        let expression = p.expression(0, 0)?;
        p.space();
        ensure(p.at == p.text.len(), "unexpected formula token")?;
        ensure(
            expression.depth() <= MAX_FORMULA_DEPTH,
            "formula AST depth budget exceeded",
        )?;
        Ok(expression)
    }
    fn depth(&self) -> usize {
        match self {
            Self::Binary(_, a, b) => 1 + a.depth().max(b.depth()),
            Self::Negate(a) => 1 + a.depth(),
            _ => 1,
        }
    }
    pub fn references(&self) -> BTreeSet<Id> {
        let mut ids = BTreeSet::new();
        self.collect(&mut ids);
        ids
    }
    fn collect(&self, ids: &mut BTreeSet<Id>) {
        match self {
            Self::Reference(id) => {
                ids.insert(*id);
            }
            Self::Binary(_, a, b) => {
                a.collect(ids);
                b.collect(ids);
            }
            Self::Negate(a) => a.collect(ids),
            Self::Literal(..) => {}
        }
    }
    pub fn remapped(&self, ids: &BTreeMap<Id, Id>) -> Result<String> {
        self.format_remapped(ids, 0)
    }
    /// Preserve authored tokens and whitespace; replacing UUIDs with 32-hex IDs
    /// cannot grow a valid source formula beyond its byte or syntax budgets.
    pub fn remap_source(source: &str, ids: &BTreeMap<Id, Id>) -> Result<String> {
        Self::parse(source)?;
        let mut output = String::with_capacity(source.len());
        let mut remaining = source;
        while let Some(start) = remaining.find("@{") {
            output.push_str(&remaining[..start]);
            let tail = &remaining[start + 2..];
            let end = tail
                .find('}')
                .ok_or_else(|| invalid("unclosed parameter reference"))?;
            let id = Id(tail[..end]
                .parse()
                .map_err(|_| invalid("invalid parameter UUID"))?);
            let mapped = ids
                .get(&id)
                .ok_or_else(|| invalid("formula dependency remap missing"))?;
            output.push_str(&format!("@{{{}}}", mapped.0.simple()));
            remaining = &tail[end + 1..];
        }
        output.push_str(remaining);
        ensure(output.len() <= source.len(), "formula remap grew source")?;
        Ok(output)
    }
    fn format_remapped(&self, ids: &BTreeMap<Id, Id>, minimum: u8) -> Result<String> {
        Ok(match self {
            Self::Literal(v, u) => {
                format!("{v}{}", if *u == LengthUnit::Metres { "m" } else { "" })
            }
            Self::Reference(id) => format!(
                "@{{{}}}",
                ids.get(id)
                    .ok_or_else(|| invalid("formula dependency remap missing"))?
                    .0
                    .simple()
            ),
            Self::Binary(op, a, b) => {
                let precedence = if matches!(op, '+' | '-') { 1 } else { 2 };
                let text = format!(
                    "{}{op}{}",
                    a.format_remapped(ids, precedence)?,
                    b.format_remapped(ids, precedence + 1)?
                );
                if precedence < minimum {
                    format!("({text})")
                } else {
                    text
                }
            }
            Self::Negate(a) => format!("-{}", a.format_remapped(ids, 3)?),
        })
    }
}
struct Parser<'a> {
    text: &'a [u8],
    at: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn node(&mut self) -> Result<()> {
        self.nodes += 1;
        ensure(
            self.nodes <= MAX_FORMULA_NODES,
            "formula node budget exceeded",
        )
    }
    fn expression(&mut self, minimum: u8, depth: usize) -> Result<ParameterExpression> {
        ensure(depth < MAX_FORMULA_DEPTH, "formula depth budget exceeded")?;
        self.space();
        let mut left = match self.text.get(self.at).copied() {
            Some(b'-') => {
                self.node()?;
                self.at += 1;
                ParameterExpression::Negate(Box::new(self.expression(3, depth + 1)?))
            }
            Some(b'(') => {
                self.at += 1;
                let e = self.expression(0, depth + 1)?;
                self.space();
                ensure(
                    self.text.get(self.at) == Some(&b')'),
                    "missing closing parenthesis",
                )?;
                self.at += 1;
                e
            }
            Some(b'@') => {
                self.node()?;
                self.at += 1;
                ensure(
                    self.text.get(self.at) == Some(&b'{'),
                    "reference syntax is @{UUID}",
                )?;
                self.at += 1;
                let start = self.at;
                while self.text.get(self.at).is_some_and(|b| *b != b'}') {
                    self.at += 1;
                }
                ensure(
                    self.text.get(self.at) == Some(&b'}'),
                    "unclosed parameter reference",
                )?;
                let id = Id(std::str::from_utf8(&self.text[start..self.at])
                    .unwrap()
                    .parse()
                    .map_err(|_| invalid("invalid parameter UUID"))?);
                self.at += 1;
                ParameterExpression::Reference(id)
            }
            Some(b) if b.is_ascii_digit() || b == b'.' => {
                self.node()?;
                let start = self.at;
                while self
                    .text
                    .get(self.at)
                    .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
                {
                    self.at += 1;
                }
                let value = std::str::from_utf8(&self.text[start..self.at])
                    .unwrap()
                    .parse::<f64>()
                    .map_err(|_| invalid("invalid number"))?;
                ensure(
                    value.is_finite() && value <= 1e6,
                    "literal outside finite range",
                )?;
                let unit = if self.text.get(self.at) == Some(&b'm') {
                    self.at += 1;
                    LengthUnit::Metres
                } else {
                    LengthUnit::Scalar
                };
                ParameterExpression::Literal(value, unit)
            }
            _ => return Err(invalid("expected literal, UUID reference or parenthesis")),
        };
        loop {
            self.space();
            let Some(op) = self.text.get(self.at).copied() else {
                break;
            };
            let precedence = match op {
                b'+' | b'-' => 1,
                b'*' | b'/' => 2,
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.at += 1;
            self.node()?;
            let right = self.expression(precedence + 1, depth + 1)?;
            left = ParameterExpression::Binary(op as char, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterValue {
    pub value: f64,
    pub unit: LengthUnit,
}
impl ParameterValue {
    fn checked(self) -> Result<Self> {
        ensure(
            self.value.is_finite() && self.value.abs() <= 1e6,
            "parameter result outside finite ±1000000 range",
        )?;
        Ok(self)
    }
    pub fn length(self) -> Result<f64> {
        ensure(
            self.unit == LengthUnit::Metres,
            "binding requires Length, received Scalar",
        )?;
        Ok(self.value)
    }
}
pub fn evaluate_parameters(
    parameters: &BTreeMap<Id, LengthParameter>,
) -> Result<BTreeMap<Id, ParameterValue>> {
    ensure(
        parameters.len() <= crate::MAX_LENGTH_PARAMETERS,
        "parameter count budget exceeded",
    )?;
    fn dependency_depth(
        id: Id,
        ps: &BTreeMap<Id, LengthParameter>,
        active: &mut BTreeSet<Id>,
        depths: &mut BTreeMap<Id, usize>,
    ) -> Result<usize> {
        if let Some(depth) = depths.get(&id) {
            return Ok(*depth);
        }
        ensure(
            active.len() < MAX_FORMULA_DEPTH && active.insert(id),
            "parameter dependency cycle/depth budget exceeded",
        )?;
        let p = &ps
            .get(&id)
            .ok_or_else(|| invalid("referenced parameter missing"))?
            .parameters;
        let mut depth = 1;
        if let Some(formula) = &p.formula {
            for reference in ParameterExpression::parse(formula)?.references() {
                depth = depth.max(1 + dependency_depth(reference, ps, active, depths)?);
            }
        }
        ensure(
            depth <= MAX_FORMULA_DEPTH,
            "parameter dependency depth budget exceeded",
        )?;
        active.remove(&id);
        depths.insert(id, depth);
        Ok(depth)
    }
    let mut depths = BTreeMap::new();
    for id in parameters.keys() {
        dependency_depth(*id, parameters, &mut BTreeSet::new(), &mut depths)?;
    }
    let mut evaluator = Evaluator {
        parameters,
        cache: BTreeMap::new(),
        visiting: BTreeSet::new(),
        remaining: MAX_PARAMETER_EVALUATIONS,
    };
    for id in parameters.keys() {
        evaluator.parameter(*id, 0)?;
    }
    Ok(evaluator.cache)
}
struct Evaluator<'a> {
    parameters: &'a BTreeMap<Id, LengthParameter>,
    cache: BTreeMap<Id, ParameterValue>,
    visiting: BTreeSet<Id>,
    remaining: usize,
}
impl Evaluator<'_> {
    fn step(&mut self, depth: usize) -> Result<()> {
        ensure(
            depth < MAX_FORMULA_DEPTH && self.remaining > 0,
            "parameter evaluation depth/work budget exceeded",
        )?;
        self.remaining -= 1;
        Ok(())
    }
    fn parameter(&mut self, id: Id, depth: usize) -> Result<ParameterValue> {
        self.step(depth)?;
        if let Some(value) = self.cache.get(&id) {
            return Ok(*value);
        }
        ensure(
            self.visiting.insert(id),
            format!("parameter dependency cycle at {id}"),
        )?;
        let p = &self
            .parameters
            .get(&id)
            .ok_or_else(|| invalid("referenced parameter missing"))?
            .parameters;
        p.validate()?;
        let unit = p.unit;
        let value = if let Some(formula) = &p.formula {
            self.expression(&ParameterExpression::parse(formula)?, 0)?
        } else {
            ParameterValue {
                value: p.value,
                unit,
            }
        };
        ensure(
            value.unit == unit,
            format!("formula dimension mismatch for {id}"),
        )?;
        let value = value.checked()?;
        self.visiting.remove(&id);
        self.cache.insert(id, value);
        Ok(value)
    }
    fn expression(&mut self, e: &ParameterExpression, depth: usize) -> Result<ParameterValue> {
        self.step(depth)?;
        match e {
            ParameterExpression::Literal(value, unit) => ParameterValue {
                value: *value,
                unit: *unit,
            }
            .checked(),
            ParameterExpression::Reference(id) => self.parameter(*id, 0),
            ParameterExpression::Negate(a) => {
                let a = self.expression(a, depth + 1)?;
                ParameterValue {
                    value: -a.value,
                    ..a
                }
                .checked()
            }
            ParameterExpression::Binary(op, a, b) => {
                let a = self.expression(a, depth + 1)?;
                let b = self.expression(b, depth + 1)?;
                let (value, unit) = match op {
                    '+' | '-' => {
                        ensure(
                            a.unit == b.unit,
                            "addition/subtraction requires matching dimensions",
                        )?;
                        (
                            if *op == '+' {
                                a.value + b.value
                            } else {
                                a.value - b.value
                            },
                            a.unit,
                        )
                    }
                    '*' => {
                        ensure(
                            a.unit == LengthUnit::Scalar || b.unit == LengthUnit::Scalar,
                            "Length × Length is unsupported",
                        )?;
                        (
                            a.value * b.value,
                            if a.unit == LengthUnit::Scalar {
                                b.unit
                            } else {
                                a.unit
                            },
                        )
                    }
                    '/' => {
                        ensure(b.value != 0., "division by zero")?;
                        ensure(
                            b.unit == LengthUnit::Scalar || a.unit == b.unit,
                            "Scalar / Length is unsupported",
                        )?;
                        (
                            a.value / b.value,
                            if a.unit == b.unit {
                                LengthUnit::Scalar
                            } else {
                                a.unit
                            },
                        )
                    }
                    _ => return Err(invalid("unsupported operator")),
                };
                ParameterValue { value, unit }.checked()
            }
        }
    }
}

pub fn parameter_dependencies(
    parameters: &BTreeMap<Id, LengthParameter>,
    roots: BTreeSet<Id>,
) -> Result<BTreeSet<Id>> {
    let mut result = BTreeSet::new();
    let mut pending: Vec<_> = roots.into_iter().collect();
    while let Some(id) = pending.pop() {
        if !result.insert(id) {
            continue;
        }
        ensure(
            result.len() <= crate::MAX_LENGTH_PARAMETERS,
            "parameter dependency budget exceeded",
        )?;
        let p = &parameters
            .get(&id)
            .ok_or_else(|| invalid("parameter dependency missing"))?
            .parameters;
        if let Some(formula) = &p.formula {
            pending.extend(ParameterExpression::parse(formula)?.references());
        }
    }
    Ok(result)
}
