//! Command-line driver: argument parsing, JSON plan output and `inspect`.

use std::path::PathBuf;
use tpt_crypto_stub::{Blake3, KeyedBlake3};
use tpt_model::load::save;
use tpt_model::{EntityId, Model, Registry};
use tpt_model_cli::driver::{parse, Command, SquashOptions};
use tpt_model_cli::inspect;
use tpt_model_cli::{analyze, scan, SourceFile};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn help_is_recognised_at_the_top_and_after_a_command() {
    assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
    assert_eq!(parse(&args(&["squash", "--help"])), Ok(Command::Help));
}

#[test]
fn squash_options_parse() {
    let cmd = parse(&args(&["squash", "--min-supported", "2", "--json", "src"])).unwrap();
    assert_eq!(
        cmd,
        Command::Squash(SquashOptions {
            min_supported: Some(2),
            json: true,
            paths: vec![PathBuf::from("src")],
            ..Default::default()
        })
    );
}

#[test]
fn usage_problems_are_errors() {
    assert!(parse(&args(&[])).is_err());
    assert!(parse(&args(&["frobnicate"])).is_err());
    assert!(
        parse(&args(&["squash", "src"])).is_err(),
        "--min-supported is required"
    );
    assert!(parse(&args(&["inspect"])).is_err(), "inspect needs a file");
}

#[test]
fn key_must_be_64_hex_digits() {
    let good = "ab".repeat(32);
    let cmd = parse(&args(&["inspect", "--key-hex", &good, "x.bin"])).unwrap();
    assert_eq!(
        cmd,
        Command::Inspect {
            key: Some([0xab; 32]),
            file: PathBuf::from("x.bin"),
        }
    );
    assert!(parse(&args(&["inspect", "--key-hex", "abcd", "x.bin"])).is_err());
    assert!(parse(&args(&["inspect", "--key-hex", &"zz".repeat(32), "x.bin"])).is_err());
}

#[test]
fn plan_json_is_one_escaped_line() {
    let source = "#[derive(Model)]\n#[tpt(version = 1, name = \"Cad\")]\npub struct CadV1 {\n    pub x: u32,\n}\n\n#[derive(Model)]\n#[tpt(version = 2, name = \"Cad\")]\npub struct CadV2 {\n    pub x: u32,\n}\n";
    let files = [SourceFile {
        path: PathBuf::from("src/lib.rs"),
        text: source.to_string(),
    }];
    let plan = analyze(&scan(&files).unwrap(), 2).unwrap();
    let json = plan.to_json();
    assert_eq!(
        json.matches('\n').count(),
        1,
        "one line plus the terminator"
    );
    assert!(json.starts_with("{\"min_supported\":2,\"removals\":[{\"what\":\"CadV1 (Cad v1)\""));
    assert!(json.contains("\"file\":\"src/lib.rs\""));
}

#[test]
fn inspect_shows_the_header_and_the_tree() {
    #[derive(Debug, serde::Serialize, serde::Deserialize, Model)]
    #[tpt(version = 1, name = "Note")]
    struct NoteV1 {
        title: String,
        tags: Vec<String>,
    }
    let mut registry = Registry::new();
    registry.register::<NoteV1>().unwrap();
    let id = EntityId::new_v7(1_700_000_000_000, [2; 10]);
    let note = NoteV1 {
        title: "hi".into(),
        tags: vec!["a".into()],
    };
    let bytes = save(&Blake3, id, &note).unwrap();
    let out = inspect::render(&bytes, None).unwrap();
    assert!(out.contains("version:   1"), "{out}");
    assert!(
        out.contains("hash:      verified (unkeyed BLAKE3)"),
        "{out}"
    );
    assert!(out.contains("title: \"hi\""), "{out}");
    assert!(out.contains("[0]: \"a\""), "{out}");
}

#[test]
fn inspect_with_the_wrong_key_fails_the_hash_check() {
    #[derive(Debug, serde::Serialize, serde::Deserialize, Model)]
    #[tpt(version = 1, name = "Keyed")]
    struct KeyedV1 {
        n: u32,
    }
    let id = EntityId::new_v7(1_700_000_000_000, [4; 10]);
    let bytes = save(&KeyedBlake3::new([1; 32]), id, &KeyedV1 { n: 1 }).unwrap();
    assert!(inspect::render(&bytes, Some([2; 32])).is_err());
    assert!(inspect::render(&bytes, Some([1; 32])).is_ok());
}

#[test]
fn new_migration_parses_name_and_paths() {
    let cmd = parse(&args(&["new-migration", "--name", "Cad", "src"])).unwrap();
    assert_eq!(
        cmd,
        Command::NewMigration {
            name: "Cad".into(),
            paths: vec![PathBuf::from("src")],
        }
    );
    assert!(
        parse(&args(&["new-migration", "src"])).is_err(),
        "--name is required"
    );
}

#[test]
fn scaffold_copies_the_latest_version_and_adds_a_migration() {
    let source = "use tpt_model::Model;

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
    let files = [SourceFile {
        path: PathBuf::from("src/lib.rs"),
        text: source.to_string(),
    }];
    let out = tpt_model_cli::scaffold_migration(&files, "Cad").unwrap();
    assert!(out.contains("#[tpt(version = 3, name = \"Cad\")]"), "{out}");
    assert!(out.contains("pub struct CadV3 {"), "{out}");
    assert!(
        out.contains("    pub mass: f64,"),
        "fields are copied: {out}"
    );
    assert!(out.contains("pub struct CadV2ToV3;"), "{out}");
    assert!(out.contains("type From = CadV2;"), "{out}");
    assert!(out.contains("type To = CadV3;"), "{out}");
    assert!(
        !out.contains("CadV1"),
        "older versions are not copied: {out}"
    );
}

#[test]
fn scaffold_reports_an_unknown_type() {
    let files = [SourceFile {
        path: PathBuf::from("src/lib.rs"),
        text: "fn main() {}\n".into(),
    }];
    let err = tpt_model_cli::scaffold_migration(&files, "Nope").unwrap_err();
    assert!(err.contains("no versioned model named `Nope`"), "{err}");
}
