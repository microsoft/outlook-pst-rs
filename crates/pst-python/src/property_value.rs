use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// PropertyValueをPythonオブジェクトに変換
pub fn property_value_to_python(py: Python, value: &outlook_pst::ltp::prop_context::PropertyValue) -> PyResult<PyObject> {
    use outlook_pst::ltp::prop_context::PropertyValue;

    match value {
        PropertyValue::Null => Ok(py.None()),
        PropertyValue::Integer16(v) => Ok(v.to_object(py)),
        PropertyValue::Integer32(v) => Ok(v.to_object(py)),
        PropertyValue::Floating32(v) => Ok(v.to_object(py)),
        PropertyValue::Floating64(v) => Ok(v.to_object(py)),
        PropertyValue::Currency(v) => Ok(v.to_object(py)),
        PropertyValue::FloatingTime(v) => Ok(v.to_object(py)),
        PropertyValue::ErrorCode(v) => Ok(v.to_object(py)),
        PropertyValue::Boolean(v) => Ok(v.to_object(py)),
        PropertyValue::Integer64(v) => Ok(v.to_object(py)),
        PropertyValue::String8(v) => Ok(v.to_string().to_object(py)),
        PropertyValue::Unicode(v) => Ok(v.to_string().to_object(py)),
        PropertyValue::Time(v) => {
            // Windows FILETIME (100-nanosecond intervals since January 1, 1601)
            // to Unix timestamp (seconds since January 1, 1970)
            let windows_epoch = 116444736000000000u64; // January 1, 1601 in 100-nanosecond intervals
            let unix_timestamp = (*v as u64).saturating_sub(windows_epoch) / 10_000_000;
            let datetime_module = py.import_bound("datetime")?;
            let datetime_class = datetime_module.getattr("datetime")?;
            let datetime = datetime_class.call_method1("fromtimestamp", (unix_timestamp as i64,))?;
            Ok(datetime.to_object(py))
        }
        PropertyValue::Guid(v) => {
            // GUIDを文字列に変換
            let guid_str = format!("{:?}", v);
            Ok(guid_str.to_object(py))
        }
        PropertyValue::Binary(v) => {
            Ok(PyBytes::new_bound(py, v.buffer()).to_object(py))
        }
        PropertyValue::Object(_) => {
            // Objectは複雑なので、とりあえずNoneを返す
            Ok(py.None())
        }
        PropertyValue::MultipleInteger16(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleInteger32(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleFloating32(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleFloating64(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleCurrency(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleFloatingTime(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleInteger64(v) => {
            Ok(v.to_object(py))
        }
        PropertyValue::MultipleString8(v) => {
            let strings: Vec<String> = v.iter().map(|s| s.to_string()).collect();
            Ok(strings.to_object(py))
        }
        PropertyValue::MultipleUnicode(v) => {
            let strings: Vec<String> = v.iter().map(|s| s.to_string()).collect();
            Ok(strings.to_object(py))
        }
        PropertyValue::MultipleTime(v) => {
            let windows_epoch = 116444736000000000u64;
            let timestamps: Vec<i64> = v.iter()
                .map(|&t| {
                    ((t as u64).saturating_sub(windows_epoch) / 10_000_000) as i64
                })
                .collect();
            let datetime_module = py.import_bound("datetime")?;
            let datetime_class = datetime_module.getattr("datetime")?;
            let datetimes: Vec<PyObject> = timestamps.iter()
                .map(|&ts| {
                    datetime_class.call_method1("fromtimestamp", (ts,))
                        .map(|obj| obj.to_object(py))
                })
                .collect::<PyResult<Vec<_>>>()?;
            Ok(datetimes.to_object(py))
        }
        PropertyValue::MultipleGuid(v) => {
            let guid_strings: Vec<String> = v.iter().map(|g| format!("{:?}", g)).collect();
            Ok(guid_strings.to_object(py))
        }
        PropertyValue::MultipleBinary(v) => {
            let binaries: Vec<PyObject> = v.iter()
                .map(|b| PyBytes::new_bound(py, b.buffer()).to_object(py))
                .collect();
            Ok(binaries.to_object(py))
        }
    }
}
