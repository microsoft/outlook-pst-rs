use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use std::io;

#[pyclass(extends=PyException)]
#[derive(Debug)]
pub struct PstPythonError {
    message: String,
}

#[pymethods]
impl PstPythonError {
    #[new]
    pub fn new(message: String) -> Self {
        Self { message }
    }

    fn __str__(&self) -> &str {
        &self.message
    }
}

impl std::error::Error for PstPythonError {}

impl std::fmt::Display for PstPythonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<outlook_pst::PstError> for PstPythonError {
    fn from(err: outlook_pst::PstError) -> Self {
        Self {
            message: format!("PST Error: {}", err),
        }
    }
}

impl From<outlook_pst::messaging::MessagingError> for PstPythonError {
    fn from(err: outlook_pst::messaging::MessagingError) -> Self {
        Self {
            message: format!("Messaging Error: {}", err),
        }
    }
}

impl From<io::Error> for PstPythonError {
    fn from(err: io::Error) -> Self {
        Self {
            message: format!("I/O Error: {}", err),
        }
    }
}

impl From<PstPythonError> for PyErr {
    fn from(err: PstPythonError) -> Self {
        PyException::new_err(err.message)
    }
}
