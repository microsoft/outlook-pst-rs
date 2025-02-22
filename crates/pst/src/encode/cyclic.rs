//! ## [Cyclic Encoding](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/9979fc01-0a3e-496f-900f-a6a867951f23)
//!
//! This is a direct port of the [crate::ndb::NdbCryptMethod::Cyclic] code from the PST specification.

use super::*;

/// Encode/decode the [crate::ndb::block::DataBlock] data.
pub fn encode_decode_block(data: &mut [u8], key: u32) {
    let r_table = key_data_r();
    let s_table = key_data_s();
    let i_table = key_data_i();

    let mut key = (key ^ (key >> 16)) as u16;

    for b in data.iter_mut() {
        let low_key = key as u8;
        let high_key = (key >> 8) as u8;

        *b = (*b).wrapping_add(low_key);
        *b = r_table[*b as usize];
        *b = (*b).wrapping_add(high_key);
        *b = s_table[*b as usize];
        *b = (*b).wrapping_sub(high_key);
        *b = i_table[*b as usize];
        *b = (*b).wrapping_sub(low_key);

        key = key.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"Hello, World!";
    const KEY: u32 = 0x1234_5678;

    #[test]
    fn test_encode_block() {
        let mut data = SAMPLE.to_vec();
        encode_decode_block(&mut data, KEY);
        assert_ne!(SAMPLE, &data);
    }

    #[test]
    fn test_decode_block() {
        let mut data = SAMPLE.to_vec();
        encode_decode_block(&mut data, KEY);
        encode_decode_block(&mut data, KEY);
        assert_eq!(SAMPLE, &data);
    }
}
