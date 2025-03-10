//! ## [Property Context (PC)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/294c83c6-ff92-42f5-b6b6-876c29fa9737)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use core::mem;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
    io::{self, Cursor, Read, Seek, Write},
};

use super::{heap::*, prop_type::*, read_write::*, tree::*, *};
use crate::ndb::{
    block::{AnsiDataTree, AnsiSubNodeTree, Block, UnicodeDataTree, UnicodeSubNodeTree},
    header::NdbCryptMethod,
    node_id::{NodeId, NodeIdType},
    page::{
        AnsiBlockBTree, AnsiNodeBTreeEntry, NodeBTreeEntry, RootBTree, UnicodeBlockBTree,
        UnicodeNodeBTreeEntry,
    },
    read_write::NodeIdReadWrite,
};

#[derive(Copy, Clone)]
pub enum PropertyValueRecord {
    Small(u32),
    Heap(HeapId),
    Node(NodeId),
}

impl PropertyValueRecord {
    pub fn small_value(&self, prop_type: PropertyType) -> Option<PropertyValue> {
        match (self, prop_type) {
            (PropertyValueRecord::Small(value), PropertyType::Integer16) => {
                Some(PropertyValue::Integer16((*value & 0xFFFF) as i16))
            }
            (PropertyValueRecord::Small(value), PropertyType::Integer32) => {
                Some(PropertyValue::Integer32(*value as i32))
            }
            (PropertyValueRecord::Small(value), PropertyType::Floating32) => {
                Some(PropertyValue::Floating32(f32::from_bits(*value)))
            }
            (PropertyValueRecord::Small(value), PropertyType::ErrorCode) => {
                Some(PropertyValue::ErrorCode(*value as i32))
            }
            (PropertyValueRecord::Small(value), PropertyType::Boolean) => {
                Some(PropertyValue::Boolean(*value & 0xFF != 0))
            }
            _ => None,
        }
    }
}

impl Debug for PropertyValueRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValueRecord::Small(value) => write!(f, "Small(0x{value:08X})"),
            PropertyValueRecord::Heap(heap_id) => write!(f, "{heap_id:?}"),
            PropertyValueRecord::Node(node_id) => write!(f, "{node_id:?}"),
        }
    }
}

impl From<PropertyValueRecord> for u32 {
    fn from(value: PropertyValueRecord) -> Self {
        match value {
            PropertyValueRecord::Small(value) => value,
            PropertyValueRecord::Heap(heap_id) => u32::from(heap_id),
            PropertyValueRecord::Node(node_id) => u32::from(node_id),
        }
    }
}

pub type PropertyTreeRecordKey = u16;

impl HeapTreeEntryKey for PropertyTreeRecordKey {
    const SIZE: u8 = 2;
}

impl HeapNodeReadWrite for PropertyTreeRecordKey {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        f.read_u16::<LittleEndian>()
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(*self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyTreeRecordValue {
    prop_type: PropertyType,
    value: PropertyValueRecord,
}

impl PropertyTreeRecordValue {
    pub fn new(prop_type: PropertyType, value: PropertyValueRecord) -> Self {
        Self { prop_type, value }
    }

    pub fn prop_type(&self) -> PropertyType {
        self.prop_type
    }

    pub fn value(&self) -> PropertyValueRecord {
        self.value
    }
}

impl HeapTreeEntryValue for PropertyTreeRecordValue {
    const SIZE: u8 = 6;
}

impl HeapNodeReadWrite for PropertyTreeRecordValue {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        // wPropType
        let prop_type = f.read_u16::<LittleEndian>()?;
        let prop_type = PropertyType::try_from(prop_type)?;

        // dwValueHnid
        let value = f.read_u32::<LittleEndian>()?;
        let value = match prop_type {
            PropertyType::Null => PropertyValueRecord::Small(0),

            PropertyType::Integer16 => PropertyValueRecord::Small(value & 0xFFFF),

            PropertyType::Integer32 | PropertyType::Floating32 | PropertyType::ErrorCode => {
                PropertyValueRecord::Small(value)
            }

            PropertyType::Boolean => PropertyValueRecord::Small(value & 0xFF),

            PropertyType::Floating64
            | PropertyType::Currency
            | PropertyType::FloatingTime
            | PropertyType::Integer64
            | PropertyType::Time
            | PropertyType::Guid
            | PropertyType::Object => PropertyValueRecord::Heap(HeapId::from(value)),

            _ => match NodeId::from(value).id_type() {
                Ok(NodeIdType::HeapNode) => PropertyValueRecord::Heap(HeapId::from(value)),
                _ => PropertyValueRecord::Node(NodeId::from(value)),
            },
        };

        Ok(Self { prop_type, value })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(u16::from(self.prop_type))?;
        f.write_u32::<LittleEndian>(u32::from(self.value))
    }
}

/// [PC BTH Record](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/7daab6f5-ce65-437e-80d5-1b1be4088bd3)
#[derive(Clone, Copy)]
pub struct PropertyTreeRecord {
    key: PropertyTreeRecordKey,
    data: PropertyTreeRecordValue,
}

impl PropertyTreeRecord {
    pub fn new(prop_id: u16, prop_type: PropertyType, value: PropertyValueRecord) -> Self {
        Self {
            key: prop_id,
            data: PropertyTreeRecordValue::new(prop_type, value),
        }
    }

    pub fn prop_id(&self) -> u16 {
        self.key
    }

    pub fn prop_type(&self) -> PropertyType {
        self.data.prop_type()
    }

    pub fn value(&self) -> PropertyValueRecord {
        self.data.value()
    }
}

impl PropertyTreeRecordReadWrite for PropertyTreeRecord {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let key = f.read_u16::<LittleEndian>()?;
        let data = PropertyTreeRecordValue::read(f)?;

        Ok(Self { key, data })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.key)?;
        self.data.write(f)
    }
}

#[derive(Clone, Default)]
pub struct String8Value {
    buffer: Vec<u8>,
}

impl String8Value {
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl Display for String8Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let buffer: Vec<_> = self.buffer.iter().map(|&b| u16::from(b)).collect();
        let value = String::from_utf16_lossy(&buffer);
        write!(f, "{value}")
    }
}

impl Debug for String8Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.to_string();
        write!(f, "String8Value {{ {value:?} }}")
    }
}

#[derive(Clone, Default)]
pub struct UnicodeValue {
    buffer: Vec<u16>,
}

impl UnicodeValue {
    pub fn buffer(&self) -> &[u16] {
        &self.buffer
    }
}

impl Display for UnicodeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = String::from_utf16_lossy(&self.buffer);
        write!(f, "{value}")
    }
}

impl Debug for UnicodeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.to_string();
        write!(f, "UnicodeValue {{ {value:?} }}")
    }
}

#[derive(Clone, Copy, Default)]
pub struct GuidValue {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl GuidValue {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }

    pub fn data1(&self) -> u32 {
        self.data1
    }

    pub fn data2(&self) -> u16 {
        self.data2
    }

    pub fn data3(&self) -> u16 {
        self.data3
    }

    pub fn data4(&self) -> &[u8; 8] {
        &self.data4
    }
}

impl Debug for GuidValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GuidValue {{ {:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X} }}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7]
        )
    }
}

#[derive(Clone, Default)]
pub struct BinaryValue {
    buffer: Vec<u8>,
}

impl BinaryValue {
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl Debug for BinaryValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self
            .buffer
            .iter()
            .map(|ch| format!("{ch:02X}"))
            .collect::<Vec<_>>()
            .join("-");
        write!(f, "BinaryValue {{ {value} }}")
    }
}

#[derive(Clone, Copy, Default)]
pub struct ObjectValue {
    node_id: NodeId,
    size: u32,
}

impl ObjectValue {
    pub fn node(&self) -> NodeId {
        self.node_id
    }

    pub fn size(&self) -> u32 {
        self.size
    }
}

impl Debug for ObjectValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ObjectValue {{ {:?}, size: 0x{:X} }}",
            self.node_id, self.size
        )
    }
}

#[derive(Clone, Default, Debug)]
pub enum PropertyValue {
    /// `PtypNull`: None: This property is a placeholder.
    #[default]
    Null,
    /// `PtypInteger16`: 2 bytes; a 16-bit integer
    Integer16(i16),
    /// `PtypInteger32`: 4 bytes; a 32-bit integer
    Integer32(i32),
    /// `PtypFloating32`: 4 bytes; a 32-bit floating-point number
    Floating32(f32),
    /// `PtypFloating64`: 8 bytes; a 64-bit floating-point number
    Floating64(f64),
    /// `PtypCurrency`: 8 bytes; a 64-bit signed, scaled integer representation of a decimal
    /// currency value, with four places to the right of the decimal point
    Currency(i64),
    /// `PtypFloatingTime`: 8 bytes; a 64-bit floating point number in which the whole number part
    /// represents the number of days since December 30, 1899, and the fractional part represents
    /// the fraction of a day since midnight
    FloatingTime(f64),
    /// `PtypErrorCode`: 4 bytes; a 32-bit integer encoding error information as specified in
    /// section [2.4.1](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcdata/c9dc2fb0-73ca-4cc2-bdee-cc6ffb9b70eb).
    ErrorCode(i32),
    /// `PtypBoolean`: 1 byte; restricted to 1 or 0
    Boolean(bool),
    /// `PtypInteger64`: 8 bytes; a 64-bit integer
    Integer64(i64),
    /// `PtypString8`: Variable size; a string of multibyte characters in externally specified
    /// encoding with terminating null character (single 0 byte).
    String8(String8Value),
    /// `PtypString`: Variable size; a string of Unicode characters in UTF-16LE format encoding
    /// with terminating null character (0x0000).
    Unicode(UnicodeValue),
    /// `PtypTime`: 8 bytes; a 64-bit integer representing the number of 100-nanosecond intervals
    /// since January 1, 1601
    Time(i64),
    /// `PtypGuid`: 16 bytes; a GUID with Data1, Data2, and Data3 fields in little-endian format
    Guid(GuidValue),
    /// `PtypBinary`: Variable size; a COUNT field followed by that many bytes.
    Binary(BinaryValue),
    /// `PtypObject`: The property value is a Component Object Model (COM) object, as specified in
    /// section [2.11.1.5](https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxcdata/5a024c95-2264-4832-9840-d6260c9c2cdb).
    Object(ObjectValue),

    /// `PtypMultipleInteger16`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Integer16] values.
    MultipleInteger16(Vec<i16>),
    /// `PtypMultipleInteger32`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Integer32] values.
    MultipleInteger32(Vec<i32>),
    /// `PtypMultipleFloating32`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Floating32] values.
    MultipleFloating32(Vec<f32>),
    /// `PtypFloating64`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Floating64] values.
    MultipleFloating64(Vec<f64>),
    /// `PtypMultipleCurrency`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Currency] values.
    MultipleCurrency(Vec<i64>),
    /// `PtypMultipleFloatingTime`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::FloatingTime] values.
    MultipleFloatingTime(Vec<f64>),
    /// `PtypMultipleInteger64`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Integer64] values.
    MultipleInteger64(Vec<i64>),
    /// `PtypMultipleString8`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::String8] values.
    MultipleString8(Vec<String8Value>),
    /// `PtypMultipleString`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Unicode] values.
    MultipleUnicode(Vec<UnicodeValue>),
    /// `PtypMultipleTime`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Time] values.
    MultipleTime(Vec<i64>),
    /// `PtypMultipleGuid`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Guid] values.
    MultipleGuid(Vec<GuidValue>),
    /// `PtypMultipleBinary`: Variable size; a COUNT field followed by that many
    /// [PropertyValue::Binary] values.
    MultipleBinary(Vec<BinaryValue>),
}

impl From<&PropertyValue> for PropertyType {
    fn from(value: &PropertyValue) -> Self {
        match value {
            PropertyValue::Null => PropertyType::Null,
            PropertyValue::Integer16(_) => PropertyType::Integer16,
            PropertyValue::Integer32(_) => PropertyType::Integer32,
            PropertyValue::Floating32(_) => PropertyType::Floating32,
            PropertyValue::Floating64(_) => PropertyType::Floating64,
            PropertyValue::Currency(_) => PropertyType::Currency,
            PropertyValue::FloatingTime(_) => PropertyType::FloatingTime,
            PropertyValue::ErrorCode(_) => PropertyType::ErrorCode,
            PropertyValue::Boolean(_) => PropertyType::Boolean,
            PropertyValue::Integer64(_) => PropertyType::Integer64,
            PropertyValue::String8(_) => PropertyType::String8,
            PropertyValue::Unicode(_) => PropertyType::Unicode,
            PropertyValue::Time(_) => PropertyType::Time,
            PropertyValue::Guid(_) => PropertyType::Guid,
            PropertyValue::Binary(_) => PropertyType::Binary,
            PropertyValue::Object(_) => PropertyType::Object,
            PropertyValue::MultipleInteger16(_) => PropertyType::MultipleInteger16,
            PropertyValue::MultipleInteger32(_) => PropertyType::MultipleInteger32,
            PropertyValue::MultipleFloating32(_) => PropertyType::MultipleFloating32,
            PropertyValue::MultipleFloating64(_) => PropertyType::MultipleFloating64,
            PropertyValue::MultipleCurrency(_) => PropertyType::MultipleCurrency,
            PropertyValue::MultipleFloatingTime(_) => PropertyType::MultipleFloatingTime,
            PropertyValue::MultipleInteger64(_) => PropertyType::MultipleInteger64,
            PropertyValue::MultipleString8(_) => PropertyType::MultipleString8,
            PropertyValue::MultipleUnicode(_) => PropertyType::MultipleUnicode,
            PropertyValue::MultipleTime(_) => PropertyType::MultipleTime,
            PropertyValue::MultipleGuid(_) => PropertyType::MultipleGuid,
            PropertyValue::MultipleBinary(_) => PropertyType::MultipleBinary,
        }
    }
}

impl PropertyValueReadWrite for PropertyValue {
    fn read(f: &mut dyn Read, prop_type: PropertyType) -> io::Result<Self> {
        match prop_type {
            PropertyType::Floating64 => {
                let value = f.read_f64::<LittleEndian>()?;
                Ok(Self::Floating64(value))
            }

            PropertyType::Currency => {
                let value = f.read_i64::<LittleEndian>()?;
                Ok(Self::Currency(value))
            }

            PropertyType::FloatingTime => {
                let value = f.read_f64::<LittleEndian>()?;
                Ok(Self::FloatingTime(value))
            }

            PropertyType::Integer64 => {
                let value = f.read_i64::<LittleEndian>()?;
                Ok(Self::Integer64(value))
            }

            PropertyType::String8 => {
                let mut buffer = Vec::new();
                f.read_to_end(&mut buffer)?;
                Ok(Self::String8(String8Value { buffer }))
            }

            PropertyType::Unicode => {
                let mut buffer = Vec::new();
                while let Ok(ch) = f.read_u16::<LittleEndian>() {
                    buffer.push(ch);
                }
                Ok(Self::Unicode(UnicodeValue { buffer }))
            }

            PropertyType::Time => {
                let value = f.read_i64::<LittleEndian>()?;
                Ok(Self::Time(value))
            }

            PropertyType::Guid => {
                let data1 = f.read_u32::<LittleEndian>()?;
                let data2 = f.read_u16::<LittleEndian>()?;
                let data3 = f.read_u16::<LittleEndian>()?;
                let mut data4 = [0; 8];
                f.read_exact(&mut data4)?;
                Ok(Self::Guid(GuidValue {
                    data1,
                    data2,
                    data3,
                    data4,
                }))
            }

            PropertyType::Binary => {
                let mut buffer = Vec::new();
                f.read_to_end(&mut buffer)?;
                Ok(Self::Binary(BinaryValue { buffer }))
            }

            PropertyType::Object => {
                let node_id = NodeId::read(f)?;
                let size = f.read_u32::<LittleEndian>()?;
                Ok(Self::Object(ObjectValue { node_id, size }))
            }

            PropertyType::MultipleInteger16 => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_i16::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleInteger16(values))
            }

            PropertyType::MultipleInteger32 => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_i32::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleInteger32(values))
            }

            PropertyType::MultipleFloating32 => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_f32::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleFloating32(values))
            }

            PropertyType::MultipleFloating64 => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_f64::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleFloating64(values))
            }

            PropertyType::MultipleCurrency => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_i64::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleCurrency(values))
            }

            PropertyType::MultipleFloatingTime => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_f64::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleFloatingTime(values))
            }

            PropertyType::MultipleInteger64 => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_i64::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleInteger64(values))
            }

            PropertyType::MultipleString8 => {
                // ulCount
                let count = f.read_u32::<LittleEndian>()? as usize;

                // rgulDataOffsets
                let mut offsets = Vec::with_capacity(count);
                for _ in 0..count {
                    offsets.push(f.read_u32::<LittleEndian>()? as usize);
                }

                // rgDataItems
                let mut start = (offsets.len() + 1) * mem::size_of::<u32>();
                let mut values = Vec::with_capacity(offsets.len());
                for i in 0..offsets.len() {
                    let next = offsets[i];
                    if next != start {
                        return Err(LtpError::InvalidMultiValuePropertyOffset(next).into());
                    }

                    let buffer = if i < offsets.len() - 1 {
                        let next = offsets[i + 1];
                        if next <= start {
                            return Err(LtpError::InvalidMultiValuePropertyOffset(next).into());
                        }

                        let mut buffer = vec![0; next - start];
                        start = next;
                        f.read_exact(&mut buffer)?;
                        buffer
                    } else {
                        let mut buffer = Vec::new();
                        f.read_to_end(&mut buffer)?;
                        buffer
                    };

                    values.push(String8Value { buffer });
                }

                Ok(Self::MultipleString8(values))
            }

            PropertyType::MultipleUnicode => {
                // ulCount
                let count = f.read_u32::<LittleEndian>()? as usize;

                // rgulDataOffsets
                let mut offsets = Vec::with_capacity(count);
                for _ in 0..count {
                    offsets.push(f.read_u32::<LittleEndian>()? as usize);
                }

                // rgDataItems
                let mut start = (offsets.len() + 1) * mem::size_of::<u32>();
                let mut values = Vec::with_capacity(offsets.len());
                for i in 0..offsets.len() {
                    let next = offsets[i];
                    if next != start {
                        return Err(LtpError::InvalidMultiValuePropertyOffset(next).into());
                    }

                    let mut buffer = Vec::new();
                    if i < offsets.len() - 1 {
                        let next = offsets[i + 1];
                        if next <= start {
                            return Err(LtpError::InvalidMultiValuePropertyOffset(next).into());
                        }

                        while start < next {
                            let ch = f.read_u16::<LittleEndian>()?;
                            buffer.push(ch);
                            start += mem::size_of::<u16>();
                        }
                    } else {
                        while let Ok(ch) = f.read_u16::<LittleEndian>() {
                            buffer.push(ch);
                        }
                    };

                    values.push(UnicodeValue { buffer });
                }

                Ok(Self::MultipleUnicode(values))
            }

            PropertyType::MultipleTime => {
                let mut values = Vec::new();
                while let Ok(value) = f.read_i64::<LittleEndian>() {
                    values.push(value);
                }
                Ok(Self::MultipleTime(values))
            }

            PropertyType::MultipleGuid => {
                let count = f.read_u32::<LittleEndian>()? as usize;
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    let data1 = f.read_u32::<LittleEndian>()?;
                    let data2 = f.read_u16::<LittleEndian>()?;
                    let data3 = f.read_u16::<LittleEndian>()?;
                    let mut data4 = [0; 8];
                    f.read_exact(&mut data4)?;
                    values.push(GuidValue {
                        data1,
                        data2,
                        data3,
                        data4,
                    });
                }

                Ok(Self::MultipleGuid(values))
            }

            PropertyType::MultipleBinary => {
                // ulCount
                let count = f.read_u32::<LittleEndian>()? as usize;

                // rgulDataOffsets
                let mut offsets = Vec::with_capacity(count);
                for _ in 0..count {
                    offsets.push(f.read_u32::<LittleEndian>()? as usize);
                }

                // rgDataItems
                let mut start = (offsets.len() + 1) * mem::size_of::<u32>();
                let mut values = Vec::with_capacity(offsets.len());
                for i in 0..offsets.len() {
                    let next = offsets[i];
                    if next != start {
                        return Err(LtpError::InvalidMultiValuePropertyOffset(next).into());
                    }

                    let buffer = if i < offsets.len() - 1 {
                        let next = offsets[i + 1];
                        if next <= start {
                            return Err(LtpError::InvalidMultiValuePropertyOffset(next).into());
                        }

                        let mut buffer = vec![0; next - start];
                        start = next;
                        f.read_exact(&mut buffer)?;
                        buffer
                    } else {
                        let mut buffer = Vec::new();
                        f.read_to_end(&mut buffer)?;
                        buffer
                    };

                    values.push(BinaryValue { buffer });
                }

                Ok(Self::MultipleBinary(values))
            }

            _ => Err(LtpError::InvalidVariableLengthPropertyType(prop_type).into()),
        }
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        match self {
            Self::Floating64(value) => f.write_f64::<LittleEndian>(*value),

            Self::Currency(value) => f.write_i64::<LittleEndian>(*value),

            Self::FloatingTime(value) => f.write_f64::<LittleEndian>(*value),

            Self::Integer64(value) => f.write_i64::<LittleEndian>(*value),

            Self::String8(value) => f.write_all(value.buffer()),

            Self::Unicode(value) => {
                for ch in value.buffer() {
                    f.write_u16::<LittleEndian>(*ch)?;
                }
                Ok(())
            }

            Self::Time(value) => f.write_i64::<LittleEndian>(*value),

            Self::Guid(value) => {
                f.write_u32::<LittleEndian>(value.data1)?;
                f.write_u16::<LittleEndian>(value.data2)?;
                f.write_u16::<LittleEndian>(value.data3)?;
                f.write_all(&value.data4)
            }

            Self::Binary(value) => f.write_all(value.buffer()),

            Self::Object(value) => {
                value.node_id.write(f)?;
                f.write_u32::<LittleEndian>(value.size)
            }

            Self::MultipleInteger16(values) => {
                for value in values {
                    f.write_i16::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleInteger32(values) => {
                for value in values {
                    f.write_i32::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleFloating32(values) => {
                for value in values {
                    f.write_f32::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleFloating64(values) => {
                for value in values {
                    f.write_f64::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleCurrency(values) => {
                for value in values {
                    f.write_i64::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleFloatingTime(values) => {
                for value in values {
                    f.write_f64::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleInteger64(values) => {
                for value in values {
                    f.write_i64::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleString8(values) => {
                // ulCount
                let count = u32::try_from(values.len())
                    .map_err(|_| LtpError::InvalidMultiValuePropertyCount(values.len()))?;
                f.write_u32::<LittleEndian>(count)?;

                // rgulDataOffsets
                let mut start = (values.len() + 1) * mem::size_of::<u32>();
                for value in values {
                    let offset = u32::try_from(start)
                        .map_err(|_| LtpError::InvalidMultiValuePropertyOffset(start))?;
                    f.write_u32::<LittleEndian>(offset)?;
                    start += value.buffer().len();
                }

                // rgDataItems
                for value in values {
                    f.write_all(value.buffer())?;
                }

                Ok(())
            }

            Self::MultipleUnicode(values) => {
                // ulCount
                let count = u32::try_from(values.len())
                    .map_err(|_| LtpError::InvalidMultiValuePropertyCount(values.len()))?;
                f.write_u32::<LittleEndian>(count)?;

                // rgulDataOffsets
                let mut start = (values.len() + 1) * mem::size_of::<u32>();
                for value in values {
                    let offset = u32::try_from(start)
                        .map_err(|_| LtpError::InvalidMultiValuePropertyOffset(start))?;
                    f.write_u32::<LittleEndian>(offset)?;
                    start += mem::size_of_val(value.buffer());
                }

                // rgDataItems
                for value in values {
                    for ch in value.buffer() {
                        f.write_u16::<LittleEndian>(*ch)?;
                    }
                }

                Ok(())
            }

            Self::MultipleTime(values) => {
                for value in values {
                    f.write_i64::<LittleEndian>(*value)?;
                }
                Ok(())
            }

            Self::MultipleGuid(values) => {
                for value in values {
                    f.write_u32::<LittleEndian>(value.data1)?;
                    f.write_u16::<LittleEndian>(value.data2)?;
                    f.write_u16::<LittleEndian>(value.data3)?;
                    f.write_all(&value.data4)?;
                }
                Ok(())
            }

            Self::MultipleBinary(values) => {
                // ulCount
                let count = u32::try_from(values.len())
                    .map_err(|_| LtpError::InvalidMultiValuePropertyCount(values.len()))?;
                f.write_u32::<LittleEndian>(count)?;

                // rgulDataOffsets
                let mut start = (values.len() + 1) * mem::size_of::<u32>();
                for value in values {
                    let offset = u32::try_from(start)
                        .map_err(|_| LtpError::InvalidMultiValuePropertyOffset(start))?;
                    f.write_u32::<LittleEndian>(offset)?;
                    start += value.buffer().len();
                }

                // rgDataItems
                for value in values {
                    f.write_all(value.buffer())?;
                }

                Ok(())
            }

            _ => Err(LtpError::InvalidVariableLengthPropertyType(self.into()).into()),
        }
    }
}

pub struct UnicodePropertyContext {
    node: UnicodeNodeBTreeEntry,
    tree: UnicodeHeapTree,
}

impl UnicodePropertyContext {
    pub fn new(node: UnicodeNodeBTreeEntry, tree: UnicodeHeapTree) -> Self {
        Self { node, tree }
    }

    pub fn tree(&self) -> &UnicodeHeapTree {
        &self.tree
    }

    pub fn properties<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_tree: &UnicodeBlockBTree,
    ) -> io::Result<BTreeMap<PropertyTreeRecordKey, PropertyTreeRecordValue>> {
        let header = self.tree.header(f, encoding, block_tree)?;
        let key_size = header.key_size();
        if key_size != 2 {
            return Err(LtpError::InvalidPropertyTreeKeySize(key_size).into());
        }
        let entry_size = header.entry_size();
        if entry_size != 6 {
            return Err(LtpError::InvalidPropertyTreeEntrySize(entry_size).into());
        }

        Ok(self
            .tree
            .entries(f, encoding, block_tree)?
            .into_iter()
            .map(
                |entry: HeapTreeLeafEntry<PropertyTreeRecordKey, PropertyTreeRecordValue>| {
                    (
                        entry.key(),
                        PropertyTreeRecordValue::new(
                            entry.data().prop_type(),
                            entry.data().value(),
                        ),
                    )
                },
            )
            .collect())
    }

    pub fn read_property<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_btree: &UnicodeBlockBTree,
        value: PropertyTreeRecordValue,
    ) -> io::Result<PropertyValue> {
        match value.value() {
            PropertyValueRecord::Heap(heap_id) => {
                if u32::from(heap_id) == 0 {
                    return Ok(PropertyValue::Null);
                }

                let data = self
                    .tree
                    .heap()
                    .find_entry(heap_id, f, encoding, block_btree)?;
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, value.prop_type())
            }
            PropertyValueRecord::Node(sub_node_id) => {
                let sub_node =
                    self.node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(u32::from(
                            sub_node_id,
                        )))?;
                let block = block_btree.find_entry(f, u64::from(sub_node))?;
                let sub_node_tree = UnicodeSubNodeTree::read(f, &block)?;
                let block = sub_node_tree.find_entry(f, block_btree, sub_node_id)?;
                let block = block_btree.find_entry(f, u64::from(block))?;
                let data_tree = UnicodeDataTree::read(f, encoding, &block)?;
                let blocks: Vec<_> = data_tree.blocks(f, encoding, block_btree)?.collect();
                let data: Vec<_> = blocks
                    .iter()
                    .flat_map(|block| block.data())
                    .copied()
                    .collect();
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, value.prop_type())
            }
            small => small
                .small_value(value.prop_type())
                .ok_or(LtpError::InvalidSmallPropertyType(value.prop_type()).into()),
        }
    }
}

pub struct AnsiPropertyContext {
    node: AnsiNodeBTreeEntry,
    tree: AnsiHeapTree,
}

impl AnsiPropertyContext {
    pub fn new(node: AnsiNodeBTreeEntry, tree: AnsiHeapTree) -> Self {
        Self { node, tree }
    }

    pub fn tree(&self) -> &AnsiHeapTree {
        &self.tree
    }

    pub fn properties<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_tree: &AnsiBlockBTree,
    ) -> io::Result<BTreeMap<PropertyTreeRecordKey, PropertyTreeRecordValue>> {
        Ok(self
            .tree
            .entries(f, encoding, block_tree)?
            .into_iter()
            .map(
                |entry: HeapTreeLeafEntry<PropertyTreeRecordKey, PropertyTreeRecordValue>| {
                    (
                        entry.key(),
                        PropertyTreeRecordValue::new(
                            entry.data().prop_type(),
                            entry.data().value(),
                        ),
                    )
                },
            )
            .collect())
    }

    pub fn read_property<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_btree: &AnsiBlockBTree,
        value: PropertyTreeRecordValue,
    ) -> io::Result<PropertyValue> {
        match value.value() {
            PropertyValueRecord::Heap(heap_id) => {
                let data = self
                    .tree
                    .heap()
                    .find_entry(heap_id, f, encoding, block_btree)?;
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, value.prop_type())
            }
            PropertyValueRecord::Node(sub_node_id) => {
                let sub_node =
                    self.node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(u32::from(
                            sub_node_id,
                        )))?;
                let block = block_btree.find_entry(f, u32::from(sub_node))?;
                let sub_node_tree = AnsiSubNodeTree::read(f, &block)?;
                let block = sub_node_tree.find_entry(f, block_btree, sub_node_id)?;
                let block = block_btree.find_entry(f, u32::from(block))?;
                let data_tree = AnsiDataTree::read(f, encoding, &block)?;
                let blocks: Vec<_> = data_tree.blocks(f, encoding, block_btree)?.collect();
                let data: Vec<_> = blocks
                    .iter()
                    .flat_map(|block| block.data())
                    .copied()
                    .collect();
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, value.prop_type())
            }
            small => small
                .small_value(value.prop_type())
                .ok_or(LtpError::InvalidSmallPropertyType(value.prop_type()).into()),
        }
    }
}
