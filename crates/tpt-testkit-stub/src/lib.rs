//! Local stand-in for `tpt-testkit`: golden-file comparison and small helpers.

#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

/// Compares `actual` with the golden file at `path`.
///
/// When the `TPT_UPDATE_GOLDEN` environment variable is set, the golden file is
/// (re)written instead of compared. Panics with a readable message on mismatch.
pub fn assert_golden(path: impl AsRef<Path>, actual: &str) {
    let path = path.as_ref();
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create golden dir");
        }
        fs::write(path, actual).expect("write golden file");
        return;
    }
    let expected = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "missing golden file {} ({e}); run with TPT_UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    assert_eq!(
        expected.replace("\r\n", "\n"),
        actual.replace("\r\n", "\n"),
        "golden mismatch for {}",
        path.display()
    );
}

/// Lowercase hex of a byte slice.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
