use super::super::super::{capture as vim_capture, editing};
use kuroya_core::{Command, EditorVimKeyOverride, EditorVimSettings};

#[test]
fn captured_vim_key_updates_builtin_and_preserves_conflict_errors() {
    let mut vim = EditorVimSettings::default();

    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::BuiltIn("h"),
            "j".to_owned()
        ),
        Err("Already used by Move down".to_owned())
    );
    assert_eq!(editing::vim_builtin_effective_binding(&vim, "h"), "h");

    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::BuiltIn("h"),
            "H".to_owned()
        ),
        Ok(())
    );

    assert_eq!(editing::vim_builtin_effective_binding(&vim, "h"), "H");
    assert!(vim.disabled_bindings.iter().any(|binding| binding == "h"));
    assert!(vim.key_overrides.iter().any(|binding| {
        binding.before == "H" && binding.after == "h" && binding.command.is_none()
    }));
}

#[test]
fn captured_vim_key_updates_custom_rows() {
    let mut vim = EditorVimSettings {
        disabled_bindings: vec![String::new()],
        key_overrides: vec![
            EditorVimKeyOverride {
                before: "K".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "Q".to_owned(),
                after: String::new(),
                command: Some(Command::RequestHover),
            },
        ],
    };

    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomDisabled(0),
            "Z".to_owned()
        ),
        Ok(())
    );
    assert_eq!(vim.disabled_bindings[0], "Z");

    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomOverrideBefore(0),
            "H".to_owned()
        ),
        Ok(())
    );
    assert_eq!(vim.key_overrides[0].before, "H");

    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomOverrideAfter(0),
            "G".to_owned()
        ),
        Ok(())
    );
    assert_eq!(vim.key_overrides[0].after, "G");

    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomOverrideAfter(1),
            "h".to_owned()
        ),
        Err("Switch the target to Vim keys before capturing After".to_owned())
    );
}

#[test]
fn canceled_vim_key_capture_restores_custom_rows() {
    let mut vim = EditorVimSettings {
        disabled_bindings: vec!["Z".to_owned()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "H".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    };
    let mut state = vim_capture::VimKeyCaptureState::default();

    state.start(
        vim_capture::VimKeyCaptureTarget::CustomDisabled(0),
        "Z".to_owned(),
    );
    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomDisabled(0),
            "Q".to_owned()
        ),
        Ok(())
    );
    vim_capture::cancel_vim_key_capture(&mut vim, &mut state);
    assert_eq!(vim.disabled_bindings[0], "Z");

    state.start(
        vim_capture::VimKeyCaptureTarget::CustomOverrideBefore(0),
        "H".to_owned(),
    );
    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomOverrideBefore(0),
            "Q".to_owned()
        ),
        Ok(())
    );
    vim_capture::cancel_vim_key_capture(&mut vim, &mut state);
    assert_eq!(vim.key_overrides[0].before, "H");

    state.start(
        vim_capture::VimKeyCaptureTarget::CustomOverrideAfter(0),
        "0".to_owned(),
    );
    assert_eq!(
        vim_capture::apply_captured_vim_key(
            &mut vim,
            vim_capture::VimKeyCaptureTarget::CustomOverrideAfter(0),
            "G".to_owned()
        ),
        Ok(())
    );
    vim_capture::cancel_vim_key_capture(&mut vim, &mut state);
    assert_eq!(vim.key_overrides[0].after, "0");
    assert!(state.target.is_none());
    assert!(state.original.is_none());
}
