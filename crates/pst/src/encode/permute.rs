//! ## [Permutative Encoding](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5faf4800-645d-49d1-9457-2ac40eb467bd)
//!
//! This is a direct port of the [crate::ndb::NdbCryptMethod::Permute] code from the PST specification.

use super::*;

/// Encode the [crate::ndb::block::DataBlock] data.
pub fn encode_block(data: &mut [u8]) {
    permute(data, key_data_r());
}

/// Decode the [crate::ndb::block::DataBlock] data.
pub fn decode_block(data: &mut [u8]) {
    permute(data, key_data_i());
}

fn permute(data: &mut [u8], table: &[u8]) {
    for b in data.iter_mut() {
        *b = table[*b as usize];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"Hello, World!";

    #[test]
    fn test_encode_block() {
        let mut data = SAMPLE.to_vec();
        encode_block(&mut data);
        assert_ne!(SAMPLE, &data);
    }

    #[test]
    fn test_decode_block() {
        let mut data = SAMPLE.to_vec();
        encode_block(&mut data);
        decode_block(&mut data);
        assert_eq!(SAMPLE, &data);
    }
}
