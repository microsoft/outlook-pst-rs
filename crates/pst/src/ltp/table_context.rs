//! ## [Table Context (TC)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5e48be0d-a75a-4918-a277-50408ff96740)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    io::{self, Cursor, Read, Seek, Write},
};

use super::{heap::*, prop_context::*, prop_type::*, read_write::*, tree::*, *};
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

pub const LTP_ROW_ID_PROP_ID: u16 = 0x67F2;
pub const LTP_ROW_VERSION_PROP_ID: u16 = 0x67F3;

pub const fn existence_bitmap_size(column_count: usize) -> usize {
    column_count / 8 + if column_count % 8 == 0 { 0 } else { 1 }
}

pub const fn check_existence_bitmap(column: usize, existence_bitmap: &[u8]) -> LtpResult<bool> {
    if column >= existence_bitmap.len() * 8 {
        return Err(LtpError::InvalidTableContextColumnCount(column));
    }
    Ok(existence_bitmap[column / 8] & (1_u8 << (7 - (column % 8))) != 0)
}

/// [TCINFO](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/45b3a0c5-d6d6-4e02-aebf-13766ff693f0)
#[derive(Clone, Default, Debug)]
pub struct TableContextInfo {
    end_4byte_values: u16,
    end_2byte_values: u16,
    end_1byte_values: u16,
    end_existence_bitmap: u16,
    row_index: HeapId,
    rows: Option<NodeId>,
    deprecated_index: u32,
    columns: Vec<TableColumnDescriptor>,
}

impl TableContextInfo {
    pub fn new(
        end_4byte_values: u16,
        end_2byte_values: u16,
        end_1byte_values: u16,
        end_existence_bitmap: u16,
        row_index: HeapId,
        rows: Option<NodeId>,
        columns: Vec<TableColumnDescriptor>,
    ) -> LtpResult<Self> {
        if columns.len() > 0xFF {
            return Err(LtpError::InvalidTableContextColumnCount(columns.len()));
        }

        if end_4byte_values % 4 != 0 {
            return Err(LtpError::InvalidTableContext4ByteOffset(end_4byte_values));
        }

        if end_2byte_values % 2 != 0 || end_2byte_values < end_4byte_values {
            return Err(LtpError::InvalidTableContext2ByteOffset(end_2byte_values));
        }

        if end_1byte_values < end_2byte_values {
            return Err(LtpError::InvalidTableContext1ByteOffset(end_1byte_values));
        }

        if end_existence_bitmap < end_1byte_values
            || (end_existence_bitmap - end_1byte_values) as usize
                != existence_bitmap_size(columns.len())
        {
            return Err(LtpError::InvalidTableContextBitmaskOffset(
                end_existence_bitmap,
            ));
        }

        for column in columns.iter() {
            match (column.prop_type(), column.prop_id()) {
                (PropertyType::Integer32, LTP_ROW_ID_PROP_ID) => {
                    match (column.offset(), column.existence_bitmap_index()) {
                        (0, 0) => {}
                        _ => {
                            return Err(LtpError::InvalidTableContextRowIdColumn(
                                column.prop_id(),
                                column.prop_type(),
                            ));
                        }
                    }
                }
                (PropertyType::Integer32, LTP_ROW_VERSION_PROP_ID) => {
                    match (column.offset(), column.existence_bitmap_index()) {
                        (4, 1) => {}
                        _ => {
                            return Err(LtpError::InvalidTableContextRowIdColumn(
                                column.prop_id(),
                                column.prop_type(),
                            ));
                        }
                    }
                }
                _ => {}
            }

            match column.prop_type() {
                PropertyType::Integer16
                | PropertyType::Integer32
                | PropertyType::Floating32
                | PropertyType::Floating64
                | PropertyType::Currency
                | PropertyType::FloatingTime
                | PropertyType::ErrorCode
                | PropertyType::Boolean
                | PropertyType::Integer64
                | PropertyType::String8
                | PropertyType::Unicode
                | PropertyType::Time
                | PropertyType::Guid
                | PropertyType::Binary
                | PropertyType::Object
                | PropertyType::MultipleInteger16
                | PropertyType::MultipleInteger32
                | PropertyType::MultipleFloating32
                | PropertyType::MultipleFloating64
                | PropertyType::MultipleCurrency
                | PropertyType::MultipleFloatingTime
                | PropertyType::MultipleInteger64
                | PropertyType::MultipleString8
                | PropertyType::MultipleUnicode
                | PropertyType::MultipleTime
                | PropertyType::MultipleGuid
                | PropertyType::MultipleBinary => {}
                prop_type => {
                    return Err(LtpError::InvalidTableColumnPropertyType(prop_type));
                }
            }

            match (column.prop_type(), column.offset()) {
                (PropertyType::Boolean, offset)
                    if offset >= end_2byte_values && offset < end_1byte_values => {}
                (PropertyType::Integer16, offset)
                    if offset % 2 == 0
                        && offset >= end_4byte_values
                        && offset + 2 <= end_2byte_values => {}
                (
                    PropertyType::Integer32
                    | PropertyType::Floating32
                    | PropertyType::ErrorCode
                    | PropertyType::String8
                    | PropertyType::Unicode
                    | PropertyType::Guid
                    | PropertyType::Binary
                    | PropertyType::Object
                    | PropertyType::MultipleInteger16
                    | PropertyType::MultipleInteger32
                    | PropertyType::MultipleFloating32
                    | PropertyType::MultipleFloating64
                    | PropertyType::MultipleCurrency
                    | PropertyType::MultipleFloatingTime
                    | PropertyType::MultipleInteger64
                    | PropertyType::MultipleString8
                    | PropertyType::MultipleUnicode
                    | PropertyType::MultipleTime
                    | PropertyType::MultipleGuid
                    | PropertyType::MultipleBinary,
                    offset,
                ) if offset % 4 == 0 && offset + 4 <= end_4byte_values => {}
                (
                    PropertyType::Floating64
                    | PropertyType::Currency
                    | PropertyType::FloatingTime
                    | PropertyType::Integer64
                    | PropertyType::Time,
                    offset,
                ) if offset % 4 == 0 && offset + 8 <= end_4byte_values => {}
                (_, offset) => {
                    return Err(LtpError::InvalidTableColumnOffset(offset));
                }
            }

            match (column.prop_type(), column.size()) {
                (PropertyType::Boolean, 1) => {}
                (PropertyType::Integer16, 2) => {}
                (
                    PropertyType::Integer32
                    | PropertyType::Floating32
                    | PropertyType::ErrorCode
                    | PropertyType::String8
                    | PropertyType::Unicode
                    | PropertyType::Guid
                    | PropertyType::Binary
                    | PropertyType::Object
                    | PropertyType::MultipleInteger16
                    | PropertyType::MultipleInteger32
                    | PropertyType::MultipleFloating32
                    | PropertyType::MultipleFloating64
                    | PropertyType::MultipleCurrency
                    | PropertyType::MultipleFloatingTime
                    | PropertyType::MultipleInteger64
                    | PropertyType::MultipleString8
                    | PropertyType::MultipleUnicode
                    | PropertyType::MultipleTime
                    | PropertyType::MultipleGuid
                    | PropertyType::MultipleBinary,
                    4,
                ) => {}
                (
                    PropertyType::Floating64
                    | PropertyType::Currency
                    | PropertyType::FloatingTime
                    | PropertyType::Integer64
                    | PropertyType::Time,
                    8,
                ) => {}
                (_, size) => {
                    return Err(LtpError::InvalidTableColumnSize(size));
                }
            }

            if usize::from(column.existence_bitmap_index()) > columns.len() {
                return Err(LtpError::InvalidTableColumnBitmaskOffset(
                    column.existence_bitmap_index(),
                ));
            }
        }

        Ok(Self {
            end_4byte_values,
            end_2byte_values,
            end_1byte_values,
            end_existence_bitmap,
            row_index,
            rows,
            deprecated_index: 0,
            columns,
        })
    }

    pub fn end_4byte_values(&self) -> u16 {
        self.end_4byte_values
    }

    pub fn end_2byte_values(&self) -> u16 {
        self.end_2byte_values
    }

    pub fn end_1byte_values(&self) -> u16 {
        self.end_1byte_values
    }

    pub fn end_existence_bitmap(&self) -> u16 {
        self.end_existence_bitmap
    }

    pub fn columns(&self) -> &[TableColumnDescriptor] {
        &self.columns
    }
}

impl TableContextReadWrite for TableContextInfo {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        // bType
        let signature = HeapNodeType::try_from(f.read_u8()?)?;
        if signature != HeapNodeType::Table {
            return Err(LtpError::InvalidTableContextHeapTreeNodeType(signature).into());
        }

        // cCols
        let column_count = f.read_u8()?;

        // rgib
        let end_4byte_values = f.read_u16::<LittleEndian>()?;
        let end_2byte_values = f.read_u16::<LittleEndian>()?;
        let end_1byte_values = f.read_u16::<LittleEndian>()?;
        let end_existence_bitmap = f.read_u16::<LittleEndian>()?;

        // hidRowIndex
        let row_index = HeapId::read(f)?;

        // hnidRows
        let rows = NodeId::read(f)?;
        let rows = if u32::from(rows) == 0 {
            None
        } else {
            Some(rows)
        };

        // hidIndex
        let deprecated_index = f.read_u32::<LittleEndian>()?;

        // rgTCOLDESC
        let mut columns = Vec::with_capacity(usize::from(column_count));
        for _ in 0..column_count {
            columns.push(TableColumnDescriptor::read(f)?);
        }

        Ok(Self {
            deprecated_index,
            ..Self::new(
                end_4byte_values,
                end_2byte_values,
                end_1byte_values,
                end_existence_bitmap,
                row_index,
                rows,
                columns,
            )?
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        if self.columns.len() > 0xFF {
            return Err(LtpError::InvalidTableContextColumnCount(self.columns.len()).into());
        }

        // bType
        f.write_u8(HeapNodeType::Table as u8)?;

        // cCols
        f.write_u8(self.columns.len() as u8)?;

        // rgib
        f.write_u16::<LittleEndian>(self.end_4byte_values)?;
        f.write_u16::<LittleEndian>(self.end_2byte_values)?;
        f.write_u16::<LittleEndian>(self.end_1byte_values)?;
        f.write_u16::<LittleEndian>(self.end_existence_bitmap)?;

        // hidRowIndex
        self.row_index.write(f)?;

        // hnidRows
        self.rows.unwrap_or_default().write(f)?;

        // hidIndex
        f.write_u32::<LittleEndian>(self.deprecated_index)?;

        // rgTCOLDESC
        for column in &self.columns {
            column.write(f)?;
        }

        Ok(())
    }
}

/// [TCOLDESC](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/3a2f63cf-bb40-4559-910c-e55ec43d9cbb)
#[derive(Clone, Copy, Default, Debug)]
pub struct TableColumnDescriptor {
    prop_type: PropertyType,
    prop_id: u16,
    offset: u16,
    size: u8,
    existence_bitmap_index: u8,
}

impl TableColumnDescriptor {
    pub fn new(
        prop_type: PropertyType,
        prop_id: u16,
        offset: u16,
        size: u8,
        existence_bitmap_index: u8,
    ) -> Self {
        Self {
            prop_type,
            prop_id,
            offset,
            size,
            existence_bitmap_index,
        }
    }

    pub fn prop_type(&self) -> PropertyType {
        self.prop_type
    }

    pub fn prop_id(&self) -> u16 {
        self.prop_id
    }

    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub fn size(&self) -> u8 {
        self.size
    }

    pub fn existence_bitmap_index(&self) -> u8 {
        self.existence_bitmap_index
    }
}

impl TableContextReadWrite for TableColumnDescriptor {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let prop_type = PropertyType::try_from(f.read_u16::<LittleEndian>()?)?;
        let prop_id = f.read_u16::<LittleEndian>()?;
        let offset = f.read_u16::<LittleEndian>()?;
        let size = f.read_u8()?;
        let existence_bitmap_index = f.read_u8()?;

        Ok(Self {
            prop_type,
            prop_id,
            offset,
            size,
            existence_bitmap_index,
        })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.prop_type as u16)?;
        f.write_u16::<LittleEndian>(self.prop_id)?;
        f.write_u16::<LittleEndian>(self.offset)?;
        f.write_u8(self.size)?;
        f.write_u8(self.existence_bitmap_index)?;

        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub struct TableRowId {
    id: u32,
}

impl TableRowId {
    pub fn new(id: u32) -> Self {
        Self { id }
    }
}

impl From<TableRowId> for u32 {
    fn from(value: TableRowId) -> Self {
        value.id
    }
}

impl HeapTreeEntryKey for TableRowId {
    const SIZE: u8 = 4;
}

impl HeapNodeReadWrite for TableRowId {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let id = f.read_u32::<LittleEndian>()?;
        Ok(Self { id })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.id)
    }
}

pub trait TableRowIndex: HeapTreeEntryValue + Copy + Into<u32> {}

#[derive(Clone, Copy, Default, Debug)]
pub struct UnicodeTableRowIndex {
    index: u32,
}

impl UnicodeTableRowIndex {
    pub fn new(index: u32) -> Self {
        Self { index }
    }
}

impl From<UnicodeTableRowIndex> for u32 {
    fn from(value: UnicodeTableRowIndex) -> Self {
        value.index
    }
}

impl HeapTreeEntryValue for UnicodeTableRowIndex {
    const SIZE: u8 = 4;
}

impl HeapNodeReadWrite for UnicodeTableRowIndex {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let index = f.read_u32::<LittleEndian>()?;
        Ok(Self { index })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.index)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AnsiTableRowIndex {
    index: u16,
}

impl AnsiTableRowIndex {
    pub fn new(index: u16) -> Self {
        Self { index }
    }
}

impl From<AnsiTableRowIndex> for u32 {
    fn from(value: AnsiTableRowIndex) -> Self {
        u32::from(value.index)
    }
}

impl HeapTreeEntryValue for AnsiTableRowIndex {
    const SIZE: u8 = 2;
}

impl HeapNodeReadWrite for AnsiTableRowIndex {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let index = f.read_u16::<LittleEndian>()?;
        Ok(Self { index })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.index)
    }
}

/// [TCROWID](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e20b5cf4-ea56-48b8-a8fa-e086c9b862ca)
pub type UnicodeTableRowIdRecord = HeapTreeLeafEntry<TableRowId, UnicodeTableRowIndex>;

/// [TCROWID](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/e20b5cf4-ea56-48b8-a8fa-e086c9b862ca)
pub type AnsiTableRowIdRecord = HeapTreeLeafEntry<TableRowId, AnsiTableRowIndex>;

#[derive(Clone, Debug)]
pub enum TableRowColumnValue {
    Small(PropertyValue),
    Heap(HeapId),
    Node(NodeId),
}

/// [Row Data Format](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/c48fa6b4-bfd4-49d7-80f8-8718bc4bcddc)
pub struct TableRowData {
    id: TableRowId,
    unique: u32,
    align_4byte: Vec<u8>,
    align_2byte: Vec<u8>,
    align_1byte: Vec<u8>,
    existence_bitmap: Vec<u8>,
}

impl TableRowData {
    pub fn new(
        id: TableRowId,
        unique: u32,
        align_4byte: Vec<u8>,
        align_2byte: Vec<u8>,
        align_1byte: Vec<u8>,
        existence_bitmap: Vec<u8>,
    ) -> Self {
        Self {
            id,
            unique,
            align_4byte,
            align_2byte,
            align_1byte,
            existence_bitmap,
        }
    }

    pub fn id(&self) -> TableRowId {
        self.id
    }

    pub fn unique(&self) -> u32 {
        self.unique
    }

    pub fn columns(
        &self,
        context: &TableContextInfo,
    ) -> io::Result<Vec<Option<TableRowColumnValue>>> {
        context
            .columns()
            .iter()
            .map(|column| {
                let existence_bit = column.existence_bitmap_index() as usize;
                if !check_existence_bitmap(existence_bit, &self.existence_bitmap)? {
                    return Ok(None);
                }

                match (column.prop_type(), column.offset(), column.size()) {
                    (PropertyType::Null, _, 0) => Ok(None),
                    (PropertyType::Integer16, offset, 2) => {
                        let mut cursor = self.read_2byte_offset(context, offset)?;
                        let value = cursor.read_i16::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Integer16(
                            value,
                        ))))
                    }
                    (PropertyType::Integer32, 0, 4) => Ok(Some(TableRowColumnValue::Small(
                        PropertyValue::Integer32(u32::from(self.id) as i32),
                    ))),
                    (PropertyType::Integer32, 4, 4) => Ok(Some(TableRowColumnValue::Small(
                        PropertyValue::Integer32(self.unique as i32),
                    ))),
                    (PropertyType::Integer32, offset, 4) => {
                        let mut cursor = self.read_4byte_offset(offset)?;
                        let value = cursor.read_i32::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Integer32(
                            value,
                        ))))
                    }
                    (PropertyType::Floating32, offset, 4) => {
                        let mut cursor = self.read_4byte_offset(offset)?;
                        let value = cursor.read_f32::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Floating32(
                            value,
                        ))))
                    }
                    (PropertyType::Floating64, offset, 8) => {
                        let mut cursor = self.read_8byte_offset(offset)?;
                        let value = cursor.read_f64::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Floating64(
                            value,
                        ))))
                    }
                    (PropertyType::Currency, offset, 8) => {
                        let mut cursor = self.read_8byte_offset(offset)?;
                        let value = cursor.read_i64::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Currency(
                            value,
                        ))))
                    }
                    (PropertyType::FloatingTime, offset, 8) => {
                        let mut cursor = self.read_8byte_offset(offset)?;
                        let value = cursor.read_f64::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(
                            PropertyValue::FloatingTime(value),
                        )))
                    }
                    (PropertyType::ErrorCode, offset, 4) => {
                        let mut cursor = self.read_4byte_offset(offset)?;
                        let value = cursor.read_i32::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::ErrorCode(
                            value,
                        ))))
                    }
                    (PropertyType::Boolean, offset, 1) => {
                        let value = self.read_1byte_offset(context, offset)?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Boolean(
                            match value {
                                0x00 => false,
                                0x01 => true,
                                _ => {
                                    return Err(
                                        LtpError::InvalidTableColumnBooleanValue(value).into()
                                    )
                                }
                            },
                        ))))
                    }
                    (PropertyType::Integer64, offset, 8) => {
                        let mut cursor = self.read_8byte_offset(offset)?;
                        let value = cursor.read_i64::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Integer64(
                            value,
                        ))))
                    }
                    (PropertyType::Time, offset, 8) => {
                        let mut cursor = self.read_8byte_offset(offset)?;
                        let value = cursor.read_i64::<LittleEndian>()?;
                        Ok(Some(TableRowColumnValue::Small(PropertyValue::Time(value))))
                    }
                    (
                        PropertyType::String8
                        | PropertyType::Unicode
                        | PropertyType::Guid
                        | PropertyType::Binary
                        | PropertyType::Object
                        | PropertyType::MultipleInteger16
                        | PropertyType::MultipleInteger32
                        | PropertyType::MultipleFloating32
                        | PropertyType::MultipleFloating64
                        | PropertyType::MultipleCurrency
                        | PropertyType::MultipleFloatingTime
                        | PropertyType::MultipleInteger64
                        | PropertyType::MultipleString8
                        | PropertyType::MultipleUnicode
                        | PropertyType::MultipleTime
                        | PropertyType::MultipleGuid
                        | PropertyType::MultipleBinary,
                        offset,
                        4,
                    ) => {
                        let mut cursor = self.read_4byte_offset(offset)?;
                        let node_id = NodeId::from(cursor.read_u32::<LittleEndian>()?);
                        let value = match node_id.id_type() {
                            Ok(NodeIdType::HeapNode) => {
                                TableRowColumnValue::Heap(HeapId::from(u32::from(node_id)))
                            }
                            _ => TableRowColumnValue::Node(node_id),
                        };
                        Ok(Some(value))
                    }
                    (_, _, size) => Err(LtpError::InvalidTableColumnSize(size).into()),
                }
            })
            .collect()
    }

    fn read_1byte_offset(&self, context: &TableContextInfo, offset: u16) -> LtpResult<u8> {
        if offset < context.end_2byte_values() {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        let offset_1byte = (offset - context.end_2byte_values()) as usize;
        if offset_1byte >= self.align_1byte.len() {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        Ok(self.align_1byte[offset_1byte])
    }

    fn read_2byte_offset(&self, context: &TableContextInfo, offset: u16) -> LtpResult<&[u8]> {
        if offset < context.end_4byte_values() {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        let offset_2byte = (offset - context.end_4byte_values()) as usize;
        if offset_2byte + 2 > self.align_2byte.len() {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        Ok(&self.align_2byte[offset_2byte..offset_2byte + 2])
    }

    fn read_4byte_offset(&self, offset: u16) -> LtpResult<&[u8]> {
        if offset < 8 {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        let offset_4byte = (offset - 8) as usize;
        if offset_4byte + 4 > self.align_4byte.len() {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        Ok(&self.align_4byte[offset_4byte..offset_4byte + 4])
    }

    fn read_8byte_offset(&self, offset: u16) -> LtpResult<&[u8]> {
        if offset < 8 {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        let offset_4byte = (offset - 8) as usize;
        if offset_4byte + 8 > self.align_4byte.len() {
            return Err(LtpError::InvalidTableColumnOffset(offset));
        }
        Ok(&self.align_4byte[offset_4byte..offset_4byte + 8])
    }
}

impl TableRowReadWrite for TableRowData {
    fn read(f: &mut dyn Read, context: &TableContextInfo) -> io::Result<Self> {
        // dwRowID
        let id = TableRowId {
            id: f.read_u32::<LittleEndian>()?,
        };

        // rgdwData
        let unique = f.read_u32::<LittleEndian>()?;
        let mut align_4byte = vec![0; context.end_4byte_values() as usize - 8];
        f.read_exact(align_4byte.as_mut_slice())?;

        // rgwData
        let mut align_2byte =
            vec![0; (context.end_2byte_values() - context.end_4byte_values()) as usize];
        f.read_exact(align_2byte.as_mut_slice())?;

        // rgbData
        let mut align_1byte =
            vec![0; (context.end_1byte_values() - context.end_2byte_values()) as usize];
        f.read_exact(align_1byte.as_mut_slice())?;

        // rgbCEB
        let mut existence_bitmap = vec![0; existence_bitmap_size(context.columns().len())];
        f.read_exact(existence_bitmap.as_mut_slice())?;

        Ok(Self::new(
            id,
            unique,
            align_4byte,
            align_2byte,
            align_1byte,
            existence_bitmap,
        ))
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(u32::from(self.id))?;
        f.write_u32::<LittleEndian>(self.unique)?;
        f.write_all(&self.align_4byte)?;
        f.write_all(&self.align_2byte)?;
        f.write_all(&self.align_1byte)?;
        f.write_all(&self.existence_bitmap)
    }
}

pub struct UnicodeTableContext {
    node: UnicodeNodeBTreeEntry,
    context: TableContextInfo,
    heap: UnicodeHeapNode,
    row_index: BTreeMap<TableRowId, UnicodeTableRowIndex>,
    rows: Vec<TableRowData>,
}

impl UnicodeTableContext {
    pub fn context(&self) -> &TableContextInfo {
        &self.context
    }

    pub fn read<R: Read + Seek>(
        f: &mut R,
        encoding: NdbCryptMethod,
        block_btree: &UnicodeBlockBTree,
        node: UnicodeNodeBTreeEntry,
    ) -> io::Result<Self> {
        let data = node.data();
        let block = block_btree.find_entry(f, u64::from(data))?;
        let heap = UnicodeHeapNode::new(UnicodeDataTree::read(f, encoding, &block)?);
        let header = heap.header(f, encoding, block_btree)?;

        let cursor = heap.find_entry(header.user_root(), f, encoding, block_btree)?;
        let context = TableContextInfo::read(&mut cursor.as_slice())?;

        let rows = if let Some(rows) = context.rows {
            match rows.id_type() {
                Ok(NodeIdType::HeapNode) => vec![heap.find_entry(
                    HeapId::from(u32::from(rows)),
                    f,
                    encoding,
                    block_btree,
                )?],
                _ => {
                    let sub_node = node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(u32::from(rows)))?;
                    let block = block_btree.find_entry(f, u64::from(sub_node))?;
                    let sub_node_tree = UnicodeSubNodeTree::read(f, &block)?;
                    let block = sub_node_tree.find_entry(f, block_btree, rows)?;
                    let block = block_btree.find_entry(f, u64::from(block))?;
                    let data_tree = UnicodeDataTree::read(f, encoding, &block)?;
                    let blocks: Vec<_> = data_tree.blocks(f, encoding, block_btree)?.collect();
                    blocks.iter().map(|block| block.data().to_vec()).collect()
                }
            }
            .into_iter()
            .map(|data| {
                let row_count = data.len() / context.end_existence_bitmap() as usize;
                let mut cursor = Cursor::new(data);
                let mut rows = Vec::with_capacity(row_count);
                for _ in 0..row_count {
                    let row = TableRowData::read(&mut cursor, &context)?;
                    rows.push(row);
                }
                Ok(rows)
            })
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect()
        } else {
            Default::default()
        };

        let row_index_tree = UnicodeHeapTree::new(heap, context.row_index);
        let row_index = row_index_tree
            .entries::<_, TableRowId, UnicodeTableRowIndex>(f, encoding, block_btree)?
            .into_iter()
            .map(|entry| (entry.key(), entry.data()))
            .collect();
        let heap = row_index_tree.into();

        Ok(Self {
            node,
            context,
            heap,
            row_index,
            rows,
        })
    }

    pub fn rows_matrix(&self) -> impl Iterator<Item = &TableRowData> {
        self.rows.iter()
    }

    pub fn find_row(&self, id: TableRowId) -> LtpResult<&TableRowData> {
        let index = self
            .row_index
            .get(&id)
            .ok_or(LtpError::TableRowIdNotFound(u32::from(id)))?;
        Ok(&self.rows[u32::from(*index) as usize])
    }

    pub fn read_column<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_btree: &UnicodeBlockBTree,
        value: &TableRowColumnValue,
        prop_type: PropertyType,
    ) -> io::Result<PropertyValue> {
        match value {
            TableRowColumnValue::Small(small) => Ok(small.clone()),
            TableRowColumnValue::Heap(heap_id) => {
                let data = self.heap.find_entry(*heap_id, f, encoding, block_btree)?;
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, prop_type)
            }
            TableRowColumnValue::Node(sub_node_id) => {
                let sub_node =
                    self.node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(u32::from(
                            *sub_node_id,
                        )))?;
                let block = block_btree.find_entry(f, u64::from(sub_node))?;
                let sub_node_tree = UnicodeSubNodeTree::read(f, &block)?;
                let block = sub_node_tree.find_entry(f, block_btree, *sub_node_id)?;
                let block = block_btree.find_entry(f, u64::from(block))?;
                let data_tree = UnicodeDataTree::read(f, encoding, &block)?;
                let blocks: Vec<_> = data_tree.blocks(f, encoding, block_btree)?.collect();
                let data: Vec<_> = blocks
                    .iter()
                    .flat_map(|block| block.data())
                    .copied()
                    .collect();
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, prop_type)
            }
        }
    }
}

pub struct AnsiTableContext {
    node: AnsiNodeBTreeEntry,
    context: TableContextInfo,
    heap: AnsiHeapNode,
    row_index: BTreeMap<TableRowId, AnsiTableRowIndex>,
    rows: Vec<TableRowData>,
}

impl AnsiTableContext {
    pub fn context(&self) -> &TableContextInfo {
        &self.context
    }

    pub fn read<R: Read + Seek>(
        f: &mut R,
        encoding: NdbCryptMethod,
        block_btree: &AnsiBlockBTree,
        node: AnsiNodeBTreeEntry,
    ) -> io::Result<Self> {
        let data = node.data();
        let block = block_btree.find_entry(f, u32::from(data))?;
        let heap = AnsiHeapNode::new(AnsiDataTree::read(f, encoding, &block)?);
        let header = heap.header(f, encoding, block_btree)?;

        let cursor = heap.find_entry(header.user_root(), f, encoding, block_btree)?;
        let context = TableContextInfo::read(&mut cursor.as_slice())?;

        let row_matrix = if let Some(rows) = context.rows {
            match rows.id_type() {
                Ok(NodeIdType::HeapNode) => {
                    heap.find_entry(HeapId::from(u32::from(rows)), f, encoding, block_btree)?
                }
                _ => {
                    let sub_node = node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(u32::from(rows)))?;
                    let block = block_btree.find_entry(f, u32::from(sub_node))?;
                    let sub_node_tree = AnsiSubNodeTree::read(f, &block)?;
                    let block = sub_node_tree.find_entry(f, block_btree, rows)?;
                    let block = block_btree.find_entry(f, u32::from(block))?;
                    let data_tree = AnsiDataTree::read(f, encoding, &block)?;
                    let blocks: Vec<_> = data_tree.blocks(f, encoding, block_btree)?.collect();
                    blocks
                        .iter()
                        .flat_map(|block| block.data())
                        .copied()
                        .collect()
                }
            }
        } else {
            Default::default()
        };
        let row_count = row_matrix.len() / context.end_existence_bitmap() as usize;
        let mut cursor = Cursor::new(&row_matrix);
        let mut rows = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            let row = TableRowData::read(&mut cursor, &context)?;
            rows.push(row);
        }

        let row_index_tree = AnsiHeapTree::new(heap, context.row_index);
        let row_index = row_index_tree
            .entries::<_, TableRowId, AnsiTableRowIndex>(f, encoding, block_btree)?
            .into_iter()
            .map(|entry| (entry.key(), entry.data()))
            .collect();
        let heap = row_index_tree.into();

        Ok(Self {
            node,
            context,
            heap,
            row_index,
            rows,
        })
    }

    pub fn rows_matrix(&self) -> impl Iterator<Item = &TableRowData> {
        self.rows.iter()
    }

    pub fn find_row(&self, id: TableRowId) -> LtpResult<&TableRowData> {
        let index = self
            .row_index
            .get(&id)
            .ok_or(LtpError::TableRowIdNotFound(u32::from(id)))?;
        Ok(&self.rows[u32::from(*index) as usize])
    }

    pub fn read_column<R: Read + Seek>(
        &self,
        f: &mut R,
        encoding: NdbCryptMethod,
        block_btree: &AnsiBlockBTree,
        value: &TableRowColumnValue,
        prop_type: PropertyType,
    ) -> io::Result<PropertyValue> {
        match value {
            TableRowColumnValue::Small(small) => Ok(small.clone()),
            TableRowColumnValue::Heap(heap_id) => {
                let data = self.heap.find_entry(*heap_id, f, encoding, block_btree)?;
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, prop_type)
            }
            TableRowColumnValue::Node(sub_node_id) => {
                let sub_node =
                    self.node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(u32::from(
                            *sub_node_id,
                        )))?;
                let block = block_btree.find_entry(f, u32::from(sub_node))?;
                let sub_node_tree = AnsiSubNodeTree::read(f, &block)?;
                let block = sub_node_tree.find_entry(f, block_btree, *sub_node_id)?;
                let block = block_btree.find_entry(f, u32::from(block))?;
                let data_tree = AnsiDataTree::read(f, encoding, &block)?;
                let blocks: Vec<_> = data_tree.blocks(f, encoding, block_btree)?.collect();
                let data: Vec<_> = blocks
                    .iter()
                    .flat_map(|block| block.data())
                    .copied()
                    .collect();
                let mut cursor = Cursor::new(data);
                PropertyValue::read(&mut cursor, prop_type)
            }
        }
    }
}
