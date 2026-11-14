//! Lower-case hex rendering of digests. sha2 0.11 returns a `hybrid_array::Array` that no longer
//! implements `LowerHex`, so every digest is rendered through this one helper.

use std::fmt::Write as _;

pub(crate) fn hex_lower(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::hex_lower;

    #[test]
    fn renders_fixed_width_lower_case_hex() {
        assert_eq!(hex_lower([]), "");
        assert_eq!(hex_lower([0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }
}
