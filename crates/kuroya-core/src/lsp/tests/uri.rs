use super::*;

#[test]
fn file_uri_to_path_accepts_only_local_file_uris() {
    assert!(
        file_uri_to_path("file:///tmp/main.rs")
            .is_some_and(|path| path.ends_with(Path::new("tmp").join("main.rs")))
    );
    assert!(
        file_uri_to_path("file://localhost/tmp/main.rs")
            .is_some_and(|path| path.ends_with(Path::new("tmp").join("main.rs")))
    );
    assert!(
        file_uri_to_path("file://LOCALHOST/tmp/main.rs")
            .is_some_and(|path| path.ends_with(Path::new("tmp").join("main.rs")))
    );
    #[cfg(windows)]
    assert_eq!(
        file_uri_to_path("file://server/share/main.rs"),
        Some(PathBuf::from(r"\\server\share\main.rs"))
    );
    #[cfg(windows)]
    assert_eq!(
        file_uri_to_path("file://build-server.example/share/main.rs"),
        Some(PathBuf::from(r"\\build-server.example\share\main.rs"))
    );
    #[cfg(windows)]
    assert_eq!(
        file_uri_to_path("file://server/shared%20folder/main%20file.rs"),
        Some(PathBuf::from(r"\\server\shared folder\main file.rs"))
    );
    #[cfg(not(windows))]
    assert!(file_uri_to_path("file://server/share/main.rs").is_none());
    assert!(file_uri_to_path("file:////server/share/main.rs").is_none());
    assert!(file_uri_to_path("file://").is_none());
}

#[test]
fn file_uri_to_path_rejects_query_fragment_and_invalid_percent_encoding() {
    assert!(file_uri_to_path("file:///tmp/main.rs?version=1").is_none());
    assert!(file_uri_to_path("file:///tmp/main.rs#L10").is_none());
    assert!(file_uri_to_path("file:///tmp/main%GG.rs").is_none());
    assert!(file_uri_to_path("file:///tmp/main%A.rs").is_none());
    assert!(file_uri_to_path("file:///tmp/main%FF.rs").is_none());
    assert!(file_uri_to_path("file:///tmp/main%00.rs").is_none());
    assert!(
        file_uri_to_path(&format!("file:///tmp/{}.rs", "a".repeat(MAX_LSP_URI_BYTES))).is_none()
    );
    assert!(
        file_uri_to_path("file:///tmp/main%3Fquery%23fragment.rs").is_some_and(|path| {
            path.ends_with(Path::new("tmp").join("main?query#fragment.rs"))
        })
    );
}

#[cfg(windows)]
#[test]
fn file_uri_to_path_preserves_windows_local_roots() {
    assert_eq!(
        file_uri_to_path("file:///C:/tmp/main.rs"),
        Some(PathBuf::from(r"C:\tmp\main.rs"))
    );
    assert_eq!(
        file_uri_to_path("file://localhost/C:/tmp/main.rs"),
        Some(PathBuf::from(r"C:\tmp\main.rs"))
    );
    assert_eq!(
        file_uri_to_path("file:///c:/tmp/main.rs"),
        Some(PathBuf::from(r"c:\tmp\main.rs"))
    );
    assert_eq!(
        file_uri_to_path("file:///tmp/main.rs"),
        Some(PathBuf::from(r"\tmp\main.rs"))
    );
}

#[cfg(windows)]
#[test]
fn file_uri_to_path_rejects_drive_relative_windows_uris() {
    for uri in [
        "file:///C:",
        "file:///C:relative.rs",
        "file:///C:%5Ctmp%5Cmain.rs",
        "file://localhost/C:relative.rs",
    ] {
        assert!(file_uri_to_path(uri).is_none(), "{uri}");
    }
}

#[cfg(windows)]
#[test]
fn file_uri_to_path_rejects_malformed_windows_authorities() {
    for uri in [
        "file://C:/repo/main.rs",
        "file://server%GG/share/main.rs",
        "file://server%20/share/main.rs",
        "file://server//share/main.rs",
        "file://localhost//server/share/main.rs",
        "file://localhost/%2Fserver/share/main.rs",
        r"file://server\share/main.rs",
    ] {
        assert!(file_uri_to_path(uri).is_none(), "{uri}");
    }
}

#[cfg(windows)]
#[test]
fn file_uri_to_path_rejects_encoded_windows_separators() {
    for uri in [
        "file:///C:%2Ftmp%2Fmain.rs",
        "file:///C:/repo/src%2fmain%5Cmod.rs",
        "file:///tmp%2Fmain.rs",
        "file:///tmp%5Cmain.rs",
        r"file:///tmp\main.rs",
        r"file:///C:/tmp\main.rs",
        "file://localhost/C:%2Ftmp%2Fmain.rs",
        r"file://localhost/C:/tmp\main.rs",
        "file://localhost/%2Fserver/share/main.rs",
        r"file://localhost/tmp\main.rs",
        "file:///%2Fserver/share/main.rs",
        "file://server/%2Fshare/main.rs",
        "file://server/%5Cshare/main.rs",
        "file://server/share%2Fdir/main.rs",
        "file://server/share%5Cdir/main.rs",
        "file://server/share/src%2Fmain%5cmod.rs",
        r"file://server/share\main.rs",
    ] {
        assert!(file_uri_to_path(uri).is_none(), "{uri}");
    }
}

#[test]
fn percent_encode_uri_path_escapes_reserved_and_utf8_bytes() {
    assert_eq!(
        percent_encode_uri_path("/src/plain-_.~:AZaz09.rs"),
        "/src/plain-_.~:AZaz09.rs"
    );
    assert_eq!(
        percent_encode_uri_path("/src/a b%#?\u{03bb}.rs"),
        "/src/a%20b%25%23%3F%CE%BB.rs"
    );
}

#[test]
fn path_to_file_uri_builds_relative_file_uri_without_extra_slashes() {
    let uri = path_to_file_uri(Path::new("src/main file.rs"));

    assert_eq!(uri, "file:///src/main%20file.rs");
}

#[cfg(windows)]
#[test]
fn path_to_file_uri_builds_windows_drive_and_unc_uris() {
    assert_eq!(
        path_to_file_uri(Path::new(r"C:\tmp\main file.rs")),
        "file:///C:/tmp/main%20file.rs"
    );
    assert_eq!(
        path_to_file_uri(Path::new(r"\\server\share\main file.rs")),
        "file://server/share/main%20file.rs"
    );
    assert_eq!(
        path_to_file_uri(Path::new(r"\\?\C:\tmp\main.rs")),
        "file:///C:/tmp/main.rs"
    );
    assert_eq!(
        path_to_file_uri(Path::new(r"\\?\UNC\server\share\main.rs")),
        "file://server/share/main.rs"
    );
}

#[test]
fn percent_decode_uri_path_avoids_allocation_without_escapes() {
    assert!(matches!(
        percent_decode_uri_path("/tmp/main.rs"),
        Some(Cow::Borrowed("/tmp/main.rs"))
    ));
    assert!(matches!(
        percent_decode_uri_path("/tmp/main%20file.rs"),
        Some(Cow::Owned(decoded)) if decoded == "/tmp/main file.rs"
    ));
    assert!(percent_decode_uri_path("/tmp/main\0file.rs").is_none());
}

#[test]
fn generated_file_uris_round_trip_special_path_characters() {
    let path = Path::new("src").join(format!("space and unicode {} 100% #?.rs", '\u{03bb}'));
    let uri = path_to_file_uri(&path);

    assert!(uri.contains("%20"), "{uri}");
    assert!(uri.contains("%25"), "{uri}");
    assert!(uri.contains("%23"), "{uri}");
    assert!(uri.contains("%3F"), "{uri}");
    assert!(
        file_uri_to_path(&uri).is_some_and(|decoded| decoded.ends_with(path)),
        "{uri}"
    );
}

#[test]
fn lsp_location_parsers_drop_malformed_file_uris() {
    let good_uri = path_to_file_uri(Path::new("src/good.rs"));
    let references = json!({
        "jsonrpc": "2.0",
        "id": 26,
        "result": [
            {
                "uri": "file:///src/query.rs?version=1",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 4 }
                }
            },
            {
                "targetUri": "file://server",
                "targetSelectionRange": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 4 }
                }
            },
            {
                "uri": good_uri,
                "range": {
                    "start": { "line": 2, "character": 0 },
                    "end": { "line": 2, "character": 4 }
                }
            }
        ]
    });

    let references = parse_references_response(&references).unwrap();

    assert_eq!(references.len(), 1);
    assert!(
        references[0]
            .path
            .ends_with(Path::new("src").join("good.rs"))
    );

    let definition = json!({
        "jsonrpc": "2.0",
        "id": 12,
        "result": {
            "uri": "file:///src/main.rs#L10",
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 4 }
            }
        }
    });

    assert!(parse_definition_response(&definition).is_none());
}
