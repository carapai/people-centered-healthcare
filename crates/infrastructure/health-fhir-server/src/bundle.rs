//! Bundle assembly that embeds stored resources as they are stored (ADR 0051: reads, search and
//! `_history` serve the stored bytes and never re-parse them).

use serde_json::{Map, Value};

/// One Bundle entry.
#[derive(Debug, Default)]
pub(crate) struct Entry {
    pub(crate) full_url: String,
    /// The stored resource JSON, embedded verbatim.
    pub(crate) resource: Option<String>,
    /// Everything else in the entry (`search`, `request`, `response`).
    pub(crate) rest: Map<String, Value>,
}

/// A Bundle of `kind` as JSON text.
pub(crate) fn bundle(
    kind: &str,
    total: Option<usize>,
    links: &[(&str, String)],
    entries: Vec<Entry>,
) -> String {
    let quote = |s: &str| Value::from(s).to_string();
    let mut out = String::from(r#"{"resourceType":"Bundle","type":"#);
    out.push_str(&quote(kind));
    if let Some(total) = total {
        out.push_str(&format!(r#","total":{total}"#));
    }
    if !links.is_empty() {
        out.push_str(r#","link":["#);
        let links: Vec<String> = links
            .iter()
            .map(|(relation, url)| {
                format!(r#"{{"relation":{},"url":{}}}"#, quote(relation), quote(url))
            })
            .collect();
        out.push_str(&links.join(","));
        out.push(']');
    }
    if !entries.is_empty() {
        out.push_str(r#","entry":["#);
        let entries: Vec<String> = entries
            .into_iter()
            .map(|entry| {
                let mut text = format!(r#"{{"fullUrl":{}"#, quote(&entry.full_url));
                if let Some(resource) = entry.resource {
                    text.push_str(r#","resource":"#);
                    text.push_str(&resource);
                }
                for (key, value) in entry.rest {
                    text.push_str(&format!(",{}:{value}", quote(&key)));
                }
                text.push('}');
                text
            })
            .collect();
        out.push_str(&entries.join(","));
        out.push(']');
    }
    out.push('}');
    out
}
