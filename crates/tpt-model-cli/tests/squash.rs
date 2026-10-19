use std::path::PathBuf;
use tpt_model_cli::{analyze, apply, scan, Refusal, SourceFile};

fn file(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        text: text.to_string(),
    }
}

const CAD: &str = r#"use tpt_model::{Migration, Model};

#[derive(Model)]
#[tpt(version = 1, name = "Cad")]
pub struct CadV1 {
    pub weight: f32,
}

#[derive(Model)]
#[tpt(version = 2, name = "Cad")]
pub struct CadV2 {
    pub mass: f64,
}

#[derive(Model)]
#[tpt(version = 3, name = "Cad")]
pub struct CadV3 {
    pub mass: f64,
    pub note: String,
}

pub struct Step12;

impl Migration for Step12 {
    type From = CadV1;
    type To = CadV2;
    fn migrate(old: &tpt_model::UirNode, _ctx: &tpt_model::MigrationContext) -> Result<tpt_model::UirNode, tpt_model::MigrationError> {
        Ok(old.clone())
    }
}

pub struct Step23;

impl Migration for Step23 {
    type From = CadV2;
    type To = CadV3;
    fn migrate(old: &tpt_model::UirNode, _ctx: &tpt_model::MigrationContext) -> Result<tpt_model::UirNode, tpt_model::MigrationError> {
        Ok(old.clone())
    }
}
"#;

const MAIN_OK: &str = "fn main() {\n    println!(\"hello\");\n}\n";

const MAIN_USES_OLD: &str =
    "use crate::cad::CadV1;\n\nfn main() {\n    let _ = CadV1 { weight: 1.0 };\n}\n";

#[test]
fn plans_removal_of_versions_below_the_minimum() {
    let files = vec![file("src/cad.rs", CAD), file("src/main.rs", MAIN_OK)];
    let scanned = scan(&files).unwrap();
    let plan = analyze(&scanned, 2).unwrap();
    let whats: Vec<&str> = plan.removals.iter().map(|r| r.what.as_str()).collect();
    assert!(
        whats.iter().any(|w| w.starts_with("CadV1 (Cad v1)")),
        "{whats:?}"
    );
    assert!(
        whats
            .iter()
            .any(|w| w.contains("impl Migration for Step12 (v1 -> v2)")),
        "{whats:?}"
    );
    assert!(
        whats.iter().any(|w| w.starts_with("marker type Step12")),
        "{whats:?}"
    );
    assert!(!whats
        .iter()
        .any(|w| w.contains("CadV2") || w.contains("Step23") || w.contains("CadV3")));
}

#[test]
fn apply_removes_the_planned_lines_and_keeps_the_rest() {
    let files = vec![file("src/cad.rs", CAD), file("src/main.rs", MAIN_OK)];
    let plan = analyze(&scan(&files).unwrap(), 2).unwrap();
    let applied = apply(&plan, &files);
    let cad = &applied[0].text;
    assert!(!cad.contains("struct CadV1"));
    assert!(!cad.contains("Step12"));
    assert!(cad.contains("struct CadV2"));
    assert!(cad.contains("impl Migration for Step23"));
    assert!(cad.contains("struct CadV3"));
    // Re-scanning the result must find no version below the minimum.
    let rescan = analyze(&scan(&applied).unwrap(), 2).unwrap();
    assert!(
        rescan.removals.iter().all(|r| !r.what.contains("v1")),
        "{:?}",
        rescan.removals
    );
}

#[test]
fn refuses_when_kept_code_still_uses_a_removed_type() {
    let files = vec![file("src/cad.rs", CAD), file("src/main.rs", MAIN_USES_OLD)];
    let err = analyze(&scan(&files).unwrap(), 2).unwrap_err();
    match err {
        Refusal::StillReferenced {
            target, by, file, ..
        } => {
            assert_eq!(target, "CadV1");
            assert_eq!(by, "use", "the import is the first reference found");
            assert_eq!(file, PathBuf::from("src/main.rs"));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn refuses_a_minimum_newer_than_the_latest_version() {
    let files = vec![file("src/cad.rs", CAD)];
    let err = analyze(&scan(&files).unwrap(), 5).unwrap_err();
    assert_eq!(
        err,
        Refusal::MinAboveLatest {
            name: "Cad".into(),
            min: 5,
            latest: 3
        }
    );
}

#[test]
fn patch_is_a_unified_diff_of_deletions() {
    let files = vec![file("src/cad.rs", CAD)];
    let plan = analyze(&scan(&files).unwrap(), 2).unwrap();
    let patch = plan.patch(&files);
    assert!(patch.starts_with("--- a/src/cad.rs\n+++ b/src/cad.rs\n"));
    assert!(patch.lines().any(|l| l == "-pub struct CadV1 {"), "{patch}");
    assert!(patch.lines().filter(|l| l.starts_with("@@")).count() >= 2);
}

#[test]
fn nothing_below_the_minimum_means_an_empty_plan() {
    let files = vec![file("src/cad.rs", CAD)];
    let plan = analyze(&scan(&files).unwrap(), 1).unwrap();
    assert!(plan.removals.is_empty());
    assert!(plan.render().contains("nothing to remove"));
}
