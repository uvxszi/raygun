//! Just enough of protobuf's wire format to read a Mihon store (`index.pb`): fields by number,
//! as varints or length-delimited bytes. Groups and fixed-width numbers are skipped.

/// One field of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    /// A fixed 32- or 64-bit number, which no store field uses.
    Fixed,
}

impl<'a> Value<'a> {
    pub fn text(self) -> Option<String> {
        match self {
            Self::Bytes(bytes) => std::str::from_utf8(bytes).ok().map(str::to_owned),
            _ => None,
        }
    }

    pub fn number(self) -> Option<u64> {
        match self {
            Self::Varint(number) => Some(number),
            _ => None,
        }
    }

    pub fn message(self) -> Option<Message<'a>> {
        match self {
            Self::Bytes(bytes) => Message::parse(bytes),
            _ => None,
        }
    }
}

/// A message's fields in order (repeated ones appear once each).
#[derive(Debug, Clone, Default)]
pub(crate) struct Message<'a> {
    fields: Vec<(u32, Value<'a>)>,
}

impl<'a> Message<'a> {
    /// Reads a message; `None` when the bytes aren't one.
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        let mut fields = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let key = varint(bytes, &mut at)?;
            let number = u32::try_from(key >> 3).ok().filter(|number| *number > 0)?;
            let value = match key & 7 {
                0 => Value::Varint(varint(bytes, &mut at)?),
                1 => {
                    at = at.checked_add(8).filter(|end| *end <= bytes.len())?;
                    Value::Fixed
                }
                2 => {
                    let length = usize::try_from(varint(bytes, &mut at)?).ok()?;
                    let end = at.checked_add(length).filter(|end| *end <= bytes.len())?;
                    let value = Value::Bytes(&bytes[at..end]);
                    at = end;
                    value
                }
                5 => {
                    at = at.checked_add(4).filter(|end| *end <= bytes.len())?;
                    Value::Fixed
                }
                _ => return None,
            };
            fields.push((number, value));
        }
        Some(Self { fields })
    }

    /// The last value of field `number` (protobuf's rule for a field given twice).
    pub fn get(&self, number: u32) -> Option<Value<'a>> {
        self.fields
            .iter()
            .rev()
            .find(|(field, _)| *field == number)
            .map(|(_, value)| *value)
    }

    /// Every value of a repeated field.
    pub fn all(&self, number: u32) -> impl Iterator<Item = Value<'a>> + '_ {
        self.fields
            .iter()
            .filter(move |(field, _)| *field == number)
            .map(|(_, value)| *value)
    }

    pub fn text(&self, number: u32) -> Option<String> {
        self.get(number).and_then(Value::text)
    }

    pub fn number(&self, number: u32) -> Option<u64> {
        self.get(number).and_then(Value::number)
    }

    pub fn message(&self, number: u32) -> Option<Message<'a>> {
        self.get(number).and_then(Value::message)
    }
}

fn varint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod write {
    //! Writing messages, for tests.

    pub fn varint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    pub fn number(field: u32, value: u64, out: &mut Vec<u8>) {
        varint(u64::from(field) << 3, out);
        varint(value, out);
    }

    pub fn bytes(field: u32, value: &[u8], out: &mut Vec<u8>) {
        varint((u64::from(field) << 3) | 2, out);
        varint(value.len() as u64, out);
        out.extend_from_slice(value);
    }

    pub fn text(field: u32, value: &str, out: &mut Vec<u8>) {
        bytes(field, value.as_bytes(), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_nested_messages_and_repeats() {
        let mut inner = Vec::new();
        write::number(1, 6_289_731_484_943_315_811, &mut inner);
        write::text(2, "AHottie", &mut inner);
        let mut outer = Vec::new();
        write::text(1, "Keiyoushi", &mut outer);
        write::bytes(8, &inner, &mut outer);
        write::bytes(8, &inner, &mut outer);
        write::number(5, 106_005, &mut outer);
        let message = Message::parse(&outer).unwrap();
        assert_eq!(message.text(1).as_deref(), Some("Keiyoushi"));
        assert_eq!(message.number(5), Some(106_005));
        let sources: Vec<_> = message.all(8).filter_map(Value::message).collect();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].number(1), Some(6_289_731_484_943_315_811));
        assert_eq!(sources[0].text(2).as_deref(), Some("AHottie"));
        assert!(Message::parse(&[0x0a, 0x05, b'a']).is_none(), "cut short");
        assert!(
            Message::parse(b"[{\"pkg\":1}]").is_none(),
            "JSON isn't protobuf"
        );
    }
}
