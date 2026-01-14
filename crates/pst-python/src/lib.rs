use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyBytes};
use std::rc::Rc;
use outlook_pst::ndb::node_id::NodeId;
use outlook_pst::messaging::store::EntryId;

mod errors;
mod property_value;

use errors::PstPythonError;
use property_value::property_value_to_python;

// Helper function to convert EntryId to dict
fn entry_id_to_dict<'a>(py: Python<'a>, entry_id: &EntryId) -> PyResult<Bound<'a, PyDict>> {
    let dict = PyDict::new_bound(py);
    let record_key = entry_id.record_key();
    let record_key_bytes = PyBytes::new_bound(py, record_key);
    dict.set_item("record_key", record_key_bytes)?;
    dict.set_item("node_id", format!("{:X}", u32::from(entry_id.node_id())))?;
    Ok(dict)
}

// Helper function to parse node_id from string
fn parse_node_id(node_id_str: &str) -> Result<NodeId, PstPythonError> {
    let node_id = NodeId::from(
        u32::from_str_radix(node_id_str.trim_start_matches("0x"), 16)
            .map_err(|_| PstPythonError::new("Invalid node_id format".to_string()))?
    );
    Ok(node_id)
}

// StoreProperties
#[pyclass]
pub struct PyStoreProperties {
    store: Rc<dyn outlook_pst::messaging::store::Store>,
}

unsafe impl Send for PyStoreProperties {}

#[pymethods]
impl PyStoreProperties {
    fn get(&self, py: Python, prop_id: u16) -> PyResult<PyObject> {
        if let Some(value) = self.store.properties().get(prop_id) {
            property_value_to_python(py, value)
        } else {
            Ok(py.None())
        }
    }

    fn iter<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let dict = PyDict::new_bound(py);
        for (prop_id, value) in self.store.properties().iter() {
            let py_value = property_value_to_python(py, value)?;
            dict.set_item(format!("0x{:04X}", prop_id), py_value)?;
        }
        Ok(dict)
    }

    fn record_key<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyBytes>> {
        let record_key = self.store.properties().record_key()
            .map_err(|e| PstPythonError::from(e))?;
        Ok(PyBytes::new_bound(py, record_key.record_key()))
    }

    fn make_entry_id<'a>(&self, py: Python<'a>, node_id_str: &str) -> PyResult<Bound<'a, PyDict>> {
        let node_id = parse_node_id(node_id_str)?;
        let entry_id = self.store.properties().make_entry_id(node_id)
            .map_err(|e| PstPythonError::from(e))?;
        entry_id_to_dict(py, &entry_id)
    }

    fn display_name(&self) -> PyResult<String> {
        self.store.properties().display_name()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn ipm_sub_tree_entry_id<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let entry_id = self.store.properties().ipm_sub_tree_entry_id()
            .map_err(|e| PstPythonError::from(e))?;
        entry_id_to_dict(py, &entry_id)
    }

    fn ipm_wastebasket_entry_id<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let entry_id = self.store.properties().ipm_wastebasket_entry_id()
            .map_err(|e| PstPythonError::from(e))?;
        entry_id_to_dict(py, &entry_id)
    }

    fn finder_entry_id<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let entry_id = self.store.properties().finder_entry_id()
            .map_err(|e| PstPythonError::from(e))?;
        entry_id_to_dict(py, &entry_id)
    }
}

// FolderProperties
#[pyclass]
pub struct PyFolderProperties {
    folder: Rc<dyn outlook_pst::messaging::folder::Folder>,
}

unsafe impl Send for PyFolderProperties {}

#[pymethods]
impl PyFolderProperties {
    fn get(&self, py: Python, prop_id: u16) -> PyResult<PyObject> {
        if let Some(value) = self.folder.properties().get(prop_id) {
            property_value_to_python(py, value)
        } else {
            Ok(py.None())
        }
    }

    fn iter<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let dict = PyDict::new_bound(py);
        for (prop_id, value) in self.folder.properties().iter() {
            let py_value = property_value_to_python(py, value)?;
            dict.set_item(format!("0x{:04X}", prop_id), py_value)?;
        }
        Ok(dict)
    }

    fn node_id(&self) -> String {
        format!("{:X}", u32::from(self.folder.properties().node_id()))
    }

    fn display_name(&self) -> PyResult<String> {
        self.folder.properties().display_name()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn content_count(&self) -> PyResult<i32> {
        self.folder.properties().content_count()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn unread_count(&self) -> PyResult<i32> {
        self.folder.properties().unread_count()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn has_sub_folders(&self) -> PyResult<bool> {
        self.folder.properties().has_sub_folders()
            .map_err(|e| PstPythonError::from(e).into())
    }
}

// MessageProperties
#[pyclass]
pub struct PyMessageProperties {
    message: Rc<dyn outlook_pst::messaging::message::Message>,
}

unsafe impl Send for PyMessageProperties {}

#[pymethods]
impl PyMessageProperties {
    fn get(&self, py: Python, prop_id: u16) -> PyResult<PyObject> {
        if let Some(value) = self.message.properties().get(prop_id) {
            property_value_to_python(py, value)
        } else {
            Ok(py.None())
        }
    }

    fn iter<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let dict = PyDict::new_bound(py);
        for (prop_id, value) in self.message.properties().iter() {
            let py_value = property_value_to_python(py, value)?;
            dict.set_item(format!("0x{:04X}", prop_id), py_value)?;
        }
        Ok(dict)
    }

    fn message_class(&self) -> PyResult<String> {
        self.message.properties().message_class()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn message_flags(&self) -> PyResult<i32> {
        self.message.properties().message_flags()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn message_size(&self) -> PyResult<i32> {
        self.message.properties().message_size()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn message_status(&self) -> PyResult<i32> {
        self.message.properties().message_status()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn creation_time<'a>(&self, py: Python<'a>) -> PyResult<PyObject> {
        let time = self.message.properties().creation_time()
            .map_err(|e| PstPythonError::from(e))?;
        let windows_epoch = 116444736000000000u64;
        let unix_timestamp = (time as u64).saturating_sub(windows_epoch) / 10_000_000;
        let datetime_module = py.import_bound("datetime")?;
        let datetime_class = datetime_module.getattr("datetime")?;
        let datetime = datetime_class.call_method1("fromtimestamp", (unix_timestamp as i64,))?;
        Ok(datetime.to_object(py))
    }

    fn last_modification_time<'a>(&self, py: Python<'a>) -> PyResult<PyObject> {
        let time = self.message.properties().last_modification_time()
            .map_err(|e| PstPythonError::from(e))?;
        let windows_epoch = 116444736000000000u64;
        let unix_timestamp = (time as u64).saturating_sub(windows_epoch) / 10_000_000;
        let datetime_module = py.import_bound("datetime")?;
        let datetime_class = datetime_module.getattr("datetime")?;
        let datetime = datetime_class.call_method1("fromtimestamp", (unix_timestamp as i64,))?;
        Ok(datetime.to_object(py))
    }

    fn search_key<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyBytes>> {
        let key = self.message.properties().search_key()
            .map_err(|e| PstPythonError::from(e))?;
        Ok(PyBytes::new_bound(py, key))
    }
}

// TableContext
#[pyclass]
pub struct PyTableContext {
    table: Rc<dyn outlook_pst::ltp::table_context::TableContext>,
}

unsafe impl Send for PyTableContext {}

#[pymethods]
impl PyTableContext {
    fn context<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let ctx = self.table.context();
        let dict = PyDict::new_bound(py);
        dict.set_item("end_4byte_values", ctx.end_4byte_values())?;
        dict.set_item("end_2byte_values", ctx.end_2byte_values())?;
        dict.set_item("end_1byte_values", ctx.end_1byte_values())?;
        dict.set_item("end_existence_bitmap", ctx.end_existence_bitmap())?;

        let columns = PyList::empty_bound(py);
        for col in ctx.columns() {
            let col_dict = PyDict::new_bound(py);
            col_dict.set_item("prop_type", col.prop_type() as u16)?;
            col_dict.set_item("prop_id", col.prop_id())?;
            col_dict.set_item("offset", col.offset())?;
            col_dict.set_item("size", col.size())?;
            col_dict.set_item("existence_bitmap_index", col.existence_bitmap_index())?;
            columns.append(col_dict)?;
        }
        dict.set_item("columns", columns)?;
        Ok(dict)
    }

    fn rows_matrix<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyList>> {
        let rows = PyList::empty_bound(py);
        let context = self.table.context();

        for row in self.table.rows_matrix() {
            let row_dict = PyDict::new_bound(py);
            row_dict.set_item("id", u32::from(row.id()))?;
            row_dict.set_item("unique", row.unique())?;

            let columns = row.columns(context)
                .map_err(|e| PstPythonError::new(format!("Failed to read columns: {}", e)))?;

            let props_dict = PyDict::new_bound(py);
            for (col, value) in context.columns().iter().zip(columns.iter()) {
                if let Some(value) = value.as_ref() {
                    match self.table.read_column(value, col.prop_type()) {
                        Ok(prop_value) => {
                            let py_value = property_value_to_python(py, &prop_value)?;
                            props_dict.set_item(format!("0x{:04X}", col.prop_id()), py_value)?;
                        }
                        Err(_) => {}
                    }
                }
            }
            row_dict.set_item("properties", props_dict)?;
            rows.append(row_dict)?;
        }
        Ok(rows)
    }

    fn find_row<'a>(&self, py: Python<'a>, row_id: u32) -> PyResult<Bound<'a, PyDict>> {
        let table_row_id = outlook_pst::ltp::table_context::TableRowId::new(row_id);
        let row = self.table.find_row(table_row_id)
            .map_err(|e| PstPythonError::new(format!("Row not found: {}", e)))?;
        let context = self.table.context();

        let row_dict = PyDict::new_bound(py);
        row_dict.set_item("id", u32::from(row.id()))?;
        row_dict.set_item("unique", row.unique())?;

        let columns = row.columns(context)
            .map_err(|e| PstPythonError::new(format!("Failed to read columns: {}", e)))?;

        let props_dict = PyDict::new_bound(py);
        for (col, value) in context.columns().iter().zip(columns.iter()) {
            if let Some(value) = value.as_ref() {
                match self.table.read_column(value, col.prop_type()) {
                    Ok(prop_value) => {
                        let py_value = property_value_to_python(py, &prop_value)?;
                        props_dict.set_item(format!("0x{:04X}", col.prop_id()), py_value)?;
                    }
                    Err(_) => {}
                }
            }
        }
        row_dict.set_item("properties", props_dict)?;
        Ok(row_dict)
    }
}

// NamedPropertyMapProperties
#[pyclass]
pub struct PyNamedPropertyMapProperties {
    map: Rc<dyn outlook_pst::messaging::named_prop::NamedPropertyMap>,
}

unsafe impl Send for PyNamedPropertyMapProperties {}

#[pymethods]
impl PyNamedPropertyMapProperties {
    fn get(&self, py: Python, prop_id: u16) -> PyResult<PyObject> {
        if let Some(value) = self.map.properties().get(prop_id) {
            property_value_to_python(py, value)
        } else {
            Ok(py.None())
        }
    }

    fn iter<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let dict = PyDict::new_bound(py);
        for (prop_id, value) in self.map.properties().iter() {
            let py_value = property_value_to_python(py, value)?;
            dict.set_item(format!("0x{:04X}", prop_id), py_value)?;
        }
        Ok(dict)
    }

    fn bucket_count(&self) -> PyResult<u16> {
        self.map.properties().bucket_count()
            .map_err(|e| PstPythonError::from(e).into())
    }

    fn stream_guid<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyList>> {
        let guids = self.map.properties().stream_guid()
            .map_err(|e| PstPythonError::from(e))?;
        let list = PyList::empty_bound(py);
        for guid in guids {
            list.append(format!("{:?}", guid))?;
        }
        Ok(list)
    }

    fn stream_entry<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyList>> {
        let entries = self.map.properties().stream_entry()
            .map_err(|e| PstPythonError::from(e))?;
        let list = PyList::empty_bound(py);
        for entry in entries {
            let entry_dict = PyDict::new_bound(py);
            match entry.id() {
                outlook_pst::messaging::named_prop::NamedPropertyId::Number(n) => {
                    entry_dict.set_item("id_type", "Number")?;
                    entry_dict.set_item("id", n)?;
                }
                outlook_pst::messaging::named_prop::NamedPropertyId::StringOffset(o) => {
                    entry_dict.set_item("id_type", "StringOffset")?;
                    entry_dict.set_item("id", o)?;
                }
            }
            entry_dict.set_item("prop_id", entry.prop_id())?;
            list.append(entry_dict)?;
        }
        Ok(list)
    }

    fn stream_string<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyList>> {
        let strings = self.map.properties().stream_string()
            .map_err(|e| PstPythonError::from(e))?;
        let list = PyList::empty_bound(py);
        for string_entry in strings {
            list.append(string_entry.to_string())?;
        }
        Ok(list)
    }
}

// NamedPropertyMap
#[pyclass]
pub struct PyNamedPropertyMap {
    map: Rc<dyn outlook_pst::messaging::named_prop::NamedPropertyMap>,
}

unsafe impl Send for PyNamedPropertyMap {}

#[pymethods]
impl PyNamedPropertyMap {
    fn properties(&self) -> PyNamedPropertyMapProperties {
        PyNamedPropertyMapProperties {
            map: self.map.clone(),
        }
    }
}

// SearchUpdateQueue
#[pyclass]
pub struct PySearchUpdateQueue {
    queue: Rc<dyn outlook_pst::messaging::search::SearchUpdateQueue>,
}

unsafe impl Send for PySearchUpdateQueue {}

#[pymethods]
impl PySearchUpdateQueue {
    fn updates<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyList>> {
        let updates = self.queue.updates();
        let list = PyList::empty_bound(py);

        for update in updates {
            let update_dict = PyDict::new_bound(py);
            update_dict.set_item("flags", update.flags())?;

            if let Some(data) = update.data() {
                let data_dict = PyDict::new_bound(py);
                match data {
                    outlook_pst::messaging::search::SearchUpdateData::MessageAdded { parent, message } => {
                        data_dict.set_item("type", "MessageAdded")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::MessageModified { parent, message } => {
                        data_dict.set_item("type", "MessageModified")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::MessageDeleted { parent, message } => {
                        data_dict.set_item("type", "MessageDeleted")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::MessageMoved { new_parent, message, old_parent } => {
                        data_dict.set_item("type", "MessageMoved")?;
                        data_dict.set_item("new_parent", format!("{:X}", u32::from(*new_parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                        data_dict.set_item("old_parent", format!("{:X}", u32::from(*old_parent)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::FolderAdded { parent, folder, .. } => {
                        data_dict.set_item("type", "FolderAdded")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("folder", format!("{:X}", u32::from(*folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::FolderModified { folder, .. } => {
                        data_dict.set_item("type", "FolderModified")?;
                        data_dict.set_item("folder", format!("{:X}", u32::from(*folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::FolderDeleted { folder, .. } => {
                        data_dict.set_item("type", "FolderDeleted")?;
                        data_dict.set_item("folder", format!("{:X}", u32::from(*folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::FolderMoved { parent, folder, .. } => {
                        data_dict.set_item("type", "FolderMoved")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("folder", format!("{:X}", u32::from(*folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::SearchFolderAdded { search_folder } => {
                        data_dict.set_item("type", "SearchFolderAdded")?;
                        data_dict.set_item("search_folder", format!("{:X}", u32::from(*search_folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::SearchFolderModified { search_folder, .. } => {
                        data_dict.set_item("type", "SearchFolderModified")?;
                        data_dict.set_item("search_folder", format!("{:X}", u32::from(*search_folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::SearchFolderDeleted { search_folder } => {
                        data_dict.set_item("type", "SearchFolderDeleted")?;
                        data_dict.set_item("search_folder", format!("{:X}", u32::from(*search_folder)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::MessageRowModified { parent, message } => {
                        data_dict.set_item("type", "MessageRowModified")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::MessageSpam { parent, message } => {
                        data_dict.set_item("type", "MessageSpam")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::IndexedMessageDeleted { parent, message } => {
                        data_dict.set_item("type", "IndexedMessageDeleted")?;
                        data_dict.set_item("parent", format!("{:X}", u32::from(*parent)))?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                    outlook_pst::messaging::search::SearchUpdateData::MessageIndexed { message } => {
                        data_dict.set_item("type", "MessageIndexed")?;
                        data_dict.set_item("message", format!("{:X}", u32::from(*message)))?;
                    }
                }
                update_dict.set_item("data", data_dict)?;
            }
            list.append(update_dict)?;
        }
        Ok(list)
    }
}

// Folder
#[pyclass]
pub struct PyFolder {
    folder: Rc<dyn outlook_pst::messaging::folder::Folder>,
}

unsafe impl Send for PyFolder {}

#[pymethods]
impl PyFolder {
    fn properties(&self) -> PyFolderProperties {
        PyFolderProperties {
            folder: self.folder.clone(),
        }
    }

    fn hierarchy_table(&self, _py: Python) -> PyResult<Option<PyTableContext>> {
        if let Some(table) = self.folder.hierarchy_table() {
            Ok(Some(PyTableContext {
                table: table.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    fn contents_table(&self, _py: Python) -> PyResult<Option<PyTableContext>> {
        if let Some(table) = self.folder.contents_table() {
            Ok(Some(PyTableContext {
                table: table.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    fn associated_table(&self, _py: Python) -> PyResult<Option<PyTableContext>> {
        if let Some(table) = self.folder.associated_table() {
            Ok(Some(PyTableContext {
                table: table.clone(),
            }))
        } else {
            Ok(None)
        }
    }
}

// Message
#[pyclass]
pub struct PyMessage {
    message: Rc<dyn outlook_pst::messaging::message::Message>,
}

unsafe impl Send for PyMessage {}

#[pymethods]
impl PyMessage {
    fn properties(&self) -> PyMessageProperties {
        PyMessageProperties {
            message: self.message.clone(),
        }
    }

    fn recipient_table(&self) -> PyTableContext {
        PyTableContext {
            table: self.message.recipient_table().clone(),
        }
    }

    fn attachment_table(&self, _py: Python) -> PyResult<Option<PyTableContext>> {
        if let Some(table) = self.message.attachment_table() {
            Ok(Some(PyTableContext {
                table: table.clone(),
            }))
        } else {
            Ok(None)
        }
    }
}

// Store
#[pyclass]
pub struct PyStore {
    store: Rc<dyn outlook_pst::messaging::store::Store>,
}

unsafe impl Send for PyStore {}

#[pymethods]
impl PyStore {
    fn properties(&self) -> PyStoreProperties {
        PyStoreProperties {
            store: self.store.clone(),
        }
    }

    fn root_hierarchy_table(&self) -> PyResult<PyTableContext> {
        let table = self.store.root_hierarchy_table()
            .map_err(PstPythonError::from)?;
        Ok(PyTableContext {
            table,
        })
    }

    fn unique_value(&self) -> u32 {
        self.store.unique_value()
    }

    fn open_folder(&self, node_id_str: &str) -> PyResult<PyFolder> {
        let node_id = parse_node_id(node_id_str)?;
        let entry_id = self.store.properties().make_entry_id(node_id)
            .map_err(|e| PstPythonError::from(e))?;
        let folder = self.store.open_folder(&entry_id)
            .map_err(|e| PstPythonError::from(e))?;
        Ok(PyFolder { folder })
    }

    #[pyo3(signature = (node_id_str, prop_ids=None))]
    fn open_message(&self, node_id_str: &str, prop_ids: Option<Vec<u16>>) -> PyResult<PyMessage> {
        let node_id = parse_node_id(node_id_str)?;
        let entry_id = self.store.properties().make_entry_id(node_id)
            .map_err(|e| PstPythonError::from(e))?;
        let prop_ids_slice = prop_ids.as_deref();
        let message = self.store.open_message(&entry_id, prop_ids_slice)
            .map_err(|e| PstPythonError::from(e))?;
        Ok(PyMessage { message })
    }

    fn named_property_map(&self) -> PyResult<PyNamedPropertyMap> {
        let map = self.store.named_property_map()
            .map_err(PstPythonError::from)?;
        Ok(PyNamedPropertyMap { map })
    }

    fn search_update_queue(&self) -> PyResult<PySearchUpdateQueue> {
        let queue = self.store.search_update_queue()
            .map_err(PstPythonError::from)?;
        Ok(PySearchUpdateQueue { queue })
    }
}

#[pymodule]
fn pst_python(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStore>()?;
    m.add_class::<PyStoreProperties>()?;
    m.add_class::<PyFolder>()?;
    m.add_class::<PyFolderProperties>()?;
    m.add_class::<PyMessage>()?;
    m.add_class::<PyMessageProperties>()?;
    m.add_class::<PyTableContext>()?;
    m.add_class::<PyNamedPropertyMap>()?;
    m.add_class::<PyNamedPropertyMapProperties>()?;
    m.add_class::<PySearchUpdateQueue>()?;
    m.add_function(wrap_pyfunction!(open_pst, m)?)?;
    Ok(())
}

#[pyfunction]
fn open_pst(path: &str) -> PyResult<PyStore> {
    let store = outlook_pst::open_store(path)
        .map_err(PstPythonError::from)?;
    Ok(PyStore { store })
}
