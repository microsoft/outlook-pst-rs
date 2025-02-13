//! ## [Block Signature](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e700a913-9db5-46a4-ac76-37cabea823e1)
//!
//! This is a direct port of the block signature calculation code from the PST specification.

/// Compute the block signature.
pub fn compute_sig(index: u32, block_id: u32) -> u16 {
    let value = index ^ block_id;
    (value >> 16) as u16 ^ (value as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sig() {
        assert_eq!(compute_sig(0x00000000, 0x00000000), 0x0000);
        assert_eq!(compute_sig(0x00000000, 0x00000001), 0x0001);
        assert_eq!(compute_sig(0x00000001, 0x00000000), 0x0001);
        assert_eq!(compute_sig(0x00000001, 0x00000001), 0x0000);
        assert_eq!(compute_sig(0x00000000, 0x00000002), 0x0002);
        assert_eq!(compute_sig(0x00000002, 0x00000000), 0x0002);
        assert_eq!(compute_sig(0x00000002, 0x00000002), 0x0000);
    }

    #[test]
    fn test_overflow() {
        assert_eq!(compute_sig(0x00000000, 0x00010000), 0x0001);
        assert_eq!(compute_sig(0x00010000, 0x00000000), 0x0001);
        assert_eq!(compute_sig(0x00010000, 0x00010000), 0x0000);
    }
}