//! Files sent to the API.

/// A file uploaded as `multipart/form-data`, such as the CSV of a bulk operation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Upload {
    /// The file name sent with the file.
    pub filename: String,
    /// The media type, such as `text/csv`.
    pub content_type: String,
    /// The file's contents.
    pub bytes: Vec<u8>,
}

impl Upload {
    /// A file with the given name, media type and contents.
    pub fn new(
        filename: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            filename: filename.into(),
            content_type: content_type.into(),
            bytes: bytes.into(),
        }
    }

    /// A CSV file.
    pub fn csv(filename: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(filename, "text/csv", bytes)
    }
}

crate::validate::no_constraints!(Upload);
