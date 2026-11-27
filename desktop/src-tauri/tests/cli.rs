use serde_json::{json, Value};
use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn cli(root: &Path, args: &[&str], input: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_creatorx"))
        .args(args)
        .env("CREATORX_LIBRARY_DIR", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(text) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}

#[test]
fn cli_and_mcp_share_controlled_tags_and_revision_protection() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();

    // Register controlled tags first
    assert!(cli(&root, &["tag", "add", "--name", "安静"], None).status.success());
    assert!(cli(&root, &["tag", "add", "--name", "生图"], None).status.success());

    // Concurrent save race: exactly one wins revision 1
    let handles: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|body| {
            let root = root.clone();
            std::thread::spawn(move || {
                let payload = json!({
                    "id": "concurrent-prompt",
                    "kind": "prompt",
                    "modality": "image",
                    "name": "Concurrent prompt",
                    "prompt": body,
                    "tags": ["安静", "生图"]
                })
                .to_string();
                cli(&root, &["library", "save", "--stdin", "--if-revision", "0"], Some(&payload))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.status.success()).count(), 1);

    // Test stdio MCP server initialize + tools/call in real process
    let mcp_requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}).to_string(),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"creatorx_asset_list","arguments":{"tag":"安静"}}}).to_string(),
    ]
    .join("\n")
        + "\n";
    let mcp_out = cli(&root, &["mcp"], Some(&mcp_requests));
    assert!(mcp_out.status.success());
    let lines: Vec<Value> = String::from_utf8(mcp_out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "creatorx");
    assert_eq!(lines[1]["result"]["isError"], false);
    assert!(lines[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("concurrent-prompt"));
}

#[test]
fn imports_complete_skill_via_cli_without_running_scripts() {
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
    assert!(cli(&root, &["tag", "add", "--name", "安静"], None).status.success());
    let output = cli(
        &root,
        &[
            "library",
            "import-skill",
            "--folder",
            source.to_str().unwrap(),
            "--modality",
            "image",
            "--tags",
            "安静",
        ],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entry: Value = serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone();
    let id = entry["id"].as_str().unwrap().to_string();
    let package = root.join("skills").join(&id);
    assert_eq!(
        std::fs::read(package.join("assets/reference.bin")).unwrap(),
        [0, 1, 2, 255]
    );
}
