//! ## [Data Types](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/1d61ee78-4466-4141-8276-f45153484619)

use std::fmt::Debug;

use super::*;

/// [Property Data Types](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/MS-OXCDATA/0c77892e-288e-435a-9c49-be1c20c7afdb)
#[repr(u16)]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum PropertyType {
    /// `PtypNull`: None: This property is a placeholder.
    #[default]
    Null = 0x0001,
    /// `PtypInteger16`: 2 bytes; a 16-bit integer
    Integer16 = 0x0002,
    /// `PtypInteger32`: 4 bytes; a 32-bit integer
    Integer32 = 0x0003,
    /// `PtypFloating32`: 4 bytes; a 32-bit floating-point number
    Floating32 = 0x0004,
    /// `PtypFloating64`: 8 bytes; a 64-bit floating-point number
    Floating64 = 0x0005,
    /// `PtypCurrency`: 8 bytes; a 64-bit signed, scaled integer representation of a decimal
    /// currency value, with four places to the right of the decimal point
    Currency = 0x0006,
    /// `PtypFloatingTime`: 8 bytes; a 64-bit floating point number in which the whole number part
    /// represents the number of days since December 30, 1899, and the fractional part represents
    /// the fraction of a day since midnight
    FloatingTime = 0x0007,
    /// `PtypErrorCode`: 4 bytes; a 32-bit integer encoding error information as specified in
    /// section [2.4.1](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcdata/c9dc2fb0-73ca-4cc2-bdee-cc6ffb9b70eb).
    ErrorCode = 0x000A,
    /// `PtypBoolean`: 1 byte; restricted to 1 or 0
    Boolean = 0x000B,
    /// `PtypInteger64`: 8 bytes; a 64-bit integer
    Integer64 = 0x0014,
    /// `PtypString8`: Variable size; a string of multibyte characters in externally specified
    /// encoding with terminating null character (single 0 byte).
    String8 = 0x001E,
    /// `PtypString`: Variable size; a string of Unicode characters in UTF-16LE format encoding
    /// with terminating null character (0x0000).
    Unicode = 0x001F,
    /// `PtypTime`: 8 bytes; a 64-bit integer representing the number of 100-nanosecond intervals
    /// since January 1, 1601
    Time = 0x0040,
    /// `PtypGuid`: 16 bytes; a GUID with Data1, Data2, and Data3 fields in little-endian format
    Guid = 0x0048,
    /// `PtypBinary`: Variable size; a COUNT field followed by that many bytes.
    Binary = 0x0102,
    /// `PtypObject`: The property value is a Component Object Model (COM) object, as specified in
    /// section [2.11.1.5](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcdata/5a024c95-2264-4832-9840-d6260c9c2cdb).
    Object = 0x000D,

    /// `PtypMultipleInteger16`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Integer16] values.
    MultipleInteger16 = 0x1002,
    /// `PtypMultipleInteger32`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Integer32] values.
    MultipleInteger32 = 0x1003,
    /// `PtypMultipleFloating32`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Floating32] values.
    MultipleFloating32 = 0x1004,
    /// `PtypMultipleFloating64`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Floating64] values.
    MultipleFloating64 = 0x1005,
    /// `PtypMultipleCurrency`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Currency] values.
    MultipleCurrency = 0x1006,
    /// `PtypMultipleFloatingTime`: Variable size; a COUNT field followed by that many
    /// [PropertyType::FloatingTime] values.
    MultipleFloatingTime = 0x1007,
    /// `PtypMultipleInteger64`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Integer64] values.
    MultipleInteger64 = 0x1014,
    /// `PtypMultipleString8`: Variable size; a COUNT field followed by that many
    /// [PropertyType::String8] values.
    MultipleString8 = 0x101E,
    /// `PtypMultipleString`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Unicode] values.
    MultipleUnicode = 0x101F,
    /// `PtypMultipleTime`: Variable size; a COUNT field followed by that many [PropertyType::Time]
    /// values.
    MultipleTime = 0x1040,
    /// `PtypMultipleGuid`: Variable size; a COUNT field followed by that many [PropertyType::Guid]
    /// values.
    MultipleGuid = 0x1048,
    /// `PtypMultipleBinary`: Variable size; a COUNT field followed by that many
    /// [PropertyType::Binary] values.
    MultipleBinary = 0x1102,
}

impl TryFrom<u16> for PropertyType {
    type Error = LtpError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::Null),
            0x0002 => Ok(Self::Integer16),
            0x0003 => Ok(Self::Integer32),
            0x0004 => Ok(Self::Floating32),
            0x0005 => Ok(Self::Floating64),
            0x0006 => Ok(Self::Currency),
            0x0007 => Ok(Self::FloatingTime),
            0x000A => Ok(Self::ErrorCode),
            0x000B => Ok(Self::Boolean),
            0x0014 => Ok(Self::Integer64),
            0x001E => Ok(Self::String8),
            0x001F => Ok(Self::Unicode),
            0x0040 => Ok(Self::Time),
            0x0048 => Ok(Self::Guid),
            0x0102 => Ok(Self::Binary),

            0x1002 => Ok(Self::MultipleInteger16),
            0x1003 => Ok(Self::MultipleInteger32),
            0x1004 => Ok(Self::MultipleFloating32),
            0x1005 => Ok(Self::MultipleFloating64),
            0x1006 => Ok(Self::MultipleCurrency),
            0x1007 => Ok(Self::MultipleFloatingTime),
            0x1014 => Ok(Self::MultipleInteger64),
            0x101E => Ok(Self::MultipleString8),
            0x101F => Ok(Self::MultipleUnicode),
            0x1040 => Ok(Self::MultipleTime),
            0x1048 => Ok(Self::MultipleGuid),
            0x1102 => Ok(Self::MultipleBinary),

            invalid => Err(LtpError::InvalidPropertyType(invalid)),
        }
    }
}

impl From<PropertyType> for u16 {
    fn from(value: PropertyType) -> Self {
        value as u16
    }
}
