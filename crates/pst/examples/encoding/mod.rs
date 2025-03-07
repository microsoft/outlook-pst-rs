use outlook_pst::ltp::prop_context::PropertyValue;

pub fn decode_subject(value: &PropertyValue) -> Option<String> {
    match value {
        PropertyValue::String8(value) => {
            let offset = match value.buffer().first() {
                Some(1) => 2,
                _ => 0,
            };
            Some(String::from_utf8_lossy(&value.buffer()[offset..]).to_string())
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
        20127 => Some(String::from_utf8_lossy(buffer).to_string()),
        _ => {
            let coding = codepage_strings::Coding::new(code_page).ok()?;
            Some(coding.decode(buffer).ok()?.to_string())
        }
    }
}
