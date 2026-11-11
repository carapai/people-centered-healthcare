//! Sync bodies on the wire: zstd-compressed (synchronization.md § Wire format). A request or
//! response says so with `Content-Encoding: zstd`; a node asks for compressed answers with
//! `Accept-Encoding: zstd`. A request's signature (ADR 0062) covers its content, not its
//! encoding, so the receiver decompresses before it checks the signature.

use std::io::Read as _;

/// The `Content-Encoding` and `Accept-Encoding` token.
pub const ZSTD: &str = "zstd";

/// The largest body a compressed one may expand to: a 4 MiB push batch of clinical JSON
/// compresses well, but a body that would expand past this is refused, not held in memory.
pub const MAX_DECODED: usize = 64 * 1024 * 1024;

/// The compression level: fast, and most of the gain for JSON.
const LEVEL: i32 = 3;

/// Why a body could not be decompressed.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// It is not a zstd frame, or it is cut short.
    #[error("the body is not valid zstd: {0}")]
    Invalid(String),
    /// It expands past [`MAX_DECODED`].
    #[error("the body expands past {MAX_DECODED} bytes")]
    TooLarge,
}

/// `body`, compressed.
pub fn compress(body: &[u8]) -> Vec<u8> {
    // Compressing from memory into memory has no I/O to fail.
    zstd::bulk::compress(body, LEVEL).unwrap_or_else(|_| unreachable!("in-memory zstd"))
}

/// A zstd stream compressed a piece at a time, for an answer written as it is produced (#83):
/// each [`Self::push`] returns the compressed bytes so far, and the concatenation of every push
/// and [`Self::finish`] decompresses as one body.
pub struct StreamEncoder(zstd::stream::write::Encoder<'static, Vec<u8>>);

impl StreamEncoder {
    /// A new stream at the protocol's level.
    pub fn new() -> Self {
        // Writing into memory has no I/O to fail.
        Self(
            zstd::stream::write::Encoder::new(Vec::new(), LEVEL)
                .unwrap_or_else(|_| unreachable!("in-memory zstd")),
        )
    }

    /// Compresses `piece` and returns what is ready to send.
    pub fn push(&mut self, piece: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        self.0
            .write_all(piece)
            .and_then(|()| self.0.flush())
            .unwrap_or_else(|_| unreachable!("in-memory zstd"));
        std::mem::take(self.0.get_mut())
    }

    /// Ends the stream: the last bytes to send.
    pub fn finish(self) -> Vec<u8> {
        self.0
            .finish()
            .unwrap_or_else(|_| unreachable!("in-memory zstd"))
    }
}

impl Default for StreamEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// `body`, decompressed, refusing more than [`MAX_DECODED`] bytes.
pub fn decompress(body: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let decoder =
        zstd::stream::read::Decoder::new(body).map_err(|e| DecodeError::Invalid(e.to_string()))?;
    let mut out = Vec::new();
    decoder
        .take(MAX_DECODED as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| DecodeError::Invalid(e.to_string()))?;
    if out.len() > MAX_DECODED {
        return Err(DecodeError::TooLarge);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_round_trips_and_shrinks() {
        let body = br#"{"resourceType":"Observation","status":"final"}"#.repeat(200);
        let wire = compress(&body);
        assert!(
            wire.len() < body.len() / 10,
            "{} of {}",
            wire.len(),
            body.len()
        );
        assert_eq!(decompress(&wire).expect("decompress"), body);
        assert_eq!(decompress(&compress(b"")).expect("empty"), b"");
    }

    #[test]
    fn a_stream_compressed_in_pieces_decompresses_as_one_body() {
        let pieces: Vec<Vec<u8>> = (0..50)
            .map(|n| format!(r#"{{"n":{n},"resourceType":"Observation"}},"#).into_bytes())
            .collect();
        let mut encoder = StreamEncoder::new();
        let mut wire = Vec::new();
        for piece in &pieces {
            wire.extend(encoder.push(piece));
        }
        wire.extend(encoder.finish());
        assert_eq!(decompress(&wire).expect("decompress"), pieces.concat());
    }

    #[test]
    fn garbage_and_bombs_are_refused() {
        assert!(matches!(
            decompress(b"not zstd at all"),
            Err(DecodeError::Invalid(_))
        ));
        let wire = compress(&vec![0_u8; MAX_DECODED + 1]);
        assert!(wire.len() < 64 * 1024, "a bomb is small on the wire");
        assert!(matches!(decompress(&wire), Err(DecodeError::TooLarge)));
    }
}
