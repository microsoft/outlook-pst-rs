use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use outlook_pst::ndb::node_id::NodeId;

mod errors;
mod property_value;

use errors::PstPythonError;
use property_value::property_value_to_python;

/// Pythonモジュールの初期化
#[pymodule]
fn pst_python(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(open_pst, m)?)?;
    m.add_function(wrap_pyfunction!(get_folders, m)?)?;
    m.add_function(wrap_pyfunction!(get_messages, m)?)?;
    m.add_function(wrap_pyfunction!(get_message, m)?)?;
    m.add_function(wrap_pyfunction!(get_attachments, m)?)?;
    Ok(())
}

/// PSTファイルを開いて基本情報を返す
#[pyfunction]
fn open_pst(py: Python, path: &str) -> PyResult<PyObject> {
    let store = outlook_pst::open_store(path)
        .map_err(PstPythonError::from)?;

    let result = PyDict::new_bound(py);
    result.set_item("display_name", store.properties().display_name()
        .map_err(PstPythonError::from)?)?;
    result.set_item("unique_value", store.unique_value())?;
    Ok(result.to_object(py))
}

/// フォルダ一覧を返す
#[pyfunction]
fn get_folders(py: Python, path: &str) -> PyResult<PyObject> {
    let store = outlook_pst::open_store(path)
        .map_err(PstPythonError::from)?;

    let ipm_sub_tree = store.properties().ipm_sub_tree_entry_id()
        .map_err(PstPythonError::from)?;
    let root_folder = store.open_folder(&ipm_sub_tree)
        .map_err(PstPythonError::from)?;

    let hierarchy_table = root_folder.hierarchy_table()
        .ok_or_else(|| PstPythonError::new("No hierarchy table found".to_string()))?;

    let folders = PyList::empty_bound(py);
    for row in hierarchy_table.rows_matrix() {
        let node_id = NodeId::from(u32::from(row.id()));
        let entry_id = store.properties().make_entry_id(node_id)
            .map_err(PstPythonError::from)?;
        let folder = store.open_folder(&entry_id)
            .map_err(PstPythonError::from)?;

        let folder_dict = PyDict::new_bound(py);
        let props = folder.properties();
        folder_dict.set_item("node_id", format!("{:X}", u32::from(node_id)))?;
        folder_dict.set_item("display_name", props.display_name()
            .map_err(PstPythonError::from)?)?;
        folder_dict.set_item("content_count", props.content_count()
            .map_err(PstPythonError::from)?)?;
        folder_dict.set_item("unread_count", props.unread_count()
            .map_err(PstPythonError::from)?)?;
        folder_dict.set_item("has_sub_folders", props.has_sub_folders()
            .map_err(PstPythonError::from)?)?;

        // プロパティを追加
        let props_dict = PyDict::new_bound(py);
        for (prop_id, value) in props.iter() {
            let py_value = property_value_to_python(py, value)?;
            props_dict.set_item(format!("0x{:04X}", prop_id), py_value)?;
        }
        folder_dict.set_item("properties", props_dict)?;

        folders.append(folder_dict)?;
    }

    Ok(folders.to_object(py))
}

/// メッセージ一覧を返す
#[pyfunction]
fn get_messages(py: Python, path: &str, folder_id: &str) -> PyResult<PyObject> {
    let store = outlook_pst::open_store(path)
        .map_err(PstPythonError::from)?;

    let node_id = NodeId::from(
        u32::from_str_radix(folder_id.trim_start_matches("0x"), 16)
            .map_err(|_| PstPythonError::new("Invalid folder_id".to_string()))?
    );
    let entry_id = store.properties().make_entry_id(node_id)
        .map_err(PstPythonError::from)?;
    let folder = store.open_folder(&entry_id)
        .map_err(PstPythonError::from)?;

    let contents_table = folder.contents_table()
        .ok_or_else(|| PstPythonError::new("No contents table found".to_string()))?;

    let messages = PyList::empty_bound(py);
    for row in contents_table.rows_matrix() {
        let node_id = NodeId::from(u32::from(row.id()));
        let entry_id = store.properties().make_entry_id(node_id)
            .map_err(PstPythonError::from)?;
        let message = store.open_message(&entry_id, None)
            .map_err(PstPythonError::from)?;

        let msg_dict = PyDict::new_bound(py);
        let props = message.properties();
        msg_dict.set_item("node_id", format!("{:X}", u32::from(node_id)))?;

        // よく使われるプロパティを直接追加
        if let Some(subject) = props.get(0x0037) {
            match subject {
                outlook_pst::ltp::prop_context::PropertyValue::String8(v) => {
                    msg_dict.set_item("subject", v.to_string())?;
                }
                outlook_pst::ltp::prop_context::PropertyValue::Unicode(v) => {
                    msg_dict.set_item("subject", v.to_string())?;
                }
                _ => {}
            }
        }

        messages.append(msg_dict)?;
    }

    Ok(messages.to_object(py))
}

/// メッセージ詳細を返す
#[pyfunction]
fn get_message(py: Python, path: &str, message_id: &str) -> PyResult<PyObject> {
    let store = outlook_pst::open_store(path)
        .map_err(PstPythonError::from)?;

    let node_id = NodeId::from(
        u32::from_str_radix(message_id.trim_start_matches("0x"), 16)
            .map_err(|_| PstPythonError::new("Invalid message_id".to_string()))?
    );
    let entry_id = store.properties().make_entry_id(node_id)
        .map_err(PstPythonError::from)?;
    let message = store.open_message(&entry_id, None)
        .map_err(PstPythonError::from)?;

    let dict = PyDict::new_bound(py);
    let props = message.properties();

    dict.set_item("node_id", format!("{:X}", u32::from(node_id)))?;

    // よく使われるプロパティを直接追加
    if let Some(subject) = props.get(0x0037) {
        match subject {
            outlook_pst::ltp::prop_context::PropertyValue::String8(v) => {
                dict.set_item("subject", v.to_string())?;
            }
            outlook_pst::ltp::prop_context::PropertyValue::Unicode(v) => {
                dict.set_item("subject", v.to_string())?;
            }
            _ => {}
        }
    }

    if let Some(sender) = props.get(0x0C1A) {
        match sender {
            outlook_pst::ltp::prop_context::PropertyValue::String8(v) => {
                dict.set_item("sender", v.to_string())?;
            }
            outlook_pst::ltp::prop_context::PropertyValue::Unicode(v) => {
                dict.set_item("sender", v.to_string())?;
            }
            _ => {}
        }
    }

    if let Some(body) = props.get(0x1000) {
        match body {
            outlook_pst::ltp::prop_context::PropertyValue::String8(v) => {
                dict.set_item("body", v.to_string())?;
            }
            outlook_pst::ltp::prop_context::PropertyValue::Unicode(v) => {
                dict.set_item("body", v.to_string())?;
            }
            _ => {}
        }
    }

    if let Some(received_time) = props.get(0x0E06) {
        if let outlook_pst::ltp::prop_context::PropertyValue::Time(v) = received_time {
            dict.set_item("received_time", *v)?;
        }
    }

    if let Some(sent_time) = props.get(0x0039) {
        if let outlook_pst::ltp::prop_context::PropertyValue::Time(v) = sent_time {
            dict.set_item("sent_time", *v)?;
        }
    }

    if let Ok(message_class) = props.message_class() {
        dict.set_item("message_class", message_class)?;
    }

    // すべてのプロパティを追加
    let props_dict = PyDict::new_bound(py);
    for (prop_id, value) in props.iter() {
        let py_value = property_value_to_python(py, value)?;
        props_dict.set_item(format!("0x{:04X}", prop_id), py_value)?;
    }
    dict.set_item("properties", props_dict)?;

    Ok(dict.to_object(py))
}

/// 添付ファイル一覧を返す
#[pyfunction]
fn get_attachments(py: Python, path: &str, message_id: &str) -> PyResult<PyObject> {
    let store = outlook_pst::open_store(path)
        .map_err(PstPythonError::from)?;

    let node_id = NodeId::from(
        u32::from_str_radix(message_id.trim_start_matches("0x"), 16)
            .map_err(|_| PstPythonError::new("Invalid message_id".to_string()))?
    );
    let entry_id = store.properties().make_entry_id(node_id)
        .map_err(PstPythonError::from)?;
    let message = store.open_message(&entry_id, None)
        .map_err(PstPythonError::from)?;

    let attachment_table = message.attachment_table();
    let attachments = PyList::empty_bound(py);

    if let Some(table) = attachment_table {
        let context = table.context();
        for row in table.rows_matrix() {
            let att_dict = PyDict::new_bound(py);

            // 添付ファイルの基本情報を取得
            let columns = row.columns(context)
                .map_err(|e| PstPythonError::new(format!("Failed to read columns: {}", e)))?;

            for (col, value) in context.columns().iter().zip(columns.iter()) {
                if let Some(value) = value.as_ref() {
                    match table.read_column(value, col.prop_type()) {
                        Ok(prop_value) => {
                            let py_value = property_value_to_python(py, &prop_value)?;
                            // プロパティIDをキーとして保存
                            att_dict.set_item(format!("0x{:04X}", col.prop_id()), py_value)?;
                        }
                        Err(_) => {}
                    }
                }
            }

            attachments.append(att_dict)?;
        }
    }

    Ok(attachments.to_object(py))
}
