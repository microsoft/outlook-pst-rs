//! [Dictionary](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/4238b0e2-7147-42da-88c9-ea45a1243e67)

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0:?}")]
    Io(#[from] io::Error),
    #[error("Invalid dictionary reference offset: {0}")]
    InvalidDictionaryReferenceOffset(u16),
    #[error("Invalid dictionary reference length: {0}")]
    InvalidDictionaryReferenceLength(u8),
}

pub type Result<T> = std::result::Result<T, Error>;

const INITIAL_DICTIONARY: &[u8] = b"{\\rtf1\\ansi\\mac\\deff0\\deftab720{\\fonttbl;}{\\f0\\fnil \\froman \\fswiss \\fmodern \\fscript \\fdecor MS Sans SerifSymbolArialTimes New RomanCourier{\\colortbl\\red0\\green0\\blue0\r\n\\par \\pard\\plain\\f0\\fs20\\b\\i\\u\\tab\\tx";

pub struct TokenDictionary {
    buffer: [u8; 4096],
    size: usize,
    read_offset: usize,
    write_offset: usize,
}

impl TokenDictionary {
    pub fn read_reference(&mut self, offset: DictionaryReference) -> Option<Vec<u8>> {
        let (offset, length) = (offset.offset() as usize, offset.length() as usize);
        if offset == self.write_offset {
            return None;
        }

        let mut result = Vec::with_capacity(length);

        self.read_offset = offset;
        for _ in 0..length {
            let byte = self.buffer[self.read_offset];
            result.push(byte);
            self.read_offset = (self.read_offset + 1) % self.buffer.len();
            self.write_byte(byte);
        }

        Some(result)
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.buffer[self.write_offset] = byte;
        self.size = self.buffer.len().min(self.size + 1);
        self.write_offset = (self.write_offset + 1) % self.buffer.len();
    }

    /// [Finding the Longest Match to Input](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/eb4b209b-f2f0-4876-a28b-1cfa1827423c)
    pub fn find_longest_match(&mut self, rtf: &[u8]) -> Result<Option<DictionaryReference>> {
        // SET finalOffset to the Write Offset of the Dictionary modulo 4096
        let final_offset = self.write_offset % self.buffer.len();

        // IF the Dictionary's End Offset is not equal to the Dictionary buffer size THEN
        let mut match_offset = if self.size != self.buffer.len() {
            // SET matchOffset to 0
            0
        } else {
            // SET matchOffset to (the Dictionary's Write Offset + 1) modulo 4096
            ((self.write_offset + 1) % self.buffer.len()) as u16
        };

        // SET bestMatchLength to 0
        let mut best_match: Option<DictionaryMatch> = None;

        // REPEAT
        loop {
            let best_match_length = best_match.map(|m| m.length).unwrap_or_default();
            best_match = self
                .try_match(rtf, match_offset, best_match_length)?
                .or(best_match);

            // SET matchOffset to (matchOffset + 1) modulo 4096
            match_offset = (match_offset + 1) % self.buffer.len() as u16;

            // UNTIL matchOffset equals finalOffset
            if match_offset as usize == final_offset {
                break;
            }

            // OR until bestMatchLength is 17 bytes long
            if let Some(best_match) = best_match {
                if best_match.length == 17 {
                    break;
                }
            }
        }

        // IF bestMatchLength is 0 THEN
        if best_match.map(|m| m.length).unwrap_or_default() == 0 {
            // CALL AddByteToDictionary with the byte at Input Cursor
            self.write_byte(rtf[0]);
        }

        // RETURN offset of bestMatchOffset and bestMatchLength
        Ok(best_match.and_then(|m| DictionaryReference::try_from(m).ok()))
    }

    fn try_match(
        &mut self,
        rtf: &[u8],
        match_offset: u16,
        best_match_length: u8,
    ) -> Result<Option<DictionaryMatch>> {
        // SET maxLength to the minimum of 17 and remaining bytes of Input
        let max_length = rtf.len().min(17);
        // SET matchLength to 0
        let mut match_length = 0_u8;
        // SET dictionaryOffset to matchOffset
        let mut dictionary_offset = match_offset as usize;

        // WHILE matchLength is less than maxLength AND
        while (match_length as usize) < max_length {
            let byte = rtf[match_length as usize];
            // the byte in the Dictionary at dictionaryOffset is equal to the byte in Input at the inputOffset
            if self.buffer[dictionary_offset] != byte {
                break;
            }

            // INCREMENT matchLength
            match_length += 1;

            // IF matchLength is greater than bestMatchLength THEN
            if match_length > best_match_length {
                // CALL AddByteToDictionary with the byte in Input at the inputOffset
                self.write_byte(byte);
            }

            // SET dictionaryOffset to (dictionaryOffset + 1) modulo 4096
            dictionary_offset = (dictionary_offset + 1) % self.buffer.len();
        }

        // IF matchLength is greater than bestMatchLength THEN
        if match_length > best_match_length {
            Ok(Some(DictionaryMatch {
                // SET bestMatchOffset to matchOffset
                offset: match_offset,
                // SET bestMatchLength to matchLength
                length: match_length,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn final_reference(self) -> DictionaryReference {
        DictionaryReference::new(self.write_offset as u16, 0)
    }
}

impl Default for TokenDictionary {
    fn default() -> Self {
        let mut buffer = [0; 4096];
        buffer[..INITIAL_DICTIONARY.len()].copy_from_slice(INITIAL_DICTIONARY);
        Self {
            buffer,
            size: INITIAL_DICTIONARY.len(),
            read_offset: 0,
            write_offset: INITIAL_DICTIONARY.len(),
        }
    }
}

#[derive(Clone, Copy)]
struct DictionaryMatch {
    offset: u16,
    length: u8,
}

/// [Dictionary Reference](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfcp/b12474df-e0ef-4731-9315-454a49a984d8)
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DictionaryReference(u16);

impl DictionaryReference {
    fn new(offset: u16, length_minus_2: u8) -> Self {
        let value = u16::from(length_minus_2) | (offset << 4);
        Self(value)
    }

    pub fn offset(&self) -> u16 {
        (self.0 & 0xFFF0) >> 4
    }

    pub fn length(&self) -> u8 {
        (self.0 & 0x0F) as u8 + 2
    }

    pub fn read(f: &mut dyn Read) -> io::Result<Self> {
        Ok(Self(f.read_u16::<BigEndian>()?))
    }

    pub fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<BigEndian>(self.0)
    }
}

impl TryFrom<DictionaryMatch> for DictionaryReference {
    type Error = Error;

    fn try_from(value: DictionaryMatch) -> Result<Self> {
        if value.offset > 0x0FFF {
            return Err(Error::InvalidDictionaryReferenceOffset(value.offset));
        }
        if !(2..=0x11).contains(&value.length) {
            return Err(Error::InvalidDictionaryReferenceLength(value.length));
        }

        Ok(Self::new(value.offset, value.length - 2))
    }
}
