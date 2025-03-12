use compressed_rtf::*;
use outlook_pst::ltp::prop_context::PropertyValue;

pub fn decode_subject(value: &PropertyValue) -> Option<String> {
    match value {
        PropertyValue::String8(value) => {
            let offset = match value.buffer().first() {
                Some(1) => 2,
                _ => 0,
            };
            let buffer: Vec<_> = value
                .buffer()
                .iter()
                .skip(offset)
                .map(|&b| u16::from(b))
                .collect();
            Some(String::from_utf16_lossy(&buffer))
        }
        PropertyValue::Unicode(value) => {
            let offset = match value.buffer().first() {
                Some(1) => 2,
                _ => 0,
            };
            Some(String::from_utf16_lossy(&value.buffer()[offset..]))
        }
        _ => None,
    }
}

pub fn decode_html_body(buffer: &[u8], code_page: u16) -> Option<String> {
    match code_page {
        20127 => {
            let buffer: Vec<_> = buffer.iter().map(|&b| u16::from(b)).collect();
            Some(String::from_utf16_lossy(&buffer))
        }
        _ => {
            let coding = codepage_strings::Coding::new(code_page).ok()?;
            Some(coding.decode(buffer).ok()?.to_string())
        }
    }
}

pub fn decode_rtf_compressed(buffer: &[u8]) -> Option<String> {
    decompress_rtf(buffer).ok()
}
