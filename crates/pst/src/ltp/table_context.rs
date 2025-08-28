//! ## [Table Context (TC)](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5e48be0d-a75a-4918-a277-50408ff96740)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt::Debug,
    io::{self, Cursor, Read, Write},
    marker::PhantomData,
    rc::Rc,
};

use super::{heap::*, prop_context::*, prop_type::*, read_write::*, tree::*, *};
use crate::{
    messaging::{
        read_write::StoreReadWrite,
        store::{AnsiStore, UnicodeStore},
    },
    ndb::{
        block::{Block, DataBlockCache, DataTree, IntermediateTreeBlock, SubNodeTree},
        block_id::BlockId,
        block_ref::BlockRef,
        header::Header,
        node_id::{NodeId, NodeIdType},
        page::{
            AnsiNodeBTreeEntry, BlockBTreeEntry, NodeBTreeEntry, RootBTree, UnicodeNodeBTreeEntry,
        },
        read_write::*,
        root::Root,
    },
    AnsiPstFile, PstFile, PstFileLock, UnicodePstFile,
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
    _deprecated_index: u32,
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
            _deprecated_index: 0,
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

impl TableContextInfoReadWrite for TableContextInfo {
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
        let _deprecated_index = f.read_u32::<LittleEndian>()?;

        // rgTCOLDESC
        let mut columns = Vec::with_capacity(usize::from(column_count));
        for _ in 0..column_count {
            columns.push(TableColumnDescriptor::read(f)?);
        }

        Ok(Self {
            _deprecated_index,
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
        f.write_u32::<LittleEndian>(self._deprecated_index)?;

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

impl TableColumnDescriptorReadWrite for TableColumnDescriptor {
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

impl HeapNodePageReadWrite for TableRowId {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let id = f.read_u32::<LittleEndian>()?;
        Ok(Self { id })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u32::<LittleEndian>(self.id)
    }
}

trait TableRowIndex<Pst>: HeapTreeEntryValue + HeapNodePageReadWrite + Copy
where
    Pst: PstFile,
    u32: From<Self>,
{
    type Index: Copy;
}

#[derive(Clone, Copy, Default, Debug)]
pub struct UnicodeTableRowIndex {
    index: u32,
}

impl TableRowIndex<UnicodePstFile> for UnicodeTableRowIndex {
    type Index = u32;
}

impl From<UnicodeTableRowIndex> for u32 {
    fn from(value: UnicodeTableRowIndex) -> Self {
        value.index
    }
}

impl HeapTreeEntryValue for UnicodeTableRowIndex {
    const SIZE: u8 = 4;
}

impl HeapNodePageReadWrite for UnicodeTableRowIndex {
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

impl TableRowIndex<AnsiPstFile> for AnsiTableRowIndex {
    type Index = u16;
}

impl From<AnsiTableRowIndex> for u32 {
    fn from(value: AnsiTableRowIndex) -> Self {
        u32::from(value.index)
    }
}

impl HeapTreeEntryValue for AnsiTableRowIndex {
    const SIZE: u8 = 2;
}

impl HeapNodePageReadWrite for AnsiTableRowIndex {
    fn read(f: &mut dyn Read) -> io::Result<Self> {
        let index = f.read_u16::<LittleEndian>()?;
        Ok(Self { index })
    }

    fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        f.write_u16::<LittleEndian>(self.index)
    }
}

trait TableRowIndexTree<Pst>: HeapTreeReadWrite<Pst, Key = TableRowId, Value = Self::RowIndex>
where
    Pst: PstFile,
    u32: From<Self::RowIndex>,
{
    type RowIndex: TableRowIndex<Pst>;
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

pub trait TableContext {
    fn context(&self) -> &TableContextInfo;
    fn rows_matrix<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = &'a TableRowData>>;
    fn find_row(&self, id: TableRowId) -> LtpResult<&TableRowData>;
    fn read_column(
        &self,
        value: &TableRowColumnValue,
        prop_type: PropertyType,
    ) -> io::Result<PropertyValue>;
}

struct TableContextInner<Pst, RowIndex, RowIndexTree>
where
    Pst: PstFile,
    RowIndex: TableRowIndex<Pst>,
    RowIndexTree: TableRowIndexTree<Pst, RowIndex = RowIndex>,
    u32: From<RowIndex>,
{
    store: Rc<<Pst as PstFile>::Store>,
    node: <Pst as PstFile>::NodeBTreeEntry,
    context: TableContextInfo,
    heap: <Pst as PstFile>::HeapNode,
    row_index: BTreeMap<TableRowId, RowIndex>,
    rows: Vec<TableRowData>,
    block_cache: RefCell<DataBlockCache<Pst>>,
    _phantom: PhantomData<RowIndexTree>,
}

impl<Pst, RowIndex, RowIndexTree> TableContextInner<Pst, RowIndex, RowIndexTree>
where
    Pst: PstFile + PstFileLock<Pst>,
    <Pst as PstFile>::BlockId: BlockId<Index = <Pst as PstFile>::BTreeKey> + BlockIdReadWrite,
    <Pst as PstFile>::ByteIndex: ByteIndexReadWrite,
    <Pst as PstFile>::BTreeKey: BTreePageKeyReadWrite,
    <Pst as PstFile>::BlockBTree: RootBTreeReadWrite,
    <<Pst as PstFile>::BlockBTree as RootBTree>::Entry: BTreeEntryReadWrite,
    <<Pst as PstFile>::BlockBTree as RootBTree>::IntermediatePage:
        RootBTreeIntermediatePageReadWrite<
            Pst,
            <<Pst as PstFile>::BlockBTree as RootBTree>::Entry,
            <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage,
        >,
    <<Pst as PstFile>::BlockBTree as RootBTree>::LeafPage:
        RootBTreeLeafPageReadWrite<Pst> + BTreePageReadWrite,
    <Pst as PstFile>::BlockTrailer: BlockTrailerReadWrite,
    <Pst as PstFile>::DataTreeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::DataTreeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::DataBlock: BlockReadWrite + Clone,
    <Pst as PstFile>::SubNodeTreeBlockHeader: IntermediateTreeHeaderReadWrite,
    <Pst as PstFile>::SubNodeTreeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::SubNodeTreeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::SubNodeBlock: IntermediateTreeBlockReadWrite,
    <<Pst as PstFile>::SubNodeBlock as IntermediateTreeBlock>::Entry:
        IntermediateTreeEntryReadWrite,
    <Pst as PstFile>::HeapNode: HeapNodeReadWrite<Pst> + From<RowIndexTree>,
    <Pst as PstFile>::Store: StoreReadWrite<Pst>,
    RowIndex: TableRowIndex<Pst>,
    RowIndexTree: TableRowIndexTree<Pst, RowIndex = RowIndex>,
    u32: From<RowIndex>,
{
    fn read(
        store: Rc<<Pst as PstFile>::Store>,
        node: <Pst as PstFile>::NodeBTreeEntry,
    ) -> io::Result<Self> {
        let mut file = store
            .pst()
            .reader()
            .lock()
            .map_err(|_| LtpError::FailedToLockFile)?;
        let file = &mut *file;

        let header = store.pst().header();
        let encoding = header.crypt_method();
        let mut page_cache = store.pst().block_cache();
        let block_btree = <<Pst as PstFile>::BlockBTree as RootBTreeReadWrite>::read(
            file,
            *header.root().block_btree(),
        )?;

        let data = node.data();
        let heap = <<Pst as PstFile>::HeapNode as HeapNodeReadWrite<Pst>>::read(
            file,
            &block_btree,
            &mut page_cache,
            encoding,
            data.search_key(),
        )?;
        let header = HeapNode::header(&heap)?;
        let mut block_cache: DataBlockCache<Pst> = Default::default();

        let mut cursor = Cursor::new(heap.find_entry(header.user_root())?);
        let context = TableContextInfo::read(&mut cursor)?;

        let rows = if let Some(rows) = context.rows {
            match rows.id_type() {
                Ok(NodeIdType::HeapNode) => {
                    let rows: u32 = rows.into();
                    vec![heap.find_entry(HeapId::from(rows))?.to_vec()]
                }
                _ => {
                    let sub_node = node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(rows.into()))?;
                    let block =
                        block_btree.find_entry(file, sub_node.search_key(), &mut page_cache)?;
                    let sub_node_tree = SubNodeTree::<Pst>::read(file, &block)?;
                    let block =
                        sub_node_tree.find_entry(file, &block_btree, rows, &mut page_cache)?;
                    let block =
                        block_btree.find_entry(file, block.search_key(), &mut page_cache)?;
                    let data_tree = match block_cache.remove(&block.block().block()) {
                        Some(data_tree) => data_tree,
                        None => DataTree::read(file, encoding, &block)?,
                    };
                    let result = data_tree
                        .blocks(
                            file,
                            encoding,
                            &block_btree,
                            &mut page_cache,
                            &mut block_cache,
                        )
                        .map(|blocks| {
                            blocks
                                .map(|block| block.data().to_vec())
                                .collect::<Vec<_>>()
                        });
                    block_cache.insert(block.block().block(), data_tree);
                    result?
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

        let row_index_tree = RowIndexTree::new(heap, context.row_index);
        let row_index = row_index_tree
            .entries()?
            .into_iter()
            .map(|entry| (entry.key(), entry.data()))
            .collect();
        let heap = row_index_tree.into();

        Ok(Self {
            store: store.clone(),
            node,
            context,
            heap,
            row_index,
            rows,
            block_cache: RefCell::new(block_cache),
            _phantom: PhantomData,
        })
    }

    fn read_column(
        &self,
        value: &TableRowColumnValue,
        prop_type: PropertyType,
    ) -> io::Result<PropertyValue> {
        match value {
            TableRowColumnValue::Small(small) => Ok(small.clone()),
            TableRowColumnValue::Heap(heap_id) => {
                let data = self.heap.find_entry(*heap_id)?;
                let mut cursor = Cursor::new(data);
                PropertyValueReadWrite::read(&mut cursor, prop_type)
            }
            TableRowColumnValue::Node(sub_node_id) => {
                let mut file = self
                    .store
                    .pst()
                    .reader()
                    .lock()
                    .map_err(|_| LtpError::FailedToLockFile)?;
                let file = &mut *file;

                let encoding = self.store.pst().header().crypt_method();
                let block_btree = self.store.block_btree();
                let mut page_cache = self.store.pst().block_cache();

                let sub_node =
                    self.node
                        .sub_node()
                        .ok_or(LtpError::PropertySubNodeValueNotFound(
                            (*sub_node_id).into(),
                        ))?;
                let block = block_btree.find_entry(file, sub_node.search_key(), &mut page_cache)?;
                let sub_node_tree = SubNodeTree::<Pst>::read(file, &block)?;
                let block =
                    sub_node_tree.find_entry(file, block_btree, *sub_node_id, &mut page_cache)?;
                let block = block_btree.find_entry(file, block.search_key(), &mut page_cache)?;
                let mut block_cache = self.block_cache.borrow_mut();
                let data_tree = match block_cache.remove(&block.block().block()) {
                    Some(data_tree) => data_tree,
                    None => DataTree::read(file, encoding, &block)?,
                };
                let result = data_tree
                    .reader(
                        file,
                        encoding,
                        block_btree,
                        &mut page_cache,
                        &mut block_cache,
                    )
                    .and_then(|mut r| PropertyValueReadWrite::read(&mut r, prop_type));
                block_cache.insert(block.block().block(), data_tree);
                result
            }
        }
    }
}

type UnicodeRowIndexTree = UnicodeHeapTree<TableRowId, UnicodeTableRowIndex>;

impl TableRowIndexTree<UnicodePstFile> for UnicodeRowIndexTree {
    type RowIndex = UnicodeTableRowIndex;
}

pub struct UnicodeTableContext {
    inner: TableContextInner<UnicodePstFile, UnicodeTableRowIndex, UnicodeRowIndexTree>,
}

impl UnicodeTableContext {
    pub fn read(
        store: Rc<UnicodeStore>,
        node: UnicodeNodeBTreeEntry,
    ) -> io::Result<Rc<dyn TableContext>> {
        <Self as TableContextReadWrite<UnicodePstFile>>::read(store, node)
    }
}

impl TableContext for UnicodeTableContext {
    fn context(&self) -> &TableContextInfo {
        &self.inner.context
    }

    fn rows_matrix<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = &'a TableRowData>> {
        Box::new(self.inner.rows.iter())
    }

    fn find_row(&self, id: TableRowId) -> LtpResult<&TableRowData> {
        let index = self
            .inner
            .row_index
            .get(&id)
            .ok_or(LtpError::TableRowIdNotFound(u32::from(id)))?;
        Ok(&self.inner.rows[u32::from(*index) as usize])
    }

    fn read_column(
        &self,
        value: &TableRowColumnValue,
        prop_type: PropertyType,
    ) -> io::Result<PropertyValue> {
        self.inner.read_column(value, prop_type)
    }
}

impl TableContextReadWrite<UnicodePstFile> for UnicodeTableContext {
    fn read(
        store: Rc<UnicodeStore>,
        node: UnicodeNodeBTreeEntry,
    ) -> io::Result<Rc<dyn TableContext>> {
        let inner = TableContextInner::read(store, node)?;
        Ok(Rc::new(Self { inner }))
    }
}

type AnsiRowIndexTree = AnsiHeapTree<TableRowId, AnsiTableRowIndex>;

impl TableRowIndexTree<AnsiPstFile> for AnsiRowIndexTree {
    type RowIndex = AnsiTableRowIndex;
}

pub struct AnsiTableContext {
    inner: TableContextInner<AnsiPstFile, AnsiTableRowIndex, AnsiRowIndexTree>,
}

impl AnsiTableContext {
    pub fn read(
        store: Rc<AnsiStore>,
        node: AnsiNodeBTreeEntry,
    ) -> io::Result<Rc<dyn TableContext>> {
        <Self as TableContextReadWrite<AnsiPstFile>>::read(store, node)
    }
}

impl TableContext for AnsiTableContext {
    fn context(&self) -> &TableContextInfo {
        &self.inner.context
    }

    fn rows_matrix<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = &'a TableRowData>> {
        Box::new(self.inner.rows.iter())
    }

    fn find_row(&self, id: TableRowId) -> LtpResult<&TableRowData> {
        let index = self
            .inner
            .row_index
            .get(&id)
            .ok_or(LtpError::TableRowIdNotFound(u32::from(id)))?;
        Ok(&self.inner.rows[u32::from(*index) as usize])
    }

    fn read_column(
        &self,
        value: &TableRowColumnValue,
        prop_type: PropertyType,
    ) -> io::Result<PropertyValue> {
        self.inner.read_column(value, prop_type)
    }
}

impl TableContextReadWrite<AnsiPstFile> for AnsiTableContext {
    fn read(store: Rc<AnsiStore>, node: AnsiNodeBTreeEntry) -> io::Result<Rc<dyn TableContext>> {
        let inner = TableContextInner::read(store, node)?;
        Ok(Rc::new(Self { inner }))
    }
}
