use serde_json::{json, Value};
use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
fn cli(root: &Path, args: &[&str], input: Option<Value>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_creatorx"))
        .args(args)
        .env("CREATORX_LIBRARY_DIR", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(value) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(value.to_string().as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}
#[test]
fn cli_and_native_store_share_revision_protection_across_processes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();
    let handles: Vec<_> = ["first", "second"].into_iter().map(|body| {
        let root = root.clone();
        std::thread::spawn(move || cli(&root, &["library", "save", "--stdin", "--if-revision", "0"],
            Some(json!({"id":"concurrent-prompt","kind":"prompt","name":"Concurrent prompt","prompt":body,"category":"Tests"}))))
    }).collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status.success())
            .count(),
        1
    );
    let output = cli(&root, &["library", "get", "concurrent-prompt"], None);
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["revision"], 1);
    let missing_revision = cli(
        &root,
        &["library", "save", "--stdin"],
        Some(result["data"].clone()),
    );
    assert!(!missing_revision.status.success());
    let stale = cli(
        &root,
        &["library", "save", "--stdin", "--if-revision", "0"],
        Some(result["data"].clone()),
    );
    assert!(!stale.status.success());
    let output = cli(
        &root,
        &[
            "library",
            "list",
            "--category",
            "Tests",
            "--search",
            "prompt",
        ],
        None,
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["count"], 1);
}
#[test]
fn imports_complete_skill_without_running_scripts_and_preserves_resources_on_edit() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir_all(source.join("scripts")).unwrap();
    std::fs::create_dir_all(source.join("assets")).unwrap();
    std::fs::write(source.join("SKILL.md"), "# Example\nUse the reference.").unwrap();
    std::fs::write(
        source.join("scripts/helper.py"),
        "raise RuntimeError('must not execute')",
    )
    .unwrap();
    std::fs::write(source.join("assets/reference.bin"), [0, 1, 2, 255]).unwrap();
    let root = directory.path().join("library");
    let output = cli(
        &root,
        &[
            "library",
            "import-skill",
            "--folder",
            source.to_str().unwrap(),
        ],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut entry: Value = serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone();
    let id = entry["id"].as_str().unwrap().to_string();
    let package = root.join("skills").join(&id);
    assert_eq!(
        std::fs::read(package.join("assets/reference.bin")).unwrap(),
        [0, 1, 2, 255]
    );
    entry["skillMarkdown"] = json!("# Revised\nPreserve helper and reference.");
    let output = cli(
        &root,
        &["library", "save", "--stdin", "--if-revision", "1"],
        Some(entry),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entry: Value = serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone();
    assert_eq!(
        std::fs::read(package.join("assets/reference.bin")).unwrap(),
        [0, 1, 2, 255]
    );
    assert!(std::fs::read_to_string(package.join("scripts/helper.py"))
        .unwrap()
        .contains("must not execute"));
    std::fs::write(package.join("SKILL.md"), "# External edit").unwrap();
    let output = cli(
        &root,
        &["library", "save", "--stdin", "--if-revision", "2"],
        Some(entry),
    );
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(package.join("SKILL.md")).unwrap(),
        "# External edit"
    );
}
