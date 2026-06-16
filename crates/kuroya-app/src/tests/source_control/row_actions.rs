use super::*;

#[test]
fn source_control_row_actions_follow_always_show_setting_selection_and_hover() {
    assert!(!source_control_row_actions_visible(false, false, false));
    assert!(source_control_row_actions_visible(false, true, false));
    assert!(source_control_row_actions_visible(false, false, true));
    assert!(source_control_row_actions_visible(true, false, false));
}

#[test]
fn source_control_row_click_command_follows_open_diff_on_click_setting() {
    let path = PathBuf::from("C:/repo/src/main.rs");
    let entry = GitStatusEntry {
        path: path.clone(),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Unstaged,
    };

    assert_eq!(
        source_control_row_click_command(true, &entry),
        Some(Command::OpenFileChanges(path.clone()))
    );
    assert_eq!(source_control_row_click_command(false, &entry), None);

    let staged = GitStatusEntry {
        path: path.clone(),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Staged,
    };
    assert_eq!(
        source_control_row_click_command(true, &staged),
        Some(Command::OpenStagedFileChanges(path))
    );
}

#[test]
fn source_control_row_actions_match_vscode_stage_controls() {
    assert_eq!(
        source_control_row_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Modified,
            true,
            false,
            true
        ),
        vec![
            "Open Changes",
            "Compare with HEAD",
            "Open File at HEAD",
            "Open File at Index",
            "Select for Compare",
            "Copy Patch",
            "Reveal in Explorer",
            "Open File",
            "Open Blame",
            "Open Hunks",
            "Stage Changes",
            "Discard Changes"
        ]
    );
    assert_eq!(
        source_control_row_action_labels(
            GitChangeStage::Staged,
            GitFileStatus::Modified,
            true,
            true,
            true
        ),
        vec![
            "Open Staged Changes",
            "Compare with HEAD",
            "Open File at HEAD",
            "Open File at Index",
            "Select for Compare",
            "Compare with Selected",
            "Copy Patch",
            "Reveal in Explorer",
            "Open File",
            "Open Blame",
            "Open Staged Hunks",
            "Unstage Changes",
            "Discard Changes"
        ]
    );
    assert_eq!(
        source_control_row_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Conflicted,
            false,
            false,
            true
        ),
        vec![
            "Open Changes",
            "Compare with HEAD",
            "Open File at HEAD",
            "Copy Patch",
            "Reveal in Explorer",
            "Stage Changes",
            "Discard Changes"
        ]
    );
    assert!(
        !source_control_row_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Untracked,
            true,
            false,
            true
        )
        .contains(&"Open File at HEAD")
    );
    assert!(
        !source_control_row_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Untracked,
            true,
            false,
            true
        )
        .contains(&"Open File at Index")
    );
}

#[test]
fn source_control_row_actions_follow_inline_open_file_setting() {
    let visible_actions = source_control_row_action_labels(
        GitChangeStage::Unstaged,
        GitFileStatus::Modified,
        true,
        false,
        true,
    );
    assert!(visible_actions.contains(&"Open File"));

    let hidden_actions = source_control_row_action_labels(
        GitChangeStage::Unstaged,
        GitFileStatus::Modified,
        true,
        false,
        false,
    );
    assert!(!hidden_actions.contains(&"Open File"));
    assert!(hidden_actions.contains(&"Open Blame"));
    assert!(hidden_actions.contains(&"Open Hunks"));
}

#[test]
fn source_control_row_actions_show_conflict_resolve_cta_for_existing_files() {
    assert_eq!(
        source_control_row_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Conflicted,
            true,
            false,
            false
        ),
        vec![
            "Open Changes",
            "Compare with HEAD",
            "Open File at HEAD",
            "Select for Compare",
            "Copy Patch",
            "Reveal in Explorer",
            "Open File to Resolve",
            "Open Blame",
            "Stage Changes",
            "Discard Changes"
        ]
    );

    let visible_inline_actions = source_control_row_action_labels(
        GitChangeStage::Unstaged,
        GitFileStatus::Conflicted,
        true,
        false,
        true,
    );
    assert!(visible_inline_actions.contains(&"Open File to Resolve"));
    assert!(!visible_inline_actions.contains(&"Open File"));

    let missing_conflict_actions = source_control_row_action_labels(
        GitChangeStage::Unstaged,
        GitFileStatus::Conflicted,
        false,
        false,
        false,
    );
    assert!(!missing_conflict_actions.contains(&"Open File to Resolve"));
}

#[test]
fn source_control_row_actions_route_conflict_resolve_cta_to_open_file() {
    let path = PathBuf::from("C:/repo/src/conflict.rs");
    let entry = GitStatusEntry {
        path: path.clone(),
        status: GitFileStatus::Conflicted,
        stage: GitChangeStage::Unstaged,
    };
    let actions = source_control_row_action_label_commands(&entry, true, false, false);

    assert!(
        actions
            .iter()
            .any(|(label, command)| *label == "Open File to Resolve"
                && command == &Command::OpenFile(path.clone()))
    );
    assert!(!actions.iter().any(|(label, _)| *label == "Open File"));
}

#[test]
fn source_control_keyboard_actions_match_selected_row_commands() {
    assert_eq!(
        source_control_keyboard_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Modified,
            true
        ),
        vec![
            "Enter Open Changes",
            "C Compare with HEAD",
            "Alt+H Open File at HEAD",
            "Alt+I Open File at Index",
            "P Copy Patch",
            "Alt+C Copy Path",
            "Alt+Shift+C Copy Relative Path",
            "Alt+S Show Path",
            "R Reveal in Explorer",
            "O Open File",
            "H Open Hunks",
            "S Stage Changes",
            "Delete Discard Changes",
            "B Open Blame"
        ]
    );
    assert_eq!(
        source_control_keyboard_action_labels(
            GitChangeStage::Staged,
            GitFileStatus::Modified,
            true
        ),
        vec![
            "Enter Open Staged Changes",
            "C Compare with HEAD",
            "Alt+H Open File at HEAD",
            "Alt+I Open File at Index",
            "P Copy Patch",
            "Alt+C Copy Path",
            "Alt+Shift+C Copy Relative Path",
            "Alt+S Show Path",
            "R Reveal in Explorer",
            "O Open File",
            "H Open Staged Hunks",
            "U Unstage Changes",
            "Delete Discard Changes",
            "B Open Blame"
        ]
    );
    assert_eq!(
        source_control_keyboard_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Conflicted,
            true
        ),
        vec![
            "Enter Open Changes",
            "C Compare with HEAD",
            "Alt+H Open File at HEAD",
            "P Copy Patch",
            "Alt+C Copy Path",
            "Alt+Shift+C Copy Relative Path",
            "Alt+S Show Path",
            "R Reveal in Explorer",
            "O Open File to Resolve",
            "S Stage Changes",
            "Delete Discard Changes",
            "B Open Blame"
        ]
    );
    assert!(
        !source_control_keyboard_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Conflicted,
            false
        )
        .contains(&"O Open File to Resolve")
    );
    assert!(
        !source_control_keyboard_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Untracked,
            true
        )
        .contains(&"Alt+H Open File at HEAD")
    );
    assert!(
        !source_control_keyboard_action_labels(
            GitChangeStage::Unstaged,
            GitFileStatus::Untracked,
            true
        )
        .contains(&"Alt+I Open File at Index")
    );
}
