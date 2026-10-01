//! `serde_json::Value` <-> `aws_smithy_types::Document` / `Blob` conversion.
//!
//! Documents carry tool inputs, tool result JSON, tool input schemas and
//! `additionalModelRequestFields` / `additionalModelResponseFields`.
//!
//! Blobs (image/document/video bytes, redacted reasoning) need care to stay wire-compatible with
//! the renderer:
//!
//! * **Output**: the JS SDK yields blobs as `Uint8Array`, and the Express route wrote them with
//!   `JSON.stringify`, which renders a typed array as an index-keyed object
//!   (`{"0":137,"1":80,...}`). [`blob_to_json`] reproduces that exact shape so chat history and the
//!   renderer's `reconstructUint8Array` keep working unchanged.
//! * **Input**: the renderer sends bytes as that same index-keyed object, as a base64 string
//!   (e.g. `bytes: image.base64`), as a plain number array, or as Node's `Buffer` JSON
//!   (`{"type":"Buffer","data":[...]}`). [`blob_from_json`] accepts all of them.

use aws_smithy_types::{Blob, Document, Number};
use base64::Engine;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Convert JSON to a smithy `Document`.
///
/// Integers map to `PosInt`/`NegInt`, everything else numeric to `Float`.
pub fn json_to_document(value: &Value) -> Document {
    match value {
        Value::Null => Document::Null,
        Value::Bool(b) => Document::Bool(*b),
        Value::String(s) => Document::String(s.clone()),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Document::Number(Number::PosInt(u))
            } else if let Some(i) = n.as_i64() {
                Document::Number(Number::NegInt(i))
            } else {
                Document::Number(Number::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        Value::Array(items) => Document::Array(items.iter().map(json_to_document).collect()),
        Value::Object(map) => Document::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_document(v)))
                .collect::<HashMap<_, _>>(),
        ),
    }
}

/// Convert a smithy `Document` to JSON.
///
/// Object keys are sorted (the SDK stores them in a `HashMap`) so output is deterministic.
/// Non-finite floats become `null`, matching `JSON.stringify`.
pub fn document_to_json(doc: &Document) -> Value {
    match doc {
        Document::Null => Value::Null,
        Document::Bool(b) => Value::Bool(*b),
        Document::String(s) => Value::String(s.clone()),
        Document::Number(Number::PosInt(u)) => Value::from(*u),
        Document::Number(Number::NegInt(i)) => Value::from(*i),
        Document::Number(Number::Float(f)) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Document::Array(items) => Value::Array(items.iter().map(document_to_json).collect()),
        Document::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), document_to_json(&map[k]));
            }
            Value::Object(out)
        }
    }
}

/// Render bytes the way `JSON.stringify(new Uint8Array(...))` does: `{"0":b0,"1":b1,...}`.
pub fn blob_to_json(blob: &Blob) -> Value {
    bytes_to_index_object(blob.as_ref())
}

/// See [`blob_to_json`].
pub fn bytes_to_index_object(bytes: &[u8]) -> Value {
    let mut out = Map::new();
    for (i, b) in bytes.iter().enumerate() {
        out.insert(i.to_string(), Value::from(*b));
    }
    Value::Object(out)
}

/// How a JSON *string* in a blob position is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringBlob {
    /// Base64 text (what `processImageContent` does for image bytes).
    Base64,
    /// Raw UTF-8 text (what the JS SDK's `toBase64(string)` does for any other blob member).
    Utf8,
}

/// Decode a blob from any of the JSON shapes the renderer sends. Returns `None` if the value is
/// not a recognizable byte container.
pub fn blob_from_json(value: &Value, strings: StringBlob) -> Option<Blob> {
    match value {
        Value::String(s) => match strings {
            StringBlob::Base64 => base64::engine::general_purpose::STANDARD
                .decode(s.trim())
                .ok()
                .map(Blob::new),
            StringBlob::Utf8 => Some(Blob::new(s.as_bytes().to_vec())),
        },
        Value::Array(items) => number_array(items).map(Blob::new),
        Value::Object(map) => {
            // Node Buffer JSON: { type: 'Buffer', data: [...] }
            if map.get("type").and_then(Value::as_str) == Some("Buffer") {
                if let Some(Value::Array(items)) = map.get("data") {
                    return number_array(items).map(Blob::new);
                }
            }
            // Serialized Uint8Array: every key numeric (reconstructUint8Array).
            let mut pairs = Vec::with_capacity(map.len());
            for (k, v) in map {
                let idx: usize = k.parse().ok()?;
                let byte = u8::try_from(v.as_u64()?).ok()?;
                pairs.push((idx, byte));
            }
            pairs.sort_by_key(|(i, _)| *i);
            Some(Blob::new(
                pairs.into_iter().map(|(_, b)| b).collect::<Vec<u8>>(),
            ))
        }
        _ => None,
    }
}

fn number_array(items: &[Value]) -> Option<Vec<u8>> {
    items
        .iter()
        .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn document_round_trips_nested_json() {
        let v = json!({
            "path": "/tmp/a.txt",
            "options": { "recursive": true, "depth": 3, "neg": -2, "ratio": 0.25, "none": null },
            "list": ["a", 1, false, [], {}]
        });
        let doc = json_to_document(&v);
        assert_eq!(document_to_json(&doc), v);
    }

    #[test]
    fn number_variants() {
        assert_eq!(
            json_to_document(&json!(5)),
            Document::Number(Number::PosInt(5))
        );
        assert_eq!(
            json_to_document(&json!(-5)),
            Document::Number(Number::NegInt(-5))
        );
        assert_eq!(
            json_to_document(&json!(1.5)),
            Document::Number(Number::Float(1.5))
        );
        assert_eq!(
            document_to_json(&Document::Number(Number::Float(f64::NAN))),
            Value::Null
        );
    }

    #[test]
    fn document_object_keys_sorted() {
        let doc = json_to_document(&json!({ "b": 1, "a": 2, "c": 3 }));
        let out = serde_json::to_string(&document_to_json(&doc)).unwrap();
        assert_eq!(out, r#"{"a":2,"b":1,"c":3}"#);
    }

    #[test]
    fn blob_output_is_uint8array_json() {
        let out = blob_to_json(&Blob::new(vec![137u8, 80, 78]));
        assert_eq!(
            serde_json::to_string(&out).unwrap(),
            r#"{"0":137,"1":80,"2":78}"#
        );
        assert_eq!(blob_to_json(&Blob::new(Vec::<u8>::new())), json!({}));
    }

    #[test]
    fn blob_input_shapes() {
        let expected = Blob::new(vec![1u8, 2, 255]);
        // Serialized Uint8Array (keys possibly out of order)
        assert_eq!(
            blob_from_json(&json!({ "1": 2, "0": 1, "2": 255 }), StringBlob::Base64),
            Some(expected.clone())
        );
        // base64
        assert_eq!(
            blob_from_json(&json!("AQL/"), StringBlob::Base64),
            Some(expected.clone())
        );
        // number array
        assert_eq!(
            blob_from_json(&json!([1, 2, 255]), StringBlob::Base64),
            Some(expected.clone())
        );
        // Node Buffer JSON
        assert_eq!(
            blob_from_json(
                &json!({ "type": "Buffer", "data": [1, 2, 255] }),
                StringBlob::Base64
            ),
            Some(expected)
        );
        // utf8 strings for non-image members
        assert_eq!(
            blob_from_json(&json!("hi"), StringBlob::Utf8),
            Some(Blob::new(b"hi".to_vec()))
        );
    }

    #[test]
    fn blob_input_rejects_garbage() {
        assert_eq!(blob_from_json(&json!({ "x": 1 }), StringBlob::Base64), None);
        assert_eq!(blob_from_json(&json!([1, 256]), StringBlob::Base64), None);
        assert_eq!(
            blob_from_json(&json!("not base64!!"), StringBlob::Base64),
            None
        );
        assert_eq!(blob_from_json(&json!(true), StringBlob::Base64), None);
    }

    #[test]
    fn blob_round_trip_through_index_object() {
        let bytes: Vec<u8> = (0..=255).collect();
        let json = blob_to_json(&Blob::new(bytes.clone()));
        assert_eq!(
            blob_from_json(&json, StringBlob::Base64),
            Some(Blob::new(bytes))
        );
    }
}
