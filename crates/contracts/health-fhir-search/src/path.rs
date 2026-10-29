//! The FHIRPath subset that SearchParameter expressions use (fhir-r5-model.md § Search parameters
//! and indexed projections).
//!
//! Search extraction only navigates: it never computes values. The subset is unions of paths,
//! with `ofType(T)`, `as T`, `as(T)`, `[n]`, `first()`, `.(a | b)`, `extension('url')`,
//! `where(resolve() is T)` and `where(field = 'literal')`.
//! An expression outside it fails to compile, and its parameter is not supported.

use std::borrow::Cow;

use serde_json::Value;

/// A compiled search expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPath {
    terms: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Term {
    /// `Type.a.b`, rooted at a resource type, `Resource` or `DomainResource`.
    Rooted { root: String, steps: Vec<Step> },
    /// `( expression ).a.b`.
    Group {
        inner: Box<SearchPath>,
        steps: Vec<Step>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Child(String),
    /// `.(a | b.c)`: a union evaluated relative to each element.
    Relative(Box<SearchPath>),
    OfType(String),
    Index(usize),
    Extension(String),
    WhereResolveIs(String),
    WhereEquals {
        field: String,
        literal: String,
    },
}

/// An expression outside the supported subset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported search expression at byte {at}: {reason}")]
pub struct PathError {
    /// Byte offset of the problem.
    pub at: usize,
    /// What was not understood.
    pub reason: String,
}

/// One element found by a [`SearchPath`].
#[derive(Debug, Clone, PartialEq)]
pub struct Node<'a> {
    /// The element's JSON.
    pub value: &'a Value,
    /// Its FHIR type when the path knows it: the suffix of a choice element (`Quantity` for
    /// `valueQuantity`, `dateTime` for `effectiveDateTime`) or a resource's `resourceType`.
    pub type_name: Option<Cow<'a, str>>,
}

const PRIMITIVES: &[&str] = &[
    "base64Binary",
    "boolean",
    "canonical",
    "code",
    "date",
    "dateTime",
    "decimal",
    "id",
    "instant",
    "integer",
    "integer64",
    "markdown",
    "oid",
    "positiveInt",
    "string",
    "time",
    "unsignedInt",
    "uri",
    "url",
    "uuid",
];

impl SearchPath {
    /// Compiles `expression`, or explains why it is outside the subset.
    pub fn parse(expression: &str) -> Result<Self, PathError> {
        let tokens = tokenize(expression)?;
        let mut parser = Parser { tokens, pos: 0 };
        let path = parser.union()?;
        match parser.peek() {
            None => Ok(path),
            Some((at, token)) => Err(PathError {
                at: *at,
                reason: format!("unexpected {token:?}"),
            }),
        }
    }

    /// The elements this expression selects in `resource`.
    pub fn evaluate<'a>(&self, resource: &'a Value) -> Vec<Node<'a>> {
        self.run(resource, false)
    }

    /// The elements this expression selects relative to `focus`, as a composite component's
    /// expression (`code`, `value.ofType(Quantity)`) is evaluated against each element its
    /// composite selects.
    pub fn evaluate_relative<'a>(&self, focus: &'a Value) -> Vec<Node<'a>> {
        self.run(focus, true)
    }

    fn run<'a>(&self, focus: &'a Value, relative: bool) -> Vec<Node<'a>> {
        let resource_type = focus.get("resourceType").and_then(Value::as_str);
        let mut out = Vec::new();
        for term in &self.terms {
            let (start, steps) = match term {
                Term::Rooted { root, steps } if relative => {
                    (apply(&Step::Child(root.clone()), vec![typed(focus)]), steps)
                }
                Term::Rooted { root, steps } => {
                    let matches = matches!(root.as_str(), "Resource" | "DomainResource")
                        || Some(root.as_str()) == resource_type;
                    if !matches {
                        continue;
                    }
                    (vec![typed(focus)], steps)
                }
                Term::Group { inner, steps } => (inner.run(focus, relative), steps),
            };
            out.extend(steps.iter().fold(start, |nodes, step| apply(step, nodes)));
        }
        out
    }
}

fn typed(value: &Value) -> Node<'_> {
    Node {
        value,
        type_name: value
            .get("resourceType")
            .and_then(Value::as_str)
            .map(Cow::Borrowed),
    }
}

fn flatten<'a>(value: &'a Value, type_name: Option<Cow<'a, str>>, out: &mut Vec<Node<'a>>) {
    match value {
        Value::Array(items) => {
            for item in items {
                let mut node = typed(item);
                if node.type_name.is_none() {
                    node.type_name.clone_from(&type_name);
                }
                out.push(node);
            }
        }
        Value::Null => {}
        other => {
            let mut node = typed(other);
            if node.type_name.is_none() {
                node.type_name = type_name;
            }
            out.push(node);
        }
    }
}

/// The FHIRPath type name of a choice suffix: `DateTime` is `dateTime`, `Quantity` stays.
fn choice_type(suffix: &str) -> Cow<'_, str> {
    PRIMITIVES
        .iter()
        .find(|p| p.eq_ignore_ascii_case(suffix))
        .map_or(Cow::Borrowed(suffix), |p| Cow::Borrowed(*p))
}

fn apply<'a>(step: &Step, nodes: Vec<Node<'a>>) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    match step {
        Step::Child(name) => {
            for node in nodes {
                let Value::Object(object) = node.value else {
                    continue;
                };
                if let Some(value) = object.get(name) {
                    flatten(value, None, &mut out);
                    continue;
                }
                for (key, value) in object {
                    if let Some(suffix) = key.strip_prefix(name.as_str())
                        && suffix.starts_with(|c: char| c.is_ascii_uppercase())
                    {
                        flatten(value, Some(choice_type(suffix)), &mut out);
                    }
                }
            }
        }
        Step::Relative(inner) => {
            for node in nodes {
                out.extend(inner.run(node.value, true));
            }
        }
        Step::OfType(type_name) => {
            out.extend(
                nodes
                    .into_iter()
                    .filter(|n| n.type_name.as_deref() == Some(type_name.as_str())),
            );
        }
        Step::Index(index) => out.extend(nodes.into_iter().nth(*index)),
        Step::Extension(url) => {
            for node in nodes {
                if let Some(Value::Array(extensions)) = node.value.get("extension") {
                    out.extend(
                        extensions
                            .iter()
                            .filter(|e| e.get("url").and_then(Value::as_str) == Some(url))
                            .map(typed),
                    );
                }
            }
        }
        Step::WhereResolveIs(target) => out.extend(nodes.into_iter().filter(|n| {
            n.value
                .get("reference")
                .and_then(Value::as_str)
                .and_then(crate::normalize::reference_type)
                == Some(target.as_str())
        })),
        Step::WhereEquals { field, literal } => out.extend(
            nodes
                .into_iter()
                .filter(|n| n.value.get(field).and_then(Value::as_str) == Some(literal)),
        ),
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Str(String),
    Int(usize),
    Punct(char),
}

fn tokenize(text: &str) -> Result<Vec<(usize, Token)>, PathError> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some(&(at, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut ident = String::new();
            while let Some(&(_, c)) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    ident.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push((at, Token::Ident(ident)));
        } else if c.is_ascii_digit() {
            let mut digits = String::new();
            while let Some(&(_, c)) = chars.peek() {
                if c.is_ascii_digit() {
                    digits.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            let value = digits.parse().map_err(|_| PathError {
                at,
                reason: "index too large".into(),
            })?;
            tokens.push((at, Token::Int(value)));
        } else if c == '\'' {
            chars.next();
            let mut literal = String::new();
            loop {
                match chars.next() {
                    Some((_, '\'')) => break,
                    Some((_, '\\')) => match chars.next() {
                        Some((_, escaped)) => literal.push(escaped),
                        None => break,
                    },
                    Some((_, c)) => literal.push(c),
                    None => {
                        return Err(PathError {
                            at,
                            reason: "unterminated string".into(),
                        });
                    }
                }
            }
            tokens.push((at, Token::Str(literal)));
        } else if "|.()[]=".contains(c) {
            chars.next();
            tokens.push((at, Token::Punct(c)));
        } else {
            return Err(PathError {
                at,
                reason: format!("operator `{c}` is not in the search subset"),
            });
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<(usize, Token)>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&(usize, Token)> {
        self.tokens.get(self.pos)
    }

    fn at(&self) -> usize {
        self.peek().map_or_else(
            || self.tokens.last().map_or(0, |(at, _)| *at),
            |(at, _)| *at,
        )
    }

    fn error(&self, reason: impl Into<String>) -> PathError {
        PathError {
            at: self.at(),
            reason: reason.into(),
        }
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).map(|(_, t)| t.clone());
        self.pos += 1;
        token
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek().map(|(_, t)| t) == Some(&Token::Punct(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), PathError> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.error(format!("expected `{c}`")))
        }
    }

    fn ident(&mut self) -> Result<String, PathError> {
        match self.next() {
            Some(Token::Ident(name)) => Ok(name),
            _ => {
                self.pos -= 1;
                Err(self.error("expected a name"))
            }
        }
    }

    fn string(&mut self) -> Result<String, PathError> {
        match self.next() {
            Some(Token::Str(text)) => Ok(text),
            _ => {
                self.pos -= 1;
                Err(self.error("expected a string literal"))
            }
        }
    }

    fn keyword(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some((_, Token::Ident(w))) if w == word) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn union(&mut self) -> Result<SearchPath, PathError> {
        let mut terms = vec![self.term()?];
        while self.eat('|') {
            terms.push(self.term()?);
        }
        Ok(SearchPath { terms })
    }

    fn term(&mut self) -> Result<Term, PathError> {
        let mut term = if self.eat('(') {
            let inner = self.union()?;
            self.expect(')')?;
            Term::Group {
                inner: Box::new(inner),
                steps: Vec::new(),
            }
        } else {
            Term::Rooted {
                root: self.ident()?,
                steps: Vec::new(),
            }
        };
        let steps = match &mut term {
            Term::Rooted { steps, .. } | Term::Group { steps, .. } => steps,
        };
        loop {
            if self.eat('.') {
                steps.push(self.step()?);
            } else if self.eat('[') {
                match self.next() {
                    Some(Token::Int(index)) => steps.push(Step::Index(index)),
                    _ => return Err(self.error("expected an index")),
                }
                self.expect(']')?;
            } else if self.keyword("as") {
                steps.push(Step::OfType(self.ident()?));
            } else {
                return Ok(term);
            }
        }
    }

    fn step(&mut self) -> Result<Step, PathError> {
        if self.eat('(') {
            let inner = self.union()?;
            self.expect(')')?;
            return Ok(Step::Relative(Box::new(inner)));
        }
        let name = self.ident()?;
        if !self.eat('(') {
            return Ok(Step::Child(name));
        }
        let step = match name.as_str() {
            "ofType" | "as" => Step::OfType(self.ident()?),
            "first" => Step::Index(0),
            "extension" => Step::Extension(self.string()?),
            "where" => {
                if self.keyword("resolve") {
                    self.expect('(')?;
                    self.expect(')')?;
                    if !self.keyword("is") {
                        return Err(self.error("expected `is` after resolve()"));
                    }
                    Step::WhereResolveIs(self.ident()?)
                } else {
                    let field = self.ident()?;
                    self.expect('=')?;
                    Step::WhereEquals {
                        field,
                        literal: self.string()?,
                    }
                }
            }
            other => return Err(self.error(format!("function `{other}` is not in the subset"))),
        };
        self.expect(')')?;
        Ok(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn values(expression: &str, resource: &Value) -> Vec<(Value, Option<String>)> {
        SearchPath::parse(expression)
            .expect("parses")
            .evaluate(resource)
            .into_iter()
            .map(|n| (n.value.clone(), n.type_name.map(Cow::into_owned)))
            .collect()
    }

    #[test]
    fn navigates_arrays_and_choice_types() {
        let obs = json!({"resourceType":"Observation","code":{"coding":[{"code":"a"},{"code":"b"}]},"valueQuantity":{"value":1},"effectiveDateTime":"2026"});
        assert_eq!(values("Observation.code.coding.code", &obs).len(), 2);
        assert_eq!(
            values("(Observation.value as Quantity)", &obs),
            vec![(json!({"value":1}), Some("Quantity".into()))]
        );
        assert_eq!(
            values("Observation.effective.ofType(dateTime)", &obs),
            vec![(json!("2026"), Some("dateTime".into()))]
        );
        assert!(values("Observation.effective.ofType(Period)", &obs).is_empty());
        assert!(values("Patient.name", &obs).is_empty());
        assert_eq!(values("Resource.resourceType", &obs).len(), 1);
    }

    #[test]
    fn filters_by_reference_type_literal_and_extension() {
        let obs = json!({"resourceType":"Observation","subject":{"reference":"Patient/1"},
            "performer":[{"reference":"Practitioner/2"},{"reference":"Patient/3"}],
            "extension":[{"url":"x","valueString":"y"}]});
        assert_eq!(
            values(
                "Observation.performer.where(resolve() is Patient) | Observation.subject.where(resolve() is Group)",
                &obs
            )
            .len(),
            1
        );
        assert_eq!(
            values("Observation.extension('x').value", &obs),
            vec![(json!("y"), Some("string".into()))]
        );
        let patient = json!({"resourceType":"Patient","telecom":[{"system":"phone","value":"1"},{"system":"email","value":"2"}]});
        assert_eq!(
            values("Patient.telecom.where(system='email')", &patient)[0].0["value"],
            json!("2")
        );
        assert_eq!(
            values("Patient.telecom[1].value", &patient)[0].0,
            json!("2")
        );
    }

    #[test]
    fn rejects_expressions_outside_the_subset() {
        for expression in [
            "Patient.deceased.exists() and Patient.deceased != false",
            "Patient.name.last()",
            "Patient.name +",
        ] {
            assert!(SearchPath::parse(expression).is_err(), "{expression}");
        }
    }
}
