//! `multipart/form-data` bodies, for the operations that upload a file.

use crate::operation::Body;
use crate::types::Upload;

/// The form field the file is sent in.
// The spec declares no file parameter; ASP.NET reads any file in the form.
pub(crate) const FILE_FIELD: &str = "file";

/// A body with `upload` as its only part.
///
/// The boundary is fixed rather than random, so that requests are reproducible;
/// it is lengthened until it does not occur in the file.
pub(crate) fn encode(upload: &Upload) -> Body {
    let mut boundary = String::from("repay-boundary");
    while contains(&upload.bytes, boundary.as_bytes()) {
        boundary.push_str("-x");
    }
    let filename = upload.filename.replace(['"', '\r', '\n'], "_");
    let mut bytes = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"{FILE_FIELD}\"; filename=\"{filename}\"\r\n\
         Content-Type: {}\r\n\r\n",
        upload.content_type
    )
    .into_bytes();
    bytes.extend_from_slice(&upload.bytes);
    bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Body {
        content_type: format!("multipart/form-data; boundary={boundary}"),
        bytes,
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_one_file_part() {
        let body = encode(&Upload::csv("rows.csv", "a,b\n1,2\n"));
        assert_eq!(
            body.content_type,
            "multipart/form-data; boundary=repay-boundary"
        );
        assert_eq!(
            String::from_utf8(body.bytes).unwrap(),
            "--repay-boundary\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"rows.csv\"\r\n\
             Content-Type: text/csv\r\n\r\n\
             a,b\n1,2\n\
             \r\n--repay-boundary--\r\n"
        );
    }

    #[test]
    fn lengthens_the_boundary_when_the_file_contains_it() {
        let body = encode(&Upload::csv("x\".csv", "--repay-boundary"));
        assert_eq!(
            body.content_type,
            "multipart/form-data; boundary=repay-boundary-x"
        );
        assert!(
            String::from_utf8(body.bytes)
                .unwrap()
                .contains("filename=\"x_.csv\"")
        );
    }
}
