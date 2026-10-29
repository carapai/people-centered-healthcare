//! Compiles search clauses into record-id range scans over the index tables
//! (persistence.md § Search index tables: a search compiles to record range scans).
//!
//! Every scan returns resource ids. A value with every key element known is a range over the
//! table's record ids; a value that leaves a leading element open (a token without a system, a
//! quantity without a unit, a reference id without a type) uses the table's secondary index
//! instead. The persistence tests check the plan of each shape with `EXPLAIN`.

use health_fhir_search::normalize::decimal_range;
use health_fhir_search::{Clause, ParamType, Prefix, SearchValue, StringMatch, Test};
use rust_decimal::Decimal;

use super::sql::{Bind, MAX, Sql};
use super::write::table;

/// A search the index cannot answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot search `{param}`: {reason}")]
pub struct Unsupported {
    /// The parameter.
    pub param: String,
    /// Why.
    pub reason: String,
}

/// How one clause's resource ids are computed from scan results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClausePlan {
    /// The union of these scans.
    Union(Vec<usize>),
    /// The union of `universe` without the union of `minus` (`:not`).
    Except {
        universe: Vec<usize>,
        minus: Vec<usize>,
    },
}

const SELECT: &str = "SELECT VALUE array::last(record::id(id))";

struct Scans<'a> {
    sql: &'a mut Sql,
    explain: bool,
    out: Vec<usize>,
}

impl Scans<'_> {
    fn push(&mut self, statement: String) {
        let statement = if self.explain {
            format!("{statement} EXPLAIN")
        } else {
            statement
        };
        self.out.push(self.sql.push(statement));
    }

    /// `FROM table:[lo]..[hi] WHERE filter`.
    fn range(&mut self, table: &str, lo: &[String], hi: &[String], filter: Option<String>) {
        let filter = filter.map(|f| format!(" WHERE {f}")).unwrap_or_default();
        self.push(format!(
            "{SELECT} FROM {table}:[{}]..[{}]{filter}",
            lo.join(", "),
            hi.join(", ")
        ));
    }

    /// The whole key prefix: `[prefix]..[prefix, {}]`.
    fn prefix(&mut self, table: &str, prefix: &[String], filter: Option<String>) {
        self.range(table, prefix, &with(prefix, &[MAX]), filter);
    }

    /// `FROM table WHERE rt = .. AND param = .. AND conditions`, served by a secondary index.
    fn indexed(&mut self, table: &str, rt: &str, param: &str, conditions: &[String]) {
        let mut all = vec![format!("rt = {rt}"), format!("param = {param}")];
        all.extend_from_slice(conditions);
        self.push(format!("{SELECT} FROM {table} WHERE {}", all.join(" AND ")));
    }
}

fn with(prefix: &[String], more: &[&str]) -> Vec<String> {
    let mut out = prefix.to_vec();
    out.extend(more.iter().map(|s| (*s).to_owned()));
    out
}

/// One bound of a number range.
#[derive(Clone, Copy)]
enum Bound {
    Open,
    Inclusive(Decimal),
    Exclusive(Decimal),
}

/// The values a number prefix selects: `x` in `(lo, hi)` with the given bounds, or outside
/// them for `ne`.
fn number_bounds(prefix: Prefix, value: Decimal) -> (Bound, Bound, bool) {
    match prefix {
        Prefix::Eq | Prefix::Ne => {
            let (lo, hi) = decimal_range(value);
            (
                Bound::Inclusive(lo),
                Bound::Exclusive(hi),
                prefix == Prefix::Ne,
            )
        }
        Prefix::Gt | Prefix::Sa => (Bound::Exclusive(value), Bound::Open, false),
        Prefix::Ge => (Bound::Inclusive(value), Bound::Open, false),
        Prefix::Lt | Prefix::Eb => (Bound::Open, Bound::Exclusive(value), false),
        Prefix::Le => (Bound::Open, Bound::Inclusive(value), false),
        Prefix::Ap => {
            let spread = value.abs() / Decimal::TEN;
            (
                Bound::Inclusive(value - spread),
                Bound::Inclusive(value + spread),
                false,
            )
        }
    }
}

/// The condition on `x` a number prefix selects.
fn number_condition(sql: &mut Sql, x: &str, prefix: Prefix, value: Decimal) -> String {
    let (lo, hi, negate) = number_bounds(prefix, value);
    let mut parts = Vec::new();
    match lo {
        Bound::Inclusive(v) => parts.push(format!(
            "{x} >= {}",
            sql.value(Bind::Decimal(v.to_string()))
        )),
        Bound::Exclusive(v) => {
            parts.push(format!("{x} > {}", sql.value(Bind::Decimal(v.to_string()))))
        }
        Bound::Open => {}
    }
    match hi {
        Bound::Inclusive(v) => parts.push(format!(
            "{x} <= {}",
            sql.value(Bind::Decimal(v.to_string()))
        )),
        Bound::Exclusive(v) => {
            parts.push(format!("{x} < {}", sql.value(Bind::Decimal(v.to_string()))))
        }
        Bound::Open => {}
    }
    let condition = parts.join(" AND ");
    if negate {
        format!("!({condition})")
    } else {
        condition
    }
}

/// A number range as key bounds after `prefix`, or `None` for `ne`, which needs a filter.
fn number_range(
    sql: &mut Sql,
    prefix: &[String],
    p: Prefix,
    value: Decimal,
) -> Option<(Vec<String>, Vec<String>)> {
    let (lo, hi, negate) = number_bounds(p, value);
    if negate {
        return None;
    }
    let mut decimal = |v: Decimal| sql.value(Bind::Decimal(v.to_string()));
    let lo = match lo {
        Bound::Open => prefix.to_vec(),
        Bound::Inclusive(v) => with(prefix, &[&decimal(v)]),
        Bound::Exclusive(v) => with(prefix, &[&decimal(v), MAX]),
    };
    let hi = match hi {
        Bound::Open => with(prefix, &[MAX]),
        Bound::Inclusive(v) => with(prefix, &[&decimal(v), MAX]),
        Bound::Exclusive(v) => with(prefix, &[&decimal(v)]),
    };
    Some((lo, hi))
}

/// Element `n` of the record id, for filters.
fn element(n: usize) -> String {
    format!("record::id(id)[{n}]")
}

struct Context<'a> {
    rt: &'a str,
    param_var: String,
    param: &'a str,
    table: &'static str,
    /// Single-value entries have secondary-index fields; composite entries do not.
    standalone: bool,
}

fn scan_value(
    scans: &mut Scans<'_>,
    cx: &Context<'_>,
    prefix: &[String],
    value: &SearchValue,
) -> Result<(), Unsupported> {
    let n = prefix.len();
    let table = cx.table;
    match value {
        SearchValue::Token { system, code } => match (system, code) {
            (Some(system), Some(code)) => {
                let (s, c) = (scans.sql.str(system), scans.sql.str(code));
                scans.prefix(table, &with(prefix, &[&s, &c]), None);
            }
            (Some(system), None) => {
                let s = scans.sql.str(system);
                scans.prefix(table, &with(prefix, &[&s]), None);
            }
            (None, Some(code)) => {
                let c = scans.sql.str(code);
                if cx.standalone {
                    scans.indexed(table, cx.rt, &cx.param_var, &[format!("code = {c}")]);
                } else {
                    scans.prefix(table, prefix, Some(format!("{} = {c}", element(n + 1))));
                }
            }
            (None, None) => scans.prefix(table, prefix, None),
        },
        SearchValue::String { text, mode } => match mode {
            StringMatch::StartsWith => {
                let lo = scans.sql.str(text.as_str());
                let hi = scans.sql.str(format!("{text}\u{10FFFF}"));
                scans.range(table, &with(prefix, &[&lo]), &with(prefix, &[&hi]), None);
            }
            StringMatch::Exact => {
                let folded = scans.sql.str(health_fhir_search::normalize::fold(text));
                let filter = cx
                    .standalone
                    .then(|| format!("exact = {}", scans.sql.str(text.as_str())));
                scans.prefix(table, &with(prefix, &[&folded]), filter);
            }
            StringMatch::Contains => {
                let t = scans.sql.str(text.as_str());
                scans.prefix(
                    table,
                    prefix,
                    Some(format!("string::contains({}, {t})", element(n))),
                );
            }
        },
        SearchValue::Date { prefix: p, range } => {
            let qs = scans.sql.value(Bind::Datetime(range.start.to_string()));
            let qe = scans.sql.value(Bind::Datetime(range.end.to_string()));
            let start = element(n);
            let end = if cx.standalone {
                "end".to_owned()
            } else {
                element(n + 1)
            };
            match p {
                Prefix::Eq => scans.range(
                    table,
                    &with(prefix, &[&qs]),
                    &with(prefix, &[&qe]),
                    Some(format!("{end} <= {qe}")),
                ),
                Prefix::Ne => scans.prefix(
                    table,
                    prefix,
                    Some(format!("!({start} >= {qs} AND {end} <= {qe})")),
                ),
                Prefix::Lt => scans.range(table, prefix, &with(prefix, &[&qs]), None),
                Prefix::Le => scans.range(table, prefix, &with(prefix, &[&qe]), None),
                Prefix::Sa => {
                    scans.range(table, &with(prefix, &[&qe]), &with(prefix, &[MAX]), None)
                }
                Prefix::Ap => scans.range(
                    table,
                    prefix,
                    &with(prefix, &[&qe]),
                    Some(format!("{end} > {qs}")),
                ),
                Prefix::Gt | Prefix::Ge | Prefix::Eb => {
                    let condition = match p {
                        Prefix::Gt => format!("{end} > {qe}"),
                        Prefix::Ge => format!("{end} > {qs}"),
                        _ => format!("{end} <= {qs}"),
                    };
                    if cx.standalone {
                        scans.indexed(table, cx.rt, &cx.param_var, &[condition]);
                    } else {
                        scans.prefix(table, prefix, Some(condition));
                    }
                }
            }
        }
        SearchValue::Number { prefix: p, value } => {
            match number_range(scans.sql, prefix, *p, *value) {
                Some((lo, hi)) => scans.range(table, &lo, &hi, None),
                None => {
                    let condition = number_condition(scans.sql, &element(n), *p, *value);
                    scans.prefix(table, prefix, Some(condition));
                }
            }
        }
        SearchValue::Quantity {
            prefix: p,
            value,
            system,
            code,
        } => match (system, code) {
            (Some(system), Some(code)) => {
                let (s, c) = (scans.sql.str(system), scans.sql.str(code));
                let unit = with(prefix, &[&s, &c]);
                match number_range(scans.sql, &unit, *p, *value) {
                    Some((lo, hi)) => scans.range(table, &lo, &hi, None),
                    None => {
                        let condition = number_condition(scans.sql, &element(n + 2), *p, *value);
                        scans.prefix(table, &unit, Some(condition));
                    }
                }
            }
            (Some(system), None) => {
                let s = scans.sql.str(system);
                let condition = number_condition(scans.sql, &element(n + 2), *p, *value);
                scans.prefix(table, &with(prefix, &[&s]), Some(condition));
            }
            (None, code) => {
                let x = if cx.standalone {
                    "value".to_owned()
                } else {
                    element(n + 2)
                };
                let mut conditions = Vec::new();
                if let Some(code) = code {
                    let c = scans.sql.str(code);
                    let field = if cx.standalone {
                        "code".to_owned()
                    } else {
                        element(n + 1)
                    };
                    conditions.push(format!("{field} = {c}"));
                }
                conditions.push(number_condition(scans.sql, &x, *p, *value));
                if cx.standalone {
                    scans.indexed(table, cx.rt, &cx.param_var, &conditions);
                } else {
                    scans.prefix(table, prefix, Some(conditions.join(" AND ")));
                }
            }
        },
        SearchValue::Reference {
            target_type,
            target_id,
        } => {
            let id = scans.sql.str(target_id.as_str());
            match target_type {
                Some(target_type) => {
                    let t = scans.sql.str(target_type.as_str());
                    scans.prefix(table, &with(prefix, &[&t, &id]), None);
                }
                None if cx.standalone => {
                    scans.indexed(table, cx.rt, &cx.param_var, &[format!("target_id = {id}")]);
                }
                None => scans.prefix(table, prefix, Some(format!("{} = {id}", element(n + 1)))),
            }
        }
        SearchValue::Uri(uri) => {
            let u = scans.sql.str(uri.as_str());
            scans.prefix(table, &with(prefix, &[&u]), None);
        }
        SearchValue::Composite(components) => {
            let Some((last, leading)) = components.split_last() else {
                return Err(unsupported(cx.param, "an empty composite value"));
            };
            let mut key = prefix.to_vec();
            for component in leading {
                key.extend(exact_parts(scans.sql, cx.param, component)?);
            }
            let inner = Context {
                standalone: false,
                param_var: cx.param_var.clone(),
                ..*cx
            };
            scan_value(scans, &inner, &key, last)?;
        }
    }
    Ok(())
}

fn unsupported(param: &str, reason: &str) -> Unsupported {
    Unsupported {
        param: param.to_owned(),
        reason: reason.to_owned(),
    }
}

/// The key elements of a composite's leading component, which must name its whole key so the
/// last component can be a range after it.
fn exact_parts(
    sql: &mut Sql,
    param: &str,
    value: &SearchValue,
) -> Result<Vec<String>, Unsupported> {
    Ok(match value {
        SearchValue::Token {
            system: Some(system),
            code: Some(code),
        } => vec![sql.str(system), sql.str(code)],
        SearchValue::Reference {
            target_type: Some(t),
            target_id,
        } => vec![sql.str(t.as_str()), sql.str(target_id.as_str())],
        SearchValue::Uri(uri) => vec![sql.str(uri.as_str())],
        _ => {
            return Err(unsupported(
                param,
                "a leading composite component must be a system|code token, a typed reference or a uri",
            ));
        }
    })
}

/// Appends the scans for `clause` on `resource_type` and returns how to combine them.
pub(crate) fn compile(
    sql: &mut Sql,
    resource_type: &str,
    clause: &Clause,
    explain: bool,
) -> Result<ClausePlan, Unsupported> {
    let rt = sql.str(resource_type);
    let param_var = sql.str(clause.param.as_str());
    let cx = Context {
        rt: &rt,
        param_var: param_var.clone(),
        param: &clause.param,
        table: table(clause.param_type),
        standalone: clause.param_type != ParamType::Composite,
    };
    let prefix = vec![rt.clone(), param_var.clone()];
    let mut scans = Scans {
        sql,
        explain,
        out: Vec::new(),
    };
    let union =
        |scans: &mut Scans<'_>, values: &[SearchValue]| -> Result<Vec<usize>, Unsupported> {
            let start = scans.out.len();
            for value in values {
                scan_value(scans, &cx, &prefix, value)?;
            }
            Ok(scans.out[start..].to_vec())
        };
    Ok(match &clause.test {
        Test::Missing(true) => {
            scans.prefix("idx_missing", &prefix, None);
            ClausePlan::Union(scans.out)
        }
        Test::Missing(false) => {
            scans.prefix(cx.table, &prefix, None);
            ClausePlan::Union(scans.out)
        }
        Test::Any(values) => ClausePlan::Union(union(&mut scans, values)?),
        Test::NoneOf(values) => {
            scans.prefix(cx.table, &prefix, None);
            scans.prefix("idx_missing", &prefix, None);
            let universe = scans.out.clone();
            let minus = union(&mut scans, values)?;
            ClausePlan::Except { universe, minus }
        }
    })
}
