//! Minimal protobuf wire-format reader — just enough to pull the few string
//! fields Synapse needs out of Anki's v18 blobs (card-template q/a formats and
//! media-entry names). This is deliberately *not* a general protobuf library;
//! it avoids a `prost` build dependency for a handful of well-known fields.

pub enum Field<'a> {
    // The varint/fixed payloads are consumed (to advance the cursor) but not
    // surfaced — Synapse only needs length-delimited (string/message) fields.
    Varint,
    Len(&'a [u8]),
    Fixed32,
    Fixed64,
}

/// Streaming reader over a protobuf message body.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn varint(&mut self) -> Option<u64> {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let byte = *self.buf.get(self.pos)?;
            self.pos += 1;
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    /// Next `(field_number, value)`, or `None` at end / on malformed input.
    pub fn next_field(&mut self) -> Option<(u32, Field<'a>)> {
        if self.pos >= self.buf.len() {
            return None;
        }
        let tag = self.varint()?;
        let field = (tag >> 3) as u32;
        let value = match tag & 7 {
            0 => {
                self.varint()?;
                Field::Varint
            }
            2 => {
                let len = self.varint()? as usize;
                let start = self.pos;
                let end = start.checked_add(len)?;
                if end > self.buf.len() {
                    return None;
                }
                self.pos = end;
                Field::Len(&self.buf[start..end])
            }
            5 => {
                self.pos = self.pos.checked_add(4)?;
                Field::Fixed32
            }
            1 => {
                self.pos = self.pos.checked_add(8)?;
                Field::Fixed64
            }
            _ => return None,
        };
        Some((field, value))
    }
}

/// Decode a `CardTemplateConfig`: `q_format` = field 1, `a_format` = field 2.
pub fn template_formats(blob: &[u8]) -> (String, String) {
    let (mut q, mut a) = (String::new(), String::new());
    let mut reader = Reader::new(blob);
    while let Some((field, value)) = reader.next_field() {
        if let Field::Len(bytes) = value {
            match field {
                1 => q = String::from_utf8_lossy(bytes).into_owned(),
                2 => a = String::from_utf8_lossy(bytes).into_owned(),
                _ => {}
            }
        }
    }
    (q, a)
}

/// Decode `MediaEntries { repeated MediaEntry entries = 1 }`, returning each
/// entry's `name` (`MediaEntry.name` = field 1), in order. The order matches
/// the numeric zip-entry names ("0", "1", …).
pub fn media_entry_names(blob: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut reader = Reader::new(blob);
    while let Some((field, value)) = reader.next_field() {
        if field != 1 {
            continue;
        }
        if let Field::Len(entry) = value {
            let mut inner = Reader::new(entry);
            while let Some((inner_field, inner_value)) = inner.next_field() {
                if inner_field == 1 {
                    if let Field::Len(name) = inner_value {
                        names.push(String::from_utf8_lossy(name).into_owned());
                        break;
                    }
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len_field(field: u32, bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![(field << 3 | 2) as u8];
        out.push(bytes.len() as u8); // single-byte length is fine for tests
        out.extend_from_slice(bytes);
        out
    }

    #[test]
    fn decodes_template_formats() {
        let mut blob = len_field(1, b"{{Front}}");
        blob.extend(len_field(2, b"{{Back}}"));
        assert_eq!(
            template_formats(&blob),
            ("{{Front}}".into(), "{{Back}}".into())
        );
    }

    #[test]
    fn decodes_media_entry_names() {
        let entry0 = len_field(1, b"a.png");
        let entry1 = len_field(1, b"b.mp3");
        let mut blob = len_field(1, &entry0);
        blob.extend(len_field(1, &entry1));
        assert_eq!(
            media_entry_names(&blob),
            vec!["a.png".to_string(), "b.mp3".to_string()]
        );
    }
}
