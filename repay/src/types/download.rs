//! Files returned by the API.

/// A file the API returned instead of JSON, such as a CSV, Excel or PDF export.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Download {
    /// The media type, such as `text/csv`.
    pub content_type: String,
    /// The file name the API suggested, if any.
    pub filename: Option<String>,
    /// The file's contents.
    pub bytes: Vec<u8>,
}

impl Download {
    /// A file with the given media type, suggested name and contents.
    pub fn new(
        content_type: impl Into<String>,
        filename: Option<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            content_type: content_type.into(),
            filename,
            bytes: bytes.into(),
        }
    }
}
