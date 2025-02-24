//! ## [Lists, Tables, and Properties (LTP) Layer](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/4c24c7d2-5c5a-4b99-88b2-f4b84cc293ae)

use thiserror::Error;

pub mod heap;
pub mod property;
pub mod table;
pub mod tree;

#[derive(Error, Debug)]
pub enum LtpError {
    #[error("Node Database error: {0}")]
    NodeDatabaseError(#[from] crate::ndb::NdbError),
}
