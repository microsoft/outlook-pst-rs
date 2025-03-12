#![doc = include_str!("../README.md")]

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Write};
use thiserror::Error;

mod crc;
mod dictionary;

use dictionary::{DictionaryReference, TokenDictionary};

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0:?}")]
    IoError(#[from] io::Error),
    #[error("COMPSIZE mismatch: {0}")]
    CompressedSizeMismatch(u32),
    #[error("COMPRESSED CRC mismatch: 0x{0:08X}")]
    CompressedCrcMismatch(u32),
    #[error("Invalid COMPTYPE: 0x{0:08X}")]
    InvalidCompressionType(u32),
    #[error("Dictionary reference error: {0:?}")]
    DictionaryError(#[from] dictionary::Error),
    #[error("Invalid ASCII RTF content")]
    InvalidAsciiRtf,
    #[error("COMPRESSED RTF too large: {0}")]
    CompressedRtfTooLarge(usize),
    #[error("UNCOMPRESSED RTF too large: {0}")]
    UncompressedRtfTooLarge(usize),
}

pub type Result<T> = std::result::Result<T, Error>;

const COMPRESSED: u32 = 0x75465A4C;
const UNCOMPRESSED: u32 = 0x414C454D;

pub fn decompress_rtf(data: &[u8]) -> Result<String> {
    let total_size = data.len();
    let mut cursor = Cursor::new(&data[..16]);
    let compressed_size = cursor.read_u32::<LittleEndian>()?;

    if compressed_size as usize + size_of_val(&compressed_size) != total_size {
        return Err(Error::CompressedSizeMismatch(compressed_size));
    }

    let raw_size = cursor.read_u32::<LittleEndian>()?;
    let compression_type = cursor.read_u32::<LittleEndian>()?;
    let crc = cursor.read_u32::<LittleEndian>()?;

    match compression_type {
        COMPRESSED => {
            let compressed_crc = crc::calculate_crc(0, &data[16..]);
            if crc != compressed_crc {
                return Err(Error::CompressedCrcMismatch(crc));
            }

            let mut dictionary = TokenDictionary::default();
            let mut output = Vec::with_capacity(raw_size as usize);

            let mut cursor = Cursor::new(&data[16..]);
            'decompress: while let Ok(control) = cursor.read_u8() {
                for i in 0..8 {
                    let bit = control & (0x01 << i);
                    if bit == 0 {
                        let Ok(byte) = cursor.read_u8() else {
                            break 'decompress;
                        };
                        output.push(byte);
                        dictionary.write_byte(byte);
                    } else {
                        let reference = DictionaryReference::read(&mut cursor)?;
                        let Some(mut reference) = dictionary.read_reference(reference) else {
                            break 'decompress;
                        };
                        output.append(&mut reference);
                    }
                }
            }

            let buffer: Vec<_> = output.into_iter().map(u16::from).collect();
            Ok(String::from_utf16_lossy(&buffer))
        }
        UNCOMPRESSED => {
            let data: Vec<_> = data[16..raw_size as usize + 16]
                .iter()
                .copied()
                .map(u16::from)
                .collect();
            Ok(String::from_utf16_lossy(&data))
        }
        invalid => Err(Error::InvalidCompressionType(invalid)),
    }
}

fn convert_to_ascii(rtf: &str) -> Result<Vec<u8>> {
    rtf.encode_utf16()
        .map(|ch| u8::try_from(ch).map_err(|_| Error::InvalidAsciiRtf))
        .collect()
}

pub fn compress_rtf(rtf: &str) -> Result<Vec<u8>> {
    let data = convert_to_ascii(rtf)?;
    if data.len() > u32::MAX as usize - 12 {
        return Err(Error::UncompressedRtfTooLarge(data.len()));
    }

    let mut output = Cursor::new(Vec::with_capacity(data.len() + 16));
    output.write_all(&[0_u8; 16])?;

    let mut read_offset = 0;
    let mut dictionary = TokenDictionary::default();
    let mut control = 0;
    let mut run_buffer = [0_u8; 16];
    let mut run_length = 0;

    'runs: while read_offset <= data.len() {
        let mut cursor = Cursor::new(run_buffer.as_mut_slice());

        control = 0;
        run_length = 0;

        for i in 0..8 {
            if read_offset >= data.len() {
                dictionary.final_reference().write(&mut cursor)?;
                control |= 0x01 << i;
                run_length += 2;
                break 'runs;
            }

            match dictionary.find_longest_match(&data[read_offset..])? {
                Some(best_match) => {
                    best_match.write(&mut cursor)?;
                    let best_match_length = best_match.length() as usize;
                    read_offset += best_match_length;
                    control |= 0x01 << i;
                    run_length += 2;
                }
                None => {
                    let byte = data[read_offset];
                    cursor.write_u8(byte)?;
                    read_offset += 1;
                    run_length += 1;
                }
            }
        }

        output.write_u8(control)?;
        output.write_all(&run_buffer[..run_length])?;
        run_length = 0;
    }

    if run_length > 0 {
        output.write_u8(control)?;
        output.write_all(&run_buffer[..run_length])?;
    }

    let mut output = output.into_inner();
    if output.len() > u32::MAX as usize - 12 {
        return Err(Error::CompressedRtfTooLarge(output.len()));
    }
    let compressed_size = output.len() as u32;
    let compressed_size = compressed_size - size_of_val(&compressed_size) as u32;
    let raw_size = data.len() as u32;
    let compression_type = COMPRESSED;
    let crc = crc::calculate_crc(0, &output[16..]);

    let mut header = Cursor::new(&mut output[..16]);
    header.write_u32::<LittleEndian>(compressed_size)?;
    header.write_u32::<LittleEndian>(raw_size)?;
    header.write_u32::<LittleEndian>(compression_type)?;
    header.write_u32::<LittleEndian>(crc)?;

    Ok(output)
}

pub fn encode_rtf(rtf: &str) -> Result<Vec<u8>> {
    let data = convert_to_ascii(rtf)?;
    if data.len() > u32::MAX as usize - 12 {
        return Err(Error::UncompressedRtfTooLarge(data.len()));
    }
    let raw_size = data.len() as u32;
    let compressed_size = raw_size + 12;
    let compression_type = UNCOMPRESSED;
    let crc = 0;

    let mut cursor = Cursor::new(Vec::with_capacity(raw_size as usize + 16));
    cursor.write_u32::<LittleEndian>(compressed_size)?;
    cursor.write_u32::<LittleEndian>(raw_size)?;
    cursor.write_u32::<LittleEndian>(compression_type)?;
    cursor.write_u32::<LittleEndian>(crc)?;
    cursor.write_all(&data)?;

    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPRESSED_SIMPLE_RTF: &[u8] = &[
        0x2d, 0x00, 0x00, 0x00, 0x2b, 0x00, 0x00, 0x00, 0x4c, 0x5a, 0x46, 0x75, 0xf1, 0xc5, 0xc7,
        0xa7, 0x03, 0x00, 0x0a, 0x00, 0x72, 0x63, 0x70, 0x67, 0x31, 0x32, 0x35, 0x42, 0x32, 0x0a,
        0xf3, 0x20, 0x68, 0x65, 0x6c, 0x09, 0x00, 0x20, 0x62, 0x77, 0x05, 0xb0, 0x6c, 0x64, 0x7d,
        0x0a, 0x80, 0x0f, 0xa0,
    ];

    const UNCOMPRESSED_SIMPLE_RTF: &str = "{\\rtf1\\ansi\\ansicpg1252\\pard hello world}\r\n";

    /// [Example 1: Simple Compressed RTF](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/029bff74-8c00-402e-ac2b-0210a5f57371)
    #[test]
    fn test_decompress_simple_rtf() {
        let rtf = decompress_rtf(&COMPRESSED_SIMPLE_RTF).unwrap();
        assert_eq!(rtf, UNCOMPRESSED_SIMPLE_RTF);
    }

    /// [Example 1: Simple RTF](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/ba662823-d47a-4db3-ad45-a368a82acc90)
    #[test]
    fn test_compress_simple_rtf() {
        let compressed = compress_rtf(UNCOMPRESSED_SIMPLE_RTF).unwrap();
        assert_eq!(&compressed, COMPRESSED_SIMPLE_RTF);
    }

    const COMPRESSED_CROSSING_WRITE_RTF: &[u8] = &[
        0x1a, 0x00, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x4c, 0x5a, 0x46, 0x75, 0xe2, 0xd4, 0x4b,
        0x51, 0x41, 0x00, 0x04, 0x20, 0x57, 0x58, 0x59, 0x5a, 0x0d, 0x6e, 0x7d, 0x01, 0x0e, 0xb0,
    ];

    const UNCOMPRESSED_CROSSING_WRITE_RTF: &str = "{\\rtf1 WXYZWXYZWXYZWXYZWXYZ}";

    /// [Example 2: Reading a Token from the Dictionary that Crosses WritePosition](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/421a2da5-7752-4985-8981-0f19f1e5b687)
    #[test]
    fn test_decompress_crossing_write_rtf() {
        let rtf = decompress_rtf(&COMPRESSED_CROSSING_WRITE_RTF).unwrap();
        assert_eq!(rtf, UNCOMPRESSED_CROSSING_WRITE_RTF);
    }

    /// [Example 2: Compressing with Tokens that Cross WritePosition](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/59eb3a35-6ee1-4a08-93b9-b9f4a7e3a0ca)
    #[test]
    fn test_compress_crossing_write_rtf() {
        let compressed = compress_rtf(UNCOMPRESSED_CROSSING_WRITE_RTF).unwrap();
        assert_eq!(&compressed, COMPRESSED_CROSSING_WRITE_RTF);
    }
}
