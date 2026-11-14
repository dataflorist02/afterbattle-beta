// The binary layout implemented here was independently described from hedger/BWXML
// (Apache-2.0, Copyright 2013-2014 hedger). No source code was copied.

use thiserror::Error;

pub(crate) const PACKED_SECTION_MAGIC: u32 = 0x62A1_4E45;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PackedLimits {
    pub max_file_size: usize,
    pub max_strings: usize,
    pub max_string_length: usize,
    pub max_children_per_section: usize,
    pub max_total_nodes: usize,
    pub max_depth: usize,
    pub max_blob_size: usize,
}

impl Default for PackedLimits {
    fn default() -> Self {
        Self {
            max_file_size: 32 * 1024 * 1024,
            max_strings: u16::MAX as usize + 1,
            max_string_length: 16 * 1024,
            max_children_per_section: 32 * 1024,
            max_total_nodes: 500_000,
            max_depth: 64,
            max_blob_size: 4 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PackedDocument {
    pub version: u8,
    pub string_count: usize,
    pub root: PackedSection,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PackedSection {
    pub value: PackedValue,
    pub children: Vec<PackedNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PackedNode {
    pub name: String,
    pub value: PackedValue,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PackedValue {
    Section(Box<PackedSection>),
    String(String),
    Integer(i64),
    Floats(Vec<f32>),
    Bool(bool),
    Blob(Vec<u8>),
}

impl PackedValue {
    pub(crate) fn as_section(&self) -> Option<&PackedSection> {
        match self {
            Self::Section(section) => Some(section),
            _ => None,
        }
    }

    pub(crate) fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum PackedError {
    #[error("packed input exceeds the configured size limit")]
    InputTooLarge,
    #[error("packed input is truncated at {0}")]
    Truncated(&'static str),
    #[error("packed input has an invalid magic")]
    InvalidMagic,
    #[error("packed input version {0} is unsupported")]
    UnsupportedVersion(u8),
    #[error("packed string table has no terminating empty string")]
    MissingStringTableTerminator,
    #[error("packed string table exceeds the configured entry limit")]
    TooManyStrings,
    #[error("packed string exceeds the configured length limit")]
    StringTooLong,
    #[error("packed string is not valid UTF-8")]
    InvalidUtf8,
    #[error("packed section exceeds the configured child limit")]
    TooManyChildren,
    #[error("packed document exceeds the configured node limit")]
    TooManyNodes,
    #[error("packed document exceeds the configured recursion limit")]
    DepthExceeded,
    #[error("packed section offsets are decreasing")]
    OffsetDecreasing,
    #[error("packed section offset is outside its section")]
    OffsetOutOfBounds,
    #[error("packed section offset arithmetic overflowed")]
    OffsetOverflow,
    #[error("packed node name index is outside the string table")]
    NameIndexOutOfBounds,
    #[error("packed data type {0} is unsupported")]
    UnknownType(u8),
    #[error("encrypted packed blobs are unsupported")]
    EncryptedBlob,
    #[error("packed integer has unsupported byte length {0}")]
    UnsupportedIntegerSize(usize),
    #[error("packed float data is not a sequence of 32-bit values")]
    InvalidFloatSize,
    #[error("packed blob exceeds the configured size limit")]
    BlobTooLarge,
    #[error("packed section has trailing data outside its descriptors")]
    TrailingData,
}

pub(crate) fn parse_packed_document(bytes: &[u8]) -> Result<PackedDocument, PackedError> {
    parse_packed_document_with_limits(bytes, PackedLimits::default())
}

fn parse_packed_document_with_limits(
    bytes: &[u8],
    limits: PackedLimits,
) -> Result<PackedDocument, PackedError> {
    if bytes.len() > limits.max_file_size {
        return Err(PackedError::InputTooLarge);
    }
    let magic = read_u32(bytes, 0, "magic")?;
    if magic != PACKED_SECTION_MAGIC {
        return Err(PackedError::InvalidMagic);
    }
    let version = *bytes.get(4).ok_or(PackedError::Truncated("version"))?;
    if version != 0 {
        return Err(PackedError::UnsupportedVersion(version));
    }

    let (strings, section_start) = parse_string_table(bytes, 5, limits)?;
    let section_bytes = bytes
        .get(section_start..)
        .ok_or(PackedError::Truncated("root section"))?;
    let mut state = ParseState { limits, nodes: 0 };
    let root = parse_section(section_bytes, &strings, 0, &mut state)?;
    Ok(PackedDocument {
        version,
        string_count: strings.len(),
        root,
    })
}

fn parse_string_table(
    bytes: &[u8],
    mut cursor: usize,
    limits: PackedLimits,
) -> Result<(Vec<String>, usize), PackedError> {
    let mut strings = Vec::new();
    loop {
        let remaining = bytes
            .get(cursor..)
            .ok_or(PackedError::MissingStringTableTerminator)?;
        let length = remaining
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(PackedError::MissingStringTableTerminator)?;
        if length > limits.max_string_length {
            return Err(PackedError::StringTooLong);
        }
        cursor = cursor
            .checked_add(length)
            .and_then(|value| value.checked_add(1))
            .ok_or(PackedError::OffsetOverflow)?;
        if length == 0 {
            return Ok((strings, cursor));
        }
        if strings.len() >= limits.max_strings {
            return Err(PackedError::TooManyStrings);
        }
        let start = cursor
            .checked_sub(length + 1)
            .ok_or(PackedError::OffsetOverflow)?;
        let end = start
            .checked_add(length)
            .ok_or(PackedError::OffsetOverflow)?;
        let raw = bytes
            .get(start..end)
            .ok_or(PackedError::MissingStringTableTerminator)?;
        let value = std::str::from_utf8(raw).map_err(|_| PackedError::InvalidUtf8)?;
        strings.push(value.to_owned());
    }
}

#[derive(Debug, Clone, Copy)]
struct Descriptor {
    data_type: u8,
    offset: usize,
}

struct ParseState {
    limits: PackedLimits,
    nodes: usize,
}

fn parse_section(
    bytes: &[u8],
    strings: &[String],
    depth: usize,
    state: &mut ParseState,
) -> Result<PackedSection, PackedError> {
    if depth > state.limits.max_depth {
        return Err(PackedError::DepthExceeded);
    }
    let child_count = usize::from(read_u16(bytes, 0, "section child count")?);
    if child_count > state.limits.max_children_per_section {
        return Err(PackedError::TooManyChildren);
    }
    let child_table_size = child_count
        .checked_mul(6)
        .ok_or(PackedError::OffsetOverflow)?;
    let data_start = 6usize
        .checked_add(child_table_size)
        .ok_or(PackedError::OffsetOverflow)?;
    if data_start > bytes.len() {
        return Err(PackedError::Truncated("section descriptors"));
    }

    let own = read_descriptor(bytes, 2)?;
    let mut children = Vec::with_capacity(child_count);
    for index in 0..child_count {
        let descriptor_start = 6usize
            .checked_add(index.checked_mul(6).ok_or(PackedError::OffsetOverflow)?)
            .ok_or(PackedError::OffsetOverflow)?;
        let name_index = usize::from(read_u16(bytes, descriptor_start, "node name index")?);
        let name = strings
            .get(name_index)
            .ok_or(PackedError::NameIndexOutOfBounds)?
            .clone();
        let descriptor_offset = descriptor_start
            .checked_add(2)
            .ok_or(PackedError::OffsetOverflow)?;
        let descriptor = read_descriptor(bytes, descriptor_offset)?;
        children.push((name, descriptor));
    }

    let mut previous = 0usize;
    let own_bytes = descriptor_slice(bytes, data_start, previous, own.offset)?;
    previous = own.offset;
    let value = parse_value(own.data_type, own_bytes, strings, depth, state)?;
    let mut parsed_children = Vec::with_capacity(children.len());
    for (name, descriptor) in children {
        let value_bytes = descriptor_slice(bytes, data_start, previous, descriptor.offset)?;
        previous = descriptor.offset;
        let value = parse_value(descriptor.data_type, value_bytes, strings, depth, state)?;
        parsed_children.push(PackedNode { name, value });
    }
    let described_end = resolve_end(data_start, previous, bytes.len())?;
    if described_end != bytes.len() {
        return Err(PackedError::TrailingData);
    }

    Ok(PackedSection {
        value,
        children: parsed_children,
    })
}

fn parse_value(
    data_type: u8,
    bytes: &[u8],
    strings: &[String],
    depth: usize,
    state: &mut ParseState,
) -> Result<PackedValue, PackedError> {
    state.nodes = state
        .nodes
        .checked_add(1)
        .ok_or(PackedError::TooManyNodes)?;
    if state.nodes > state.limits.max_total_nodes {
        return Err(PackedError::TooManyNodes);
    }
    match data_type {
        0 => {
            let nested_depth = depth.checked_add(1).ok_or(PackedError::DepthExceeded)?;
            Ok(PackedValue::Section(Box::new(parse_section(
                bytes,
                strings,
                nested_depth,
                state,
            )?)))
        }
        1 => {
            if bytes.len() > state.limits.max_string_length {
                return Err(PackedError::StringTooLong);
            }
            let value = std::str::from_utf8(bytes).map_err(|_| PackedError::InvalidUtf8)?;
            Ok(PackedValue::String(value.to_owned()))
        }
        2 => parse_integer(bytes).map(PackedValue::Integer),
        3 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(PackedError::InvalidFloatSize);
            }
            let values = bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok(PackedValue::Floats(values))
        }
        4 => Ok(PackedValue::Bool(!bytes.is_empty())),
        5 => {
            if bytes.len() > state.limits.max_blob_size {
                return Err(PackedError::BlobTooLarge);
            }
            Ok(PackedValue::Blob(bytes.to_vec()))
        }
        6 => Err(PackedError::EncryptedBlob),
        other => Err(PackedError::UnknownType(other)),
    }
}

fn parse_integer(bytes: &[u8]) -> Result<i64, PackedError> {
    match bytes.len() {
        0 => Ok(0),
        1 => Ok(i64::from(i8::from_le_bytes([bytes[0]]))),
        2 => Ok(i64::from(i16::from_le_bytes([bytes[0], bytes[1]]))),
        4 => Ok(i64::from(i32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))),
        8 => Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        size => Err(PackedError::UnsupportedIntegerSize(size)),
    }
}

fn read_descriptor(bytes: &[u8], start: usize) -> Result<Descriptor, PackedError> {
    let raw = read_u32(bytes, start, "data descriptor")?;
    Ok(Descriptor {
        data_type: (raw >> 28) as u8,
        offset: (raw & 0x0FFF_FFFF) as usize,
    })
}

fn descriptor_slice(
    bytes: &[u8],
    data_start: usize,
    previous: usize,
    current: usize,
) -> Result<&[u8], PackedError> {
    if current < previous {
        return Err(PackedError::OffsetDecreasing);
    }
    let start = resolve_end(data_start, previous, bytes.len())?;
    let end = resolve_end(data_start, current, bytes.len())?;
    bytes.get(start..end).ok_or(PackedError::OffsetOutOfBounds)
}

fn resolve_end(data_start: usize, offset: usize, section_end: usize) -> Result<usize, PackedError> {
    let end = data_start
        .checked_add(offset)
        .ok_or(PackedError::OffsetOverflow)?;
    if end > section_end {
        return Err(PackedError::OffsetOutOfBounds);
    }
    Ok(end)
}

fn read_u16(bytes: &[u8], start: usize, stage: &'static str) -> Result<u16, PackedError> {
    let raw = bytes
        .get(start..start.checked_add(2).ok_or(PackedError::OffsetOverflow)?)
        .ok_or(PackedError::Truncated(stage))?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], start: usize, stage: &'static str) -> Result<u32, PackedError> {
    let raw = bytes
        .get(start..start.checked_add(4).ok_or(PackedError::OffsetOverflow)?)
        .ok_or(PackedError::Truncated(stage))?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    enum FixtureValue {
        Section(Box<FixtureSection>),
        String(Vec<u8>),
        Integer(Vec<u8>),
        Floats(Vec<f32>),
        Bool(bool),
        Blob(Vec<u8>),
        Encrypted(Vec<u8>),
        Unknown(u8, Vec<u8>),
    }

    #[derive(Clone)]
    struct FixtureSection {
        own: FixtureValue,
        children: Vec<(u16, FixtureValue)>,
    }

    fn empty_section() -> FixtureSection {
        FixtureSection {
            own: FixtureValue::String(Vec::new()),
            children: Vec::new(),
        }
    }

    fn encode_value(value: &FixtureValue) -> (u8, Vec<u8>) {
        match value {
            FixtureValue::Section(section) => (0, encode_section(section)),
            FixtureValue::String(value) => (1, value.clone()),
            FixtureValue::Integer(value) => (2, value.clone()),
            FixtureValue::Floats(values) => (
                3,
                values
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect(),
            ),
            FixtureValue::Bool(false) => (4, Vec::new()),
            FixtureValue::Bool(true) => (4, vec![1]),
            FixtureValue::Blob(value) => (5, value.clone()),
            FixtureValue::Encrypted(value) => (6, value.clone()),
            FixtureValue::Unknown(kind, value) => (*kind, value.clone()),
        }
    }

    fn descriptor(kind: u8, offset: usize) -> [u8; 4] {
        (((u32::from(kind)) << 28) | u32::try_from(offset).unwrap()).to_le_bytes()
    }

    fn encode_section(section: &FixtureSection) -> Vec<u8> {
        let (own_kind, own) = encode_value(&section.own);
        let children: Vec<_> = section
            .children
            .iter()
            .map(|(name, value)| (*name, encode_value(value)))
            .collect();
        let mut output = Vec::new();
        output.extend_from_slice(&(children.len() as u16).to_le_bytes());
        let mut offset = own.len();
        output.extend_from_slice(&descriptor(own_kind, offset));
        for (name, (kind, value)) in &children {
            offset += value.len();
            output.extend_from_slice(&name.to_le_bytes());
            output.extend_from_slice(&descriptor(*kind, offset));
        }
        output.extend_from_slice(&own);
        for (_, (_, value)) in children {
            output.extend_from_slice(&value);
        }
        output
    }

    fn document(strings: &[&[u8]], section: &FixtureSection) -> Vec<u8> {
        let mut output = PACKED_SECTION_MAGIC.to_le_bytes().to_vec();
        output.push(0);
        for value in strings {
            output.extend_from_slice(value);
            output.push(0);
        }
        output.push(0);
        output.extend_from_slice(&encode_section(section));
        output
    }

    fn one_child(value: FixtureValue) -> Vec<u8> {
        document(
            &[b"child"],
            &FixtureSection {
                own: FixtureValue::String(Vec::new()),
                children: vec![(0, value)],
            },
        )
    }

    #[test]
    fn accepts_magic_version_and_empty_section() {
        let parsed = parse_packed_document(&document(&[], &empty_section())).unwrap();
        assert_eq!(parsed.version, 0);
        assert_eq!(parsed.string_count, 0);
        assert!(parsed.root.children.is_empty());
    }

    #[test]
    fn rejects_invalid_magic_and_version() {
        let mut invalid_magic = document(&[], &empty_section());
        invalid_magic[0] ^= 0xFF;
        assert_eq!(
            parse_packed_document(&invalid_magic),
            Err(PackedError::InvalidMagic)
        );
        let mut invalid_version = document(&[], &empty_section());
        invalid_version[4] = 1;
        assert_eq!(
            parse_packed_document(&invalid_version),
            Err(PackedError::UnsupportedVersion(1))
        );
    }

    #[test]
    fn parses_string_table_and_rejects_missing_terminator_or_utf8() {
        let parsed = parse_packed_document(&document(&[b"one", b"two"], &empty_section())).unwrap();
        assert_eq!(parsed.string_count, 2);

        let mut missing = PACKED_SECTION_MAGIC.to_le_bytes().to_vec();
        missing.push(0);
        missing.extend_from_slice(b"unterminated");
        assert_eq!(
            parse_packed_document(&missing),
            Err(PackedError::MissingStringTableTerminator)
        );
        assert_eq!(
            parse_packed_document(&document(&[&[0xFF]], &empty_section())),
            Err(PackedError::InvalidUtf8)
        );
    }

    #[test]
    fn parses_children_nested_sections_and_primitives() {
        let nested = FixtureSection {
            own: FixtureValue::String(b"nested-value".to_vec()),
            children: vec![(1, FixtureValue::Integer((-7_i8).to_le_bytes().to_vec()))],
        };
        let root = FixtureSection {
            own: FixtureValue::String(Vec::new()),
            children: vec![
                (0, FixtureValue::Section(Box::new(nested))),
                (2, FixtureValue::String(b"text".to_vec())),
                (3, FixtureValue::Floats(vec![1.5, -2.0])),
                (4, FixtureValue::Bool(false)),
                (5, FixtureValue::Bool(true)),
                (6, FixtureValue::Blob(vec![1, 2, 3])),
            ],
        };
        let parsed = parse_packed_document(&document(
            &[
                b"nested", b"number", b"text", b"float", b"false", b"true", b"blob",
            ],
            &root,
        ))
        .unwrap();
        assert_eq!(
            parsed.root.children[2].value,
            PackedValue::Floats(vec![1.5, -2.0])
        );
        assert_eq!(parsed.root.children[3].value, PackedValue::Bool(false));
        assert_eq!(parsed.root.children[4].value, PackedValue::Bool(true));
        assert_eq!(
            parsed.root.children[5].value,
            PackedValue::Blob(vec![1, 2, 3])
        );
        let nested = parsed.root.children[0].value.as_section().unwrap();
        assert_eq!(nested.value.as_string(), Some("nested-value"));
        assert_eq!(nested.children[0].value.as_integer(), Some(-7));
    }

    #[test]
    fn parses_all_supported_integer_widths() {
        let cases = [
            (Vec::new(), 0),
            ((-5_i8).to_le_bytes().to_vec(), -5),
            ((-500_i16).to_le_bytes().to_vec(), -500),
            ((-50_000_i32).to_le_bytes().to_vec(), -50_000),
            ((-5_000_000_000_i64).to_le_bytes().to_vec(), -5_000_000_000),
        ];
        for (bytes, expected) in cases {
            let parsed = parse_packed_document(&one_child(FixtureValue::Integer(bytes))).unwrap();
            assert_eq!(parsed.root.children[0].value.as_integer(), Some(expected));
        }
    }

    #[test]
    fn rejects_unknown_and_encrypted_types() {
        assert_eq!(
            parse_packed_document(&one_child(FixtureValue::Unknown(7, vec![]))),
            Err(PackedError::UnknownType(7))
        );
        assert_eq!(
            parse_packed_document(&one_child(FixtureValue::Encrypted(vec![1]))),
            Err(PackedError::EncryptedBlob)
        );
    }

    #[test]
    fn rejects_invalid_offsets_indices_and_integer_or_float_sizes() {
        let mut outside = one_child(FixtureValue::String(b"x".to_vec()));
        let root = 5 + "child".len() + 2;
        outside[root + 8..root + 12].copy_from_slice(&descriptor(1, 100));
        assert_eq!(
            parse_packed_document(&outside),
            Err(PackedError::OffsetOutOfBounds)
        );

        let mut decreasing = one_child(FixtureValue::String(b"x".to_vec()));
        decreasing[root + 2..root + 6].copy_from_slice(&descriptor(1, 1));
        decreasing[root + 8..root + 12].copy_from_slice(&descriptor(1, 0));
        assert_eq!(
            parse_packed_document(&decreasing),
            Err(PackedError::OffsetDecreasing)
        );

        let mut bad_name = one_child(FixtureValue::String(b"x".to_vec()));
        bad_name[root + 6..root + 8].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(
            parse_packed_document(&bad_name),
            Err(PackedError::NameIndexOutOfBounds)
        );

        assert_eq!(
            parse_packed_document(&one_child(FixtureValue::Integer(vec![1, 2, 3]))),
            Err(PackedError::UnsupportedIntegerSize(3))
        );
        let invalid_float = one_child(FixtureValue::Unknown(3, vec![1, 2, 3]));
        assert_eq!(
            parse_packed_document(&invalid_float),
            Err(PackedError::InvalidFloatSize)
        );
        assert_eq!(
            resolve_end(usize::MAX, 1, usize::MAX),
            Err(PackedError::OffsetOverflow)
        );
    }

    #[test]
    fn enforces_all_central_limits() {
        let base = document(&[], &empty_section());
        let mut limits = PackedLimits {
            max_file_size: base.len() - 1,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&base, limits),
            Err(PackedError::InputTooLarge)
        );

        let with_string = document(&[b"long"], &empty_section());
        limits = PackedLimits {
            max_string_length: 3,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&with_string, limits),
            Err(PackedError::StringTooLong)
        );
        limits = PackedLimits {
            max_strings: 0,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&with_string, limits),
            Err(PackedError::TooManyStrings)
        );

        let with_child = one_child(FixtureValue::String(Vec::new()));
        limits = PackedLimits {
            max_children_per_section: 0,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&with_child, limits),
            Err(PackedError::TooManyChildren)
        );
        limits = PackedLimits {
            max_total_nodes: 1,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&with_child, limits),
            Err(PackedError::TooManyNodes)
        );

        let nested = one_child(FixtureValue::Section(Box::new(FixtureSection {
            own: FixtureValue::Section(Box::new(empty_section())),
            children: Vec::new(),
        })));
        limits = PackedLimits {
            max_depth: 1,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&nested, limits),
            Err(PackedError::DepthExceeded)
        );

        let blob = one_child(FixtureValue::Blob(vec![1, 2]));
        limits = PackedLimits {
            max_blob_size: 1,
            ..PackedLimits::default()
        };
        assert_eq!(
            parse_packed_document_with_limits(&blob, limits),
            Err(PackedError::BlobTooLarge)
        );
    }

    #[test]
    fn rejects_truncation_at_each_main_stage() {
        let full = one_child(FixtureValue::String(b"value".to_vec()));
        for length in 0..full.len() {
            assert!(
                parse_packed_document(&full[..length]).is_err(),
                "length {length}"
            );
        }
        assert!(parse_packed_document(&full).is_ok());
    }

    #[test]
    fn deterministic_mutations_never_panic_and_results_are_deterministic() {
        let fixture = one_child(FixtureValue::String(b"value".to_vec()));
        let mut inputs = vec![Vec::new(), vec![0; 64], fixture.clone()];
        for index in 0..fixture.len() {
            let mut mutated = fixture.clone();
            mutated[index] ^= (index as u8).wrapping_mul(31).wrapping_add(1);
            inputs.push(mutated);
        }
        let mut state = 0xC0FF_EE12_u32;
        for _ in 0..256 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let length = (state as usize) % 512;
            let mut random = Vec::with_capacity(length);
            for _ in 0..length {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                random.push((state >> 24) as u8);
            }
            inputs.push(random);
        }
        for input in inputs {
            let first = std::panic::catch_unwind(|| parse_packed_document(&input));
            assert!(first.is_ok());
            let first = first.unwrap();
            assert_eq!(first, parse_packed_document(&input));
        }
    }
}
