use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use git_viewme_core::{
    FileContent, TreeEntry, TreeEntryKind, file_content_from_response, tree_from_response,
};
use serde_json::json;

fn contents(bytes: &[u8]) -> String {
    let encoded = STANDARD.encode(bytes);
    // GitHub wraps base64 content in lines of 60 characters.
    let lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(60)
        .map(|line| std::str::from_utf8(line).unwrap())
        .collect();
    json!({ "type": "file", "size": bytes.len(), "encoding": "base64", "content": lines.join("\n") }).to_string()
}

#[test]
fn file_content_from_response_utf8_is_text() {
    let text = "# git-viewme\nUsed to manage repo views. \u{2713}\n".repeat(5);

    let content = file_content_from_response(&contents(text.as_bytes())).unwrap();

    assert_eq!(content, FileContent::Text { text });
}

#[test]
fn file_content_from_response_byte_order_mark_is_dropped() {
    let content = file_content_from_response(&contents("\u{feff}hello".as_bytes())).unwrap();

    assert_eq!(
        content,
        FileContent::Text {
            text: "hello".into()
        }
    );
}

#[test]
fn file_content_from_response_nul_byte_is_binary() {
    let content = file_content_from_response(&contents(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")).unwrap();

    assert_eq!(content, FileContent::Binary { size: 16 });
}

#[test]
fn file_content_from_response_invalid_utf8_is_binary() {
    let content = file_content_from_response(&contents(&[0xff, 0xfe, 0x41])).unwrap();

    assert_eq!(content, FileContent::Binary { size: 3 });
}

#[test]
fn file_content_from_response_over_one_megabyte_is_too_large() {
    let body =
        json!({ "type": "file", "size": 1_048_577, "encoding": "none", "content": "" }).to_string();

    assert_eq!(
        file_content_from_response(&body).unwrap(),
        FileContent::TooLarge { size: 1_048_577 }
    );
}

#[test]
fn file_content_from_response_exactly_one_megabyte_is_text() {
    let text = "a".repeat(1_048_576);

    let content = file_content_from_response(&contents(text.as_bytes())).unwrap();

    assert_eq!(content, FileContent::Text { text });
}

#[test]
fn file_content_serializes_as_tagged_union() {
    assert_eq!(
        serde_json::to_value(FileContent::Text { text: "x".into() }).unwrap(),
        json!({ "kind": "text", "text": "x" })
    );
    assert_eq!(
        serde_json::to_value(FileContent::TooLarge { size: 2 }).unwrap(),
        json!({ "kind": "too_large", "size": 2 })
    );
}

#[test]
fn tree_from_response_orders_folders_first_depth_first_and_skips_submodules() {
    let body = json!({ "truncated": false, "tree": [
        { "path": "README.md", "type": "blob", "size": 10 },
        { "path": "src", "type": "tree" },
        { "path": "src/main.rs", "type": "blob", "size": 5 },
        { "path": "src/app", "type": "tree" },
        { "path": "src/app/view.rs", "type": "blob", "size": 7 },
        { "path": ".github", "type": "tree" },
        { "path": ".github/ci.yml", "type": "blob", "size": 3 },
        { "path": "Cargo.toml", "type": "blob", "size": 4 },
        { "path": "vendor", "type": "commit" },
        { "path": "assets", "type": "tree" }
    ]})
    .to_string();

    let entries = tree_from_response(&body).unwrap();

    let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            ".github",
            ".github/ci.yml",
            "assets",
            "src",
            "src/app",
            "src/app/view.rs",
            "src/main.rs",
            "Cargo.toml",
            "README.md"
        ]
    );
    assert_eq!(
        entries[0],
        TreeEntry {
            path: ".github".into(),
            kind: TreeEntryKind::Dir,
            size: None
        }
    );
    assert_eq!(
        entries[8],
        TreeEntry {
            path: "README.md".into(),
            kind: TreeEntryKind::File,
            size: Some(10)
        }
    );
}

#[test]
fn tree_entry_kind_serializes_as_lowercase_tag() {
    assert_eq!(
        serde_json::to_value(TreeEntryKind::Dir).unwrap(),
        json!("dir")
    );
}
