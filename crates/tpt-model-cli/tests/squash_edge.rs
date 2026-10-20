//! Squash edge cases: items on a shared line, test modules, and imports left behind.

use std::path::PathBuf;
use tpt_model_cli::{analyze, apply, scan, Refusal, SourceFile};

fn file(text: &str) -> [SourceFile; 1] {
    [SourceFile {
        path: PathBuf::from("src/lib.rs"),
        text: text.to_string(),
    }]
}

#[test]
fn a_kept_item_on_a_removed_line_is_refused() {
    // `Keep` shares its line with the removed v1 struct, so removing lines would delete it too.
    let files = file(
        "#[derive(Model)] #[tpt(version = 1, name = \"X\")] pub struct XV1 { pub a: u32 } pub struct Keep;\n\
         #[derive(Model)]\n#[tpt(version = 2, name = \"X\")]\npub struct XV2 {\n    pub a: u32,\n}\n",
    );
    let scanned = scan(&files).unwrap();
    let refusal = analyze(&scanned, 2).unwrap_err();
    assert!(
        matches!(refusal, Refusal::SharesLines { ref by, .. } if by == "struct Keep"),
        "{refusal:?}"
    );
}

#[test]
fn a_test_module_that_uses_a_removed_type_is_refused() {
    let files = file(
        "#[derive(Model)]\n#[tpt(version = 1, name = \"Y\")]\npub struct YV1 {\n    pub a: u32,\n}\n\n\
         #[derive(Model)]\n#[tpt(version = 2, name = \"Y\")]\npub struct YV2 {\n    pub a: u32,\n}\n\n\
         #[cfg(test)]\nmod tests {\n    use super::*;\n    fn make() -> YV1 {\n        YV1 { a: 1 }\n    }\n}\n",
    );
    let scanned = scan(&files).unwrap();
    let refusal = analyze(&scanned, 2).unwrap_err();
    assert!(
        matches!(refusal, Refusal::StillReferenced { ref target, .. } if target == "YV1"),
        "{refusal:?}"
    );
}

#[test]
fn applied_output_still_parses_and_keeps_its_imports() {
    let source = "use tpt_model::{Migration, Model};\n\n\
         #[derive(Model)]\n#[tpt(version = 1, name = \"Z\")]\npub struct ZV1 {\n    pub a: u32,\n}\n\n\
         #[derive(Model)]\n#[tpt(version = 2, name = \"Z\")]\npub struct ZV2 {\n    pub a: u32,\n}\n";
    let files = file(source);
    let scanned = scan(&files).unwrap();
    let plan = analyze(&scanned, 2).unwrap();
    let updated = apply(&plan, &files).remove(0).text;
    assert!(
        updated.starts_with("use tpt_model::{Migration, Model};"),
        "{updated}"
    );
    assert!(updated.contains("pub struct ZV2"));
    assert!(!updated.contains("ZV1"));
    syn::parse_file(&updated).expect("applied output is valid Rust");
}
