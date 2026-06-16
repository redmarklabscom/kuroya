use super::*;
use kuroya_core::TextBuffer;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_file(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "kuroya-file-io-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("src")
        .join("main.rs")
}

fn assert_no_write_temps(path: &Path) {
    let parent = path.parent().unwrap();
    assert_no_write_temps_in(parent);
}

fn assert_no_write_temps_in(parent: &Path) {
    let temp_count = fs::read_dir(parent)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.contains(".tmp."))
        })
        .count();
    assert_eq!(temp_count, 0);
}

#[tokio::test]
async fn text_file_read_rejects_oversized_files_before_decode() {
    let path = temp_file("oversized");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"hello").unwrap();

    let error = read_text_file_with_limit(&path, 4).await.unwrap_err();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("5 B"));
    assert!(error.contains("4 B"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sync_text_file_read_rejects_oversized_files_before_reading() {
    let path = temp_file("sync-oversized");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"hello").unwrap();

    let error = read_utf8_text_file_with_limit(&path, 4).unwrap_err();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("5 B"));
    assert!(error.contains("4 B"));

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn text_file_read_decodes_files_within_limit() {
    let path = temp_file("decode");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"ok\n").unwrap();

    let decoded = read_text_file_with_limit(&path, 8).await.unwrap();

    assert_eq!(decoded.text, "ok\n");
    assert!(!decoded.lossy);
    assert!(!decoded.binary);

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn text_file_read_rejects_directory_targets_before_reading() {
    let path = temp_file("directory-target");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(&path).unwrap();

    let error = read_text_file_with_limit(&path, 8).await.unwrap_err();

    assert!(error.contains("open target is not a file"));
    assert!(error.contains(&path.display().to_string()));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn file_size_limit_only_rejects_bytes_over_limit() {
    assert!(!file_size_exceeds_limit(4, 4));
    assert!(file_size_exceeds_limit(5, 4));
    assert!(!file_size_exceeds_limit(u64::MAX, 0));
}

#[test]
fn sync_text_file_read_treats_zero_limit_as_unlimited() {
    let path = temp_file("sync-unlimited");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"hello").unwrap();

    let text = read_utf8_text_file_with_limit(&path, 0).unwrap();

    assert_eq!(text, "hello");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sync_text_file_read_rejects_directory_targets_before_reading() {
    let path = temp_file("sync-directory-target-read");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(&path).unwrap();

    let error = read_utf8_text_file_with_limit(&path, 8).unwrap_err();

    assert!(error.contains("open target is not a file"));
    assert!(error.contains(&path.display().to_string()));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_open_errors_skip_non_file_metadata_probe() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);

    assert!(!open_error_needs_non_file_probe(&error));
}

#[test]
fn invalid_input_open_errors_skip_non_file_metadata_probe() {
    let error = std::io::Error::from(std::io::ErrorKind::InvalidInput);

    assert!(!open_error_needs_non_file_probe(&error));
}

#[test]
fn directory_open_errors_keep_non_file_metadata_probe() {
    let error = std::io::Error::from(std::io::ErrorKind::IsADirectory);

    assert!(open_error_needs_non_file_probe(&error));
}

#[test]
fn permission_denied_open_errors_keep_non_file_metadata_probe() {
    let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);

    assert!(open_error_needs_non_file_probe(&error));
}

#[test]
fn atomic_text_write_replaces_existing_file_without_temp_files() {
    let path = temp_file("sync");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();

    write_text_atomic(&path, "first\n").unwrap();
    write_text_atomic(&path, "second\n").unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "second\n");
    assert_no_write_temps(&path);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_text_write_rejects_file_parent_without_temp_files() {
    let path = temp_file("sync-file-parent");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    let blocking_parent = path.parent().unwrap();
    fs::create_dir_all(&root).unwrap();
    fs::write(blocking_parent, b"not a directory").unwrap();

    let error = write_text_atomic(&path, "new contents\n")
        .unwrap_err()
        .to_string();

    assert!(error.contains("save parent path is not a directory"));
    assert_eq!(
        fs::read_to_string(blocking_parent).unwrap(),
        "not a directory"
    );
    assert_no_write_temps_in(&root);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_text_write_rejects_directory_target_without_temp_files() {
    let path = temp_file("sync-directory-target");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    fs::create_dir_all(&path).unwrap();

    let error = write_text_atomic(&path, "new contents\n")
        .unwrap_err()
        .to_string();

    assert!(error.contains("save target is a directory"));
    assert!(path.is_dir());
    assert_no_write_temps(&path);

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn atomic_text_snapshot_write_async_replaces_existing_file_without_temp_files() {
    let path = temp_file("async-snapshot");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    let buffer = TextBuffer::from_text(1, None, "first\nsecond\n".to_owned());

    write_text_snapshot_atomic_async(&path, buffer.text_snapshot())
        .await
        .unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    assert_no_write_temps(&path);

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn atomic_text_snapshot_write_async_rejects_directory_target_without_temp_files() {
    let path = temp_file("async-directory-target");
    let root = path.parent().and_then(Path::parent).unwrap().to_path_buf();
    let buffer = TextBuffer::from_text(1, None, "new contents\n".to_owned());
    fs::create_dir_all(&path).unwrap();

    let error = write_text_snapshot_atomic_async(&path, buffer.text_snapshot())
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("save target is a directory"));
    assert!(path.is_dir());
    assert_no_write_temps(&path);

    fs::remove_dir_all(root).unwrap();
}
