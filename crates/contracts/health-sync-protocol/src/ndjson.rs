use serde::Serialize;
use serde::de::DeserializeOwned;

/// A body line that is not the expected JSON.
#[derive(Debug, thiserror::Error)]
#[error("line {line}: {source}")]
pub struct NdjsonError {
    /// The 1-based line number.
    pub line: usize,
    /// What went wrong.
    #[source]
    pub source: serde_json::Error,
}

/// One JSON value per line, each line ending in `\n`.
pub fn to_ndjson<T: Serialize>(values: &[T]) -> Result<Vec<u8>, serde_json::Error> {
    let mut out = Vec::new();
    for value in values {
        serde_json::to_writer(&mut out, value)?;
        out.push(b'\n');
    }
    Ok(out)
}

/// The values of an NDJSON body; blank lines are skipped.
pub fn from_ndjson<T: DeserializeOwned>(body: &[u8]) -> Result<Vec<T>, NdjsonError> {
    body.split(|b| *b == b'\n')
        .enumerate()
        .filter(|(_, line)| !line.iter().all(u8::is_ascii_whitespace))
        .map(|(i, line)| {
            serde_json::from_slice(line).map_err(|source| NdjsonError {
                line: i + 1,
                source,
            })
        })
        .collect()
}
