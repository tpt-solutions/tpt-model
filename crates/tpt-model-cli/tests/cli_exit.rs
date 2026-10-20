//! The binaries: exit codes, the help text, and `inspect` on a missing file.

use std::process::Command;

fn tool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tpt-model"))
}

#[test]
fn help_exits_zero_and_lists_every_command() {
    let out = tool().arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    for command in ["squash", "inspect", "new-migration"] {
        assert!(text.contains(command), "{text}");
    }
}

#[test]
fn usage_errors_exit_two() {
    let out = tool().arg("frobnicate").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown command"));
}

#[test]
fn refused_squash_exits_one() {
    let dir = std::env::temp_dir().join(format!("tpt-model-exit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("lib.rs");
    std::fs::write(
        &file,
        "#[derive(Model)]\n#[tpt(version = 1, name = \"X\")]\npub struct XV1 { pub a: u32 }\n",
    )
    .unwrap();
    let out = tool()
        .args(["squash", "--min-supported", "9"])
        .arg(&file)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("refusing"));
}

#[test]
fn inspect_of_a_missing_file_exits_one() {
    let out = tool()
        .args(["inspect", "definitely-missing.bin"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}
