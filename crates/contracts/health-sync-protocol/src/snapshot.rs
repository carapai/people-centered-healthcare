//! Reading a snapshot answer as it arrives (#83): `{"cursor":…,"items":[…]}`, handed out as the
//! cursor and batches of items, so a node holds one batch of a re-seed, never the whole answer.

use std::fmt;
use std::io::Read;

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::{Cursor, Item};

/// What [`read_snapshot`] hands out, in the order the answer carries them.
#[derive(Debug)]
pub enum SnapshotEvent {
    /// The upstream's cursor as of the snapshot.
    Cursor(Cursor),
    /// A batch of heads.
    Items(Vec<Item>),
}

/// Reads a snapshot answer from `reader`, calling `on` with the cursor and with batches of at
/// most `batch` items. An answer that is cut short, or is not a snapshot, is an error, so a
/// partial answer is never taken for a whole one. An error from `on` stops the read.
pub fn read_snapshot<R, F>(reader: R, batch: usize, mut on: F) -> Result<(), String>
where
    R: Read,
    F: FnMut(SnapshotEvent) -> Result<(), String>,
{
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    Answer {
        batch: batch.max(1),
        on: &mut on,
    }
    .deserialize(&mut deserializer)
    .map_err(|e| format!("the snapshot: {e}"))?;
    deserializer.end().map_err(|e| format!("the snapshot: {e}"))
}

/// A zstd-compressed `reader`, decompressed as it is read.
pub fn decoder<'a, R: Read + 'a>(reader: R) -> Result<Box<dyn Read + 'a>, String> {
    zstd::stream::read::Decoder::new(reader)
        .map(|decoder| Box::new(decoder) as Box<dyn Read + 'a>)
        .map_err(|e| format!("zstd: {e}"))
}

struct Answer<'f, F> {
    batch: usize,
    on: &'f mut F,
}

impl<'de, F> DeserializeSeed<'de> for Answer<'_, F>
where
    F: FnMut(SnapshotEvent) -> Result<(), String>,
{
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de, F> Visitor<'de> for Answer<'_, F>
where
    F: FnMut(SnapshotEvent) -> Result<(), String>,
{
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a snapshot: {\"cursor\": …, \"items\": […]}")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut cursor = false;
        let mut items = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "cursor" => {
                    let at: Cursor = map.next_value()?;
                    (self.on)(SnapshotEvent::Cursor(at)).map_err(de::Error::custom)?;
                    cursor = true;
                }
                "items" => {
                    map.next_value_seed(Items {
                        batch: self.batch,
                        on: &mut *self.on,
                    })?;
                    items = true;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        if !cursor {
            return Err(de::Error::missing_field("cursor"));
        }
        if !items {
            return Err(de::Error::missing_field("items"));
        }
        Ok(())
    }
}

struct Items<'f, F> {
    batch: usize,
    on: &'f mut F,
}

impl<'de, F> DeserializeSeed<'de> for Items<'_, F>
where
    F: FnMut(SnapshotEvent) -> Result<(), String>,
{
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, F> Visitor<'de> for Items<'_, F>
where
    F: FnMut(SnapshotEvent) -> Result<(), String>,
{
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the snapshot's items")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let mut page = Vec::with_capacity(self.batch);
        while let Some(item) = seq.next_element::<Item>()? {
            page.push(item);
            if page.len() == self.batch {
                (self.on)(SnapshotEvent::Items(std::mem::take(&mut page)))
                    .map_err(de::Error::custom)?;
            }
        }
        if !page.is_empty() {
            (self.on)(SnapshotEvent::Items(page)).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ContentHash, Hlc, Kind, StreamEncoder};

    fn item(n: usize) -> Item {
        let body = json!({"resourceType": "Observation", "status": "final", "valueInteger": n});
        let op_id = health_identifiers::OpId::new();
        Item {
            kind: Kind::Version,
            op_id,
            tx_id: op_id,
            resource_type: "Observation".into(),
            resource_id: health_identifiers::ResourceId::new().to_string(),
            parent_version_id: None,
            origin_node: health_identifiers::NodeId::new(),
            device_id: None,
            author: None,
            hlc: Hlc::default(),
            content_hash: ContentHash::of(&body),
            packages: Vec::new(),
            snapshot_version: 0,
            provisional: false,
            deleted: false,
            blobs: Vec::new(),
            body,
        }
    }

    fn answer(items: &[Item]) -> (Cursor, Vec<u8>) {
        let cursor = Cursor {
            node: health_identifiers::NodeId::new(),
            epoch: "e1".into(),
            seq: 42,
        };
        let body = serde_json::to_vec(&json!({"cursor": cursor, "items": items})).expect("json");
        (cursor, body)
    }

    fn read(reader: impl Read, batch: usize) -> Result<(Option<Cursor>, Vec<usize>), String> {
        let mut cursor = None;
        let mut batches = Vec::new();
        read_snapshot(reader, batch, |event| {
            match event {
                SnapshotEvent::Cursor(at) => cursor = Some(at),
                SnapshotEvent::Items(page) => batches.push(page.len()),
            }
            Ok(())
        })?;
        Ok((cursor, batches))
    }

    #[test]
    fn an_answer_is_handed_out_as_its_cursor_and_batches() {
        let items: Vec<Item> = (0..25).map(item).collect();
        let (cursor, body) = answer(&items);
        let (read_cursor, batches) = read(body.as_slice(), 10).expect("read");
        assert_eq!(read_cursor, Some(cursor));
        assert_eq!(batches, vec![10, 10, 5]);
    }

    #[test]
    fn a_compressed_stream_reads_the_same() {
        let items: Vec<Item> = (0..7).map(item).collect();
        let (cursor, body) = answer(&items);
        let mut encoder = StreamEncoder::new();
        let mut wire = Vec::new();
        for piece in body.chunks(100) {
            wire.extend(encoder.push(piece));
        }
        wire.extend(encoder.finish());
        let (read_cursor, batches) =
            read(decoder(wire.as_slice()).expect("decoder"), 3).expect("read");
        assert_eq!(read_cursor, Some(cursor));
        assert_eq!(batches, vec![3, 3, 1]);
    }

    #[test]
    fn a_cut_short_or_foreign_answer_is_an_error() {
        let items: Vec<Item> = (0..5).map(item).collect();
        let (_, body) = answer(&items);
        assert!(read(&body[..body.len() - 10], 2).is_err(), "cut short");
        assert!(read(&b"{\"items\":[]}"[..], 2).is_err(), "no cursor");
        assert!(read(&b"[]"[..], 2).is_err(), "not a snapshot");
        // A handler that refuses stops the read.
        let refused = read_snapshot(body.as_slice(), 2, |_| Err("stop".into()));
        assert!(refused.is_err_and(|e| e.contains("stop")));
    }
}
