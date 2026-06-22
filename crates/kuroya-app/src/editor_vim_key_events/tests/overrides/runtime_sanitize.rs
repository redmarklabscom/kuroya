use super::*;

#[test]
fn vim_runtime_settings_sanitizer_drops_unsupported_bindings() {
    let mut settings = EditorVimSettings {
        disabled_bindings: vec!["x".to_owned(), "<Nope>".to_owned(), "<Left>".to_owned()],
        key_overrides: vec![
            EditorVimKeyOverride {
                before: "K".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "H".to_owned(),
                after: "<Left>".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "<Nope>".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "<Home>".to_owned(),
                after: String::new(),
                command: Some(kuroya_core::Command::RequestHover),
            },
        ],
    };

    assert!(sanitize_vim_settings_for_runtime(&mut settings));
    assert_eq!(
        settings.disabled_bindings,
        ["x".to_owned(), "<Left>".to_owned()]
    );
    assert_eq!(
        settings.key_overrides,
        [
            EditorVimKeyOverride {
                before: "K".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "<Home>".to_owned(),
                after: String::new(),
                command: Some(kuroya_core::Command::RequestHover),
            },
        ]
    );
}

#[test]
fn vim_runtime_settings_sanitizer_drops_shadowed_and_duplicate_overrides() {
    let mut settings = EditorVimSettings {
        disabled_bindings: vec!["<Esc>".to_owned(), "<Escape>".to_owned(), "x".to_owned()],
        key_overrides: vec![
            EditorVimKeyOverride {
                before: "<Escape>".to_owned(),
                after: String::new(),
                command: Some(kuroya_core::Command::RequestHover),
            },
            EditorVimKeyOverride {
                before: "x".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "K".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "K".to_owned(),
                after: String::new(),
                command: Some(kuroya_core::Command::RequestHover),
            },
        ],
    };

    assert!(sanitize_vim_settings_for_runtime(&mut settings));
    assert_eq!(
        settings.disabled_bindings,
        ["<Esc>".to_owned(), "x".to_owned()]
    );
    assert_eq!(
        settings.key_overrides,
        [EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }]
    );
}
