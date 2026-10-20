//! Patch and apply output: CRLF preservation, hunk headers accepted by `git apply`, and
//! malformed attributes reported instead of ignored.

use std::path::PathBuf;
use std::process::Command;
use tpt_model_cli::{analyze, apply, scan, SourceFile};

const LF_SOURCE: &str = "use tpt_model::Model;

#[derive(Model)]
#[tpt(version = 1, name = \"Cad\")]
pub struct CadV1 {
    pub weight: f32,
}

#[derive(Model)]
#[tpt(version = 2, name = \"Cad\")]
pub struct CadV2 {
    pub mass: f64,
}
";

fn file(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        text: text.to_string(),
    }
}

fn plan_for(files: &[SourceFile], min: u32) -> tpt_model_cli::Plan {
    let scanned = scan(files).unwrap();
    analyze(&scanned, min).unwrap()
}

#[test]
fn apply_keeps_crlf_line_endings() {
    let crlf = LF_SOURCE.replace('\n', "\r\n");
    let files = [file("src/lib.rs", &crlf)];
    let plan = plan_for(&files, 2);
    let updated = apply(&plan, &files);
    let text = &updated[0].text;
    assert!(!text.contains("CadV1"));
    assert!(text.contains("CadV2"));
    assert_eq!(text.matches("\r\n").count(), text.matches('\n').count());
    assert_eq!(text, &apply(&plan, &files)[0].text);
}

#[test]
fn apply_keeps_a_missing_final_newline() {
    let no_newline = LF_SOURCE.trim_end();
    let files = [file("src/lib.rs", no_newline)];
    let plan = plan_for(&files, 2);
    let text = apply(&plan, &files).remove(0).text;
    assert!(!text.ends_with('\n'));
}

#[test]
fn patch_applies_with_git_apply() {
    let dir = std::env::temp_dir().join(format!("tpt-model-patch-{}", std::process::id()));
    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), LF_SOURCE).unwrap();

    let files = [file("src/lib.rs", LF_SOURCE)];
    let plan = plan_for(&files, 2);
    let patch_path = dir.join("squash.patch");
    std::fs::write(&patch_path, plan.patch(&files)).unwrap();

    let status = Command::new("git")
        .current_dir(&dir)
        .args(["apply", "--check", "squash.patch"])
        .status();
    let ok = matches!(status, Ok(s) if s.success());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "git apply --check rejected the emitted patch");
}

#[test]
fn patch_applies_to_crlf_files() {
    let dir = std::env::temp_dir().join(format!("tpt-model-crlf-{}", std::process::id()));
    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let crlf = LF_SOURCE.replace('\n', "\r\n");
    std::fs::write(src_dir.join("lib.rs"), &crlf).unwrap();

    let files = [file("src/lib.rs", &crlf)];
    let plan = plan_for(&files, 2);
    std::fs::write(dir.join("squash.patch"), plan.patch(&files)).unwrap();

    let status = Command::new("git")
        .current_dir(&dir)
        .args(["apply", "--check", "squash.patch"])
        .status();
    let ok = matches!(status, Ok(s) if s.success());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "git apply --check rejected the patch for a CRLF file");
}

#[test]
fn malformed_tpt_attribute_is_an_error() {
    let source = "#[derive(Model)]
#[tpt(version = \"two\")]
pub struct Broken {
    pub x: u32,
}
";
    let err = scan(&[file("src/broken.rs", source)]).unwrap_err();
    assert!(err.contains("malformed #[tpt] attribute"), "{err}");
}
