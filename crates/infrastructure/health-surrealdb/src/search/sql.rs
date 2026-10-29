//! A SurrealQL batch with numbered binds: every value enters a query as a bound parameter,
//! never as text.

use surrealdb::IndexedResults;
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction;

/// A value to bind, with the SurrealQL type it is cast to.
#[derive(Debug, Clone)]
pub(crate) enum Bind {
    Str(String),
    Strs(Vec<String>),
    Int(i64),
    Decimal(String),
    Datetime(String),
}

/// The `{}` sentinel: an object sorts after every string, number and datetime, so it closes
/// a record-id range over a key prefix.
pub(crate) const MAX: &str = "{}";

#[derive(Debug, Default)]
pub(crate) struct Sql {
    statements: Vec<String>,
    binds: Vec<(String, Bind)>,
}

impl Sql {
    /// Binds `value` and returns the parameter that holds it at its SurrealQL type. Decimals
    /// and datetimes are cast once with a `LET`, because record-id keys take parameters but
    /// not casts.
    pub(crate) fn value(&mut self, value: Bind) -> String {
        let name = format!("p{}", self.binds.len());
        let cast = match &value {
            Bind::Decimal(_) => Some("decimal"),
            Bind::Datetime(_) => Some("datetime"),
            Bind::Str(_) | Bind::Strs(_) | Bind::Int(_) => None,
        };
        self.binds.push((name.clone(), value));
        match cast {
            Some(cast) => {
                let variable = format!("v{}", self.binds.len() - 1);
                self.statements
                    .push(format!("LET ${variable} = <{cast}> ${name}"));
                format!("${variable}")
            }
            None => format!("${name}"),
        }
    }

    pub(crate) fn str(&mut self, value: impl Into<String>) -> String {
        self.value(Bind::Str(value.into()))
    }

    /// A fresh variable name for this batch.
    pub(crate) fn variable(&mut self, stem: &str) -> String {
        format!("${stem}{}", self.statements.len())
    }

    /// Appends a statement and returns its result index.
    pub(crate) fn push(&mut self, statement: impl Into<String>) -> usize {
        self.statements.push(statement.into());
        self.statements.len() - 1
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    pub(crate) async fn run(
        self,
        tx: &Transaction<Any>,
    ) -> Result<IndexedResults, surrealdb::Error> {
        let mut text = self.statements.join(";\n");
        text.push(';');
        let mut query = tx.query(text);
        for (name, value) in self.binds {
            query = match value {
                Bind::Str(s) | Bind::Decimal(s) | Bind::Datetime(s) => query.bind((name, s)),
                Bind::Strs(items) => query.bind((name, items)),
                Bind::Int(n) => query.bind((name, n)),
            };
        }
        query.await.and_then(IndexedResults::check)
    }
}
