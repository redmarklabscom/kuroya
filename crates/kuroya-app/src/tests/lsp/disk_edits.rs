use crate::lsp_lifecycle::{LSP_DISK_EDIT_MAX_BYTES, read_lsp_disk_edit_text};
use std::env;

#[test]
fn lsp_disk_edit_text_reader_rejects_unsafe_files() {
    let root = env::temp_dir().join(format!(
        "kuroya-lsp-disk-edit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let valid = root.join("valid.rs");
    let binary = root.join("binary.dat");
    let invalid = root.join("invalid.txt");
    let large = root.join("large.rs");

    std::fs::write(&valid, b"needle\n").unwrap();
    std::fs::write(&binary, b"needle\0binary").unwrap();
    std::fs::write(&invalid, vec![b'n', b'e', 0xff]).unwrap();
    std::fs::write(&large, b"too large").unwrap();

    assert_eq!(
        read_lsp_disk_edit_text(&valid, LSP_DISK_EDIT_MAX_BYTES).unwrap(),
        "needle\n"
    );
    assert_eq!(
        read_lsp_disk_edit_text(&binary, LSP_DISK_EDIT_MAX_BYTES).unwrap_err(),
        "binary file skipped"
    );
    assert_eq!(
        read_lsp_disk_edit_text(&invalid, LSP_DISK_EDIT_MAX_BYTES).unwrap_err(),
        "invalid UTF-8 file skipped"
    );
    assert!(
        read_lsp_disk_edit_text(&large, 4)
            .unwrap_err()
            .starts_with("file too large for LSP disk edit")
    );

    std::fs::remove_dir_all(root).unwrap();
}
