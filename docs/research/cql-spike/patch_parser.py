"""Spike-only patches to a COPY of octofhir-cql's parser (cql-rs commit 848156b,
crate 0.2.5). They get past four grammar gaps so we could see how far WHO SMART
libraries get in translation and evaluation. They are not upstream-quality fixes.
Usage: python3 -I patch_parser.py <crates/octofhir-cql-parser/src/expression.rs>
"""
import sys

path = sys.argv[1]
src = open(path).read()

def sub(old, new):
    global src
    assert src.count(old) == 1, old[:80]
    src = src.replace(old, new)

# 1. Multi-part qualified type names: FHIR.Dosage.DoseAndRate
sub("""        if let Ok(inner_name) = identifier_or_keyword_parser(input) {
            return Ok(TypeSpecifier::Named(NamedTypeSpecifier {
                namespace: Some(name),
                name: inner_name.name,
            }));
        }""",
"""        if let Ok(inner_name) = identifier_or_keyword_parser(input) {
            let mut full = inner_name.name;
            loop {
                let cp = *input;
                if lit(".").parse_next(input).is_ok() {
                    if let Ok(more) = identifier_or_keyword_parser(input) {
                        full = format!("{}.{}", full, more.name);
                        continue;
                    }
                }
                *input = cp;
                break;
            }
            return Ok(TypeSpecifier::Named(NamedTypeSpecifier {
                namespace: Some(name),
                name: full,
            }));
        }""")

# 2. `exists <aliased query>` without parentheses
sub("""    if padded_keyword("exists").parse_next(input).is_ok() {
        let operand = unary_expression(input)?;""",
"""    if padded_keyword("exists").parse_next(input).is_ok() {
        let cp = *input;
        if let Ok(q) = query_expression(input) {
            return Ok(dummy_span(Expression::UnaryOp(UnaryOpExpr {
                op: UnaryOp::Exists,
                operand: Box::new(q),
            })));
        }
        *input = cp;
        let operand = unary_expression(input)?;""")

# 3. `duration in <precision> of <interval>` desugared to
#    `<precision> between start of X and end of X`
sub("""    ws.parse_next(input)?;

    // Expect "between"
    if keyword("between").parse_next(input).is_err() {
        *input = checkpoint;
        return Err(ContextError::new());
    }""",
"""    ws.parse_next(input)?;

    if keyword("of").parse_next(input).is_ok() {
        ws.parse_next(input)?;
        let operand = interval_operator_expression(input)?;
        let low = point_call("Start", operand.clone());
        let high = point_call("End", operand);
        return Ok(dummy_span(if is_difference {
            Expression::DifferenceBetween(DifferenceBetweenExpr { precision, low: Box::new(low), high: Box::new(high) })
        } else {
            Expression::DurationBetween(DurationBetweenExpr { precision, low: Box::new(low), high: Box::new(high) })
        }));
    }

    // Expect "between"
    if keyword("between").parse_next(input).is_err() {
        *input = checkpoint;
        return Err(ContextError::new());
    }""")

# 4. `X less than 12 months before Y`: do not read `less`/`more` as a query
#    alias when followed by `than`.
sub("""fn query_alias<'a>(input: &mut Input<'a>) -> PResult<Identifier> {
    identifier_parser
        .verify(|id: &Identifier| id.quoted || !opens_expression_construct(&id.name))
        .parse_next(input)
}""",
"""fn query_alias<'a>(input: &mut Input<'a>) -> PResult<Identifier> {
    let id = identifier_parser
        .verify(|id: &Identifier| id.quoted || !opens_expression_construct(&id.name))
        .parse_next(input)?;
    if !id.quoted && (id.name == "less" || id.name == "more") {
        let cp = *input;
        if padded_keyword("than").parse_next(input).is_ok() {
            *input = cp;
            return Err(ContextError::new());
        }
        *input = cp;
    }
    Ok(id)
}""")

open(path, "w").write(src)
print("patched", path)
