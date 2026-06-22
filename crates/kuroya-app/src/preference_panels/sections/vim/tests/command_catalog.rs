use crate::{command_catalog::command_catalog_slice, commands::command_label};

#[test]
fn vim_command_dropdown_uses_stable_unique_catalog_commands() {
    let catalog = command_catalog_slice();
    let mut seen = Vec::new();

    assert!(!catalog.is_empty());
    for command in catalog {
        assert!(
            command.is_stable_keymap_command(),
            "Vim command dropdown cannot persist unstable command {command:?}"
        );
        assert!(
            !seen.iter().any(|seen_command| seen_command == command),
            "duplicate Vim command dropdown entry {command:?}"
        );
        assert!(
            !command_label(command).trim().is_empty(),
            "Vim command dropdown entry has an empty label: {command:?}"
        );
        seen.push(command.clone());
    }
}
