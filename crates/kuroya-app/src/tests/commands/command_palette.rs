use crate::{
    command_palette_items::{
        CommandPaletteQueryMemoryEntry, CommandPaletteRanker, MAX_COMMAND_PALETTE_QUERY_MEMORY,
        MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS, MAX_COMMAND_PALETTE_RECENT_COMMANDS,
        command_palette_command_match_score, command_palette_items, command_palette_match_score,
        command_palette_rank_score, normalize_command_palette_query_memory,
        normalize_recent_palette_commands, record_command_palette_query_memory,
        record_recent_palette_command, sanitize_command_palette_query_input,
        sanitize_command_palette_query_input_in_place,
    },
    commands::command_catalog,
    history::NavigationLocation,
    path_display::compact_path,
    workspace_state::paths_match_lexically,
};
use fuzzy_matcher::skim::SkimMatcherV2;
use kuroya_core::{
    Command, PluginCapabilities, PluginCommandContribution, PluginCommandRegistry,
    PluginContributions, PluginDescriptor, PluginManifest, WorkspaceTask, WorkspaceTaskKind,
    keymap::{KeyBinding, Keymap},
};
use std::collections::{BTreeMap, VecDeque};
use std::{env, path::PathBuf};

#[test]
fn command_palette_items_surface_shortcuts_and_workspace_actions() {
    let root = PathBuf::from("workspace");
    let items = command_palette_items(
        &root,
        &[],
        &[],
        &[],
        &PluginCommandRegistry::default(),
        &[KeyBinding {
            chord: "Ctrl+P".to_owned(),
            command: Command::ToggleQuickOpen,
        }],
    );

    assert!(items.iter().any(|(label, command, chord)| {
        label == "Quick Open" && command == &Command::ToggleQuickOpen && chord == "Ctrl+P"
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Toggle Read Only" && command == &Command::ToggleReadOnly
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Toggle Minimap" && command == &Command::ToggleMinimap
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Toggle Sticky Scroll" && command == &Command::ToggleStickyScroll
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Save Workspace Snapshot" && command == &Command::SaveWorkspaceSnapshot
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Restore Latest Workspace Snapshot"
            && command == &Command::RestoreLatestWorkspaceSnapshot
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open All Changes" && command == &Command::OpenAllChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open All Unstaged Changes" && command == &Command::OpenAllUnstagedChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open All Staged Changes" && command == &Command::OpenAllStagedChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy All Changes Patch" && command == &Command::CopyAllChangesPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Unstaged Changes Patch" && command == &Command::CopyUnstagedChangesPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Staged Changes Patch" && command == &Command::CopyStagedChangesPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Active File Patch" && command == &Command::CopyActiveFilePatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Active File Staged Patch" && command == &Command::CopyActiveFileStagedPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Reveal in Explorer" && command == &Command::RevealActiveFileInExplorer
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Reveal in Source Control" && command == &Command::RevealActiveFileInSourceControl
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Path" && command == &Command::CopyActiveFilePath
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Relative Path" && command == &Command::CopyActiveFileRelativePath
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Staged Changes" && command == &Command::OpenActiveFileStagedChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Compare with HEAD" && command == &Command::OpenActiveFileHeadChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open File at HEAD" && command == &Command::OpenActiveFileHeadRevision
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open File at Index" && command == &Command::OpenActiveFileIndexRevision
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Select Active File for Compare" && command == &Command::SelectActiveFileForCompare
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Compare Active File with Selected"
            && command == &Command::CompareActiveFileWithSelected
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Compare Active File with Saved" && command == &Command::CompareActiveFileWithSaved
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Next Git Change" && command == &Command::NextGitChange
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Previous Git Change" && command == &Command::PreviousGitChange
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Next Diff Hunk" && command == &Command::NextDiffHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Previous Diff Hunk" && command == &Command::PreviousDiffHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Refresh Diff" && command == &Command::RefreshActiveDiff
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Swap Compare Sides" && command == &Command::SwapActiveDiffSides
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Hunks" && command == &Command::OpenActiveFileHunks
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Staged Hunks" && command == &Command::OpenActiveFileStagedHunks
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Blame" && command == &Command::OpenActiveFileBlame
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Stage All Changes" && command == &Command::StageAllChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Stage Active File Changes" && command == &Command::StageActiveFileChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Stage Current Hunk" && command == &Command::StageActiveFileHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Stage Current Diff Hunk" && command == &Command::StageActiveDiffHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Diff Base File" && command == &Command::OpenActiveDiffBaseFile
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Base at Current Diff Hunk" && command == &Command::OpenActiveDiffHunkBase
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Diff Source File" && command == &Command::OpenActiveDiffSourceFile
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Source at Current Diff Hunk" && command == &Command::OpenActiveDiffHunkSource
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Current Hunk Diff" && command == &Command::OpenActiveFileHunkDiff
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Current Staged Hunk Diff"
            && command == &Command::OpenActiveFileStagedHunkDiff
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Accessible Diff Viewer"
            && command == &Command::OpenActiveAccessibleDiffViewer
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Current Hunk Patch" && command == &Command::CopyActiveFileHunkPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Current Staged Hunk Patch"
            && command == &Command::CopyActiveFileStagedHunkPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Diff Patch" && command == &Command::CopyActiveDiffPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Copy Current Diff Hunk Patch" && command == &Command::CopyActiveDiffHunkPatch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Unstage All Changes" && command == &Command::UnstageAllChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Unstage Active File Changes" && command == &Command::UnstageActiveFileChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Unstage Current Hunk" && command == &Command::UnstageActiveFileHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Unstage Current Diff Hunk" && command == &Command::UnstageActiveDiffHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Discard All Changes" && command == &Command::DiscardAllChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Discard Active File Changes" && command == &Command::DiscardActiveFileChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Discard Current Hunk" && command == &Command::DiscardActiveFileHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Discard Current Diff Hunk" && command == &Command::DiscardActiveDiffHunk
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Commit Staged Changes" && command == &Command::CommitStagedChanges
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Accept Current Conflict" && command == &Command::AcceptCurrentConflict
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Accept Incoming Conflict" && command == &Command::AcceptIncomingConflict
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Accept Both Conflicts" && command == &Command::AcceptBothConflicts
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Switch Git Branch" && command == &Command::ToggleGitBranchSwitcher
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Git History" && command == &Command::ToggleGitHistory
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Git Stashes" && command == &Command::ToggleGitStashes
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Open Source Control in Integrated Terminal"
            && command == &Command::OpenSourceControlInIntegratedTerminal
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Save Git Stash" && command == &Command::SaveGitStash
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Internal Devtools" && command == &Command::ToggleDevtools
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Workspace Tasks" && command == &Command::ToggleWorkspaceTasks
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Run Build Task"
            && command == &Command::RunWorkspaceTaskKind(WorkspaceTaskKind::Build)
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Run Test Task"
            && command == &Command::RunWorkspaceTaskKind(WorkspaceTaskKind::Test)
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Run Configuration"
            && command == &Command::RunWorkspaceTaskKind(WorkspaceTaskKind::Run)
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Search Terminal Output" && command == &Command::ToggleTerminalSearch
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Next Terminal Search Result" && command == &Command::NextTerminalSearchResult
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Previous Terminal Search Result"
            && command == &Command::PreviousTerminalSearchResult
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Next Terminal Session" && command == &Command::NextTerminalSession
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Previous Terminal Session" && command == &Command::PreviousTerminalSession
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Trust Workspace" && command == &Command::TrustWorkspace
    }));
    assert!(items.iter().any(|(label, command, _)| {
        label == "Revoke Workspace Trust" && command == &Command::RevokeWorkspaceTrust
    }));
    assert!(items.iter().any(|(label, command, chord)| {
        label == "New File in Workspace"
            && command == &Command::CreateFileIn(root.clone())
            && chord.is_empty()
    }));
    assert!(items.iter().any(|(label, command, chord)| {
        label == "New Folder in Workspace"
            && command == &Command::CreateFolderIn(root.clone())
            && chord.is_empty()
    }));
}

#[test]
fn command_palette_items_include_all_default_keybinding_commands_with_chords() {
    let root = PathBuf::from("workspace");
    let keymap = Keymap::default();
    let items = command_palette_items(
        &root,
        &[],
        &[],
        &[],
        &PluginCommandRegistry::default(),
        &keymap.bindings,
    );

    for binding in &keymap.bindings {
        let item = items
            .iter()
            .find(|(_, command, _)| command == &binding.command)
            .unwrap_or_else(|| {
                panic!(
                    "default keybinding command is missing from the command palette: {:?}",
                    binding.command
                )
            });
        assert_eq!(
            item.2, binding.chord,
            "command palette chord drifted for {:?}",
            binding.command
        );
    }
}

#[test]
fn command_palette_items_include_each_catalog_command_once() {
    let root = PathBuf::from("workspace");
    let items = command_palette_items(&root, &[], &[], &[], &PluginCommandRegistry::default(), &[]);

    for command in command_catalog() {
        let count = items
            .iter()
            .filter(|(_, item_command, _)| item_command == &command)
            .count();
        assert_eq!(
            count, 1,
            "command palette should include exactly one catalog entry for {:?}",
            command
        );
    }
}

#[test]
fn command_palette_items_include_existing_recent_workspaces() {
    let root = env::temp_dir().join(format!(
        "kuroya-command-palette-current-{}",
        std::process::id()
    ));
    let recent_a = env::temp_dir().join(format!(
        "kuroya-command-palette-recent-a-{}",
        std::process::id()
    ));
    let recent_b = env::temp_dir().join(format!(
        "kuroya-command-palette-recent-b-{}",
        std::process::id()
    ));
    let stale = env::temp_dir().join(format!(
        "kuroya-command-palette-stale-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&recent_a).unwrap();
    std::fs::create_dir_all(&recent_b).unwrap();

    let recents = vec![
        root.clone(),
        recent_a.clone(),
        stale.clone(),
        recent_a.clone(),
        recent_b.clone(),
    ];
    let items = command_palette_items(
        &root,
        &recents,
        &[],
        &[],
        &PluginCommandRegistry::default(),
        &[],
    );

    assert!(items.iter().any(|(label, command, chord)| {
        label == &format!("Open Recent {}", compact_path(&recent_a))
            && command == &Command::OpenWorkspace(recent_a.clone())
            && chord.is_empty()
    }));
    assert!(
        items
            .iter()
            .any(|(_, command, _)| { command == &Command::OpenWorkspace(recent_b.clone()) })
    );
    assert!(!items.iter().any(|(_, command, _)| {
        command == &Command::OpenWorkspace(root.clone())
            || command == &Command::OpenWorkspace(stale.clone())
    }));
    assert_eq!(
        items
            .iter()
            .filter(|(_, command, _)| command == &Command::OpenWorkspace(recent_a.clone()))
            .count(),
        1
    );

    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(recent_a).unwrap();
    std::fs::remove_dir_all(recent_b).unwrap();
}

#[test]
fn command_palette_recent_workspaces_skip_lexically_current_and_dedupe_equivalents() {
    let root = env::temp_dir().join(format!(
        "kuroya-command-palette-current-{}",
        std::process::id()
    ));
    let recent = env::temp_dir().join(format!(
        "kuroya-command-palette-recent-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(&recent).unwrap();

    let recents = vec![
        root.join("src").join(".."),
        recent.join("."),
        recent.clone(),
    ];
    let items = command_palette_items(
        &root,
        &recents,
        &[],
        &[],
        &PluginCommandRegistry::default(),
        &[],
    );
    let open_workspaces = items
        .iter()
        .filter_map(|(_, command, _)| match command {
            Command::OpenWorkspace(path) => Some(path),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        !open_workspaces
            .iter()
            .any(|path| paths_match_lexically(path, &root))
    );
    assert_eq!(
        open_workspaces
            .iter()
            .filter(|path| paths_match_lexically(path, &recent))
            .count(),
        1
    );

    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(recent).unwrap();
}

#[test]
fn command_palette_items_include_recent_navigation_locations() {
    let root = PathBuf::from("workspace");
    let old = NavigationLocation::new(root.join("src/lib.rs"), 3, 2);
    let recent = NavigationLocation::new(root.join("src/main.rs"), 42, 9);
    let navigation = vec![old, recent.clone(), recent.clone()];
    let items = command_palette_items(
        &root,
        &[],
        &navigation,
        &[],
        &PluginCommandRegistry::default(),
        &[],
    );

    let expected_command = Command::OpenFileAt {
        path: recent.path.clone(),
        line: recent.line,
        column: recent.column,
    };
    let expected_label = format!(
        "Go to Recent Location {}:{}:{}",
        compact_path(&recent.path),
        recent.line,
        recent.column
    );

    assert!(items.iter().any(|(label, command, chord)| {
        label == &expected_label && command == &expected_command && chord.is_empty()
    }));
    assert_eq!(
        items
            .iter()
            .filter(|(_, command, _)| command == &expected_command)
            .count(),
        1
    );
}

#[test]
fn command_palette_items_include_workspace_tasks() {
    let root = PathBuf::from("workspace");
    let test_task = WorkspaceTask {
        name: "Test All".to_owned(),
        command: "cargo".to_owned(),
        args: vec!["test".to_owned()],
        cwd: Some(root.clone()),
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Test,
        default: true,
    };
    let run_task = WorkspaceTask {
        name: "App".to_owned(),
        command: "cargo".to_owned(),
        args: vec!["run".to_owned()],
        cwd: Some(root.clone()),
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Run,
        default: true,
    };
    let items = command_palette_items(
        &root,
        &[],
        &[],
        &[test_task, run_task],
        &PluginCommandRegistry::default(),
        &[],
    );

    assert!(items.iter().any(|(label, command, chord)| {
        label == "Run Test Task Test All"
            && matches!(
                command,
                Command::RunWorkspaceTaskSnapshot {
                    index: 0,
                    fingerprint: _
                }
            )
            && chord.is_empty()
    }));
    assert!(items.iter().any(|(label, command, chord)| {
        label == "Run Configuration App"
            && matches!(
                command,
                Command::RunWorkspaceTaskSnapshot {
                    index: 1,
                    fingerprint: _
                }
            )
            && chord.is_empty()
    }));
}

#[test]
fn command_palette_workspace_task_labels_are_sanitized_and_bounded() {
    let root = PathBuf::from("workspace");
    let task = WorkspaceTask {
        name: format!("Bad\n{}\u{202e}Task", "very-long-name-".repeat(16)),
        command: "cargo".to_owned(),
        args: vec!["test".to_owned()],
        cwd: Some(root.clone()),
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Test,
        default: true,
    };

    let items = command_palette_items(
        &root,
        &[],
        &[],
        &[task],
        &PluginCommandRegistry::default(),
        &[],
    );
    let label = items
        .iter()
        .find_map(|(label, command, _)| {
            matches!(command, Command::RunWorkspaceTaskSnapshot { .. }).then_some(label)
        })
        .expect("workspace task item");

    assert!(label.starts_with("Run Test Task Bad "));
    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{202e}'));
    assert!(label.contains("..."));
}

#[test]
fn command_palette_items_include_plugin_commands_with_declared_capability() {
    let root = PathBuf::from("workspace");
    let plugin = plugin_descriptor(
        "example.plugin",
        "Example Plugin",
        true,
        vec![
            PluginCommandContribution {
                id: "example.sayHello".to_owned(),
                title: "Say Hello".to_owned(),
                category: Some("Example".to_owned()),
            },
            PluginCommandContribution {
                id: "example.sayHello".to_owned(),
                title: "Duplicate".to_owned(),
                category: Some("Example".to_owned()),
            },
        ],
    );
    let disabled = plugin_descriptor(
        "disabled.plugin",
        "Disabled Plugin",
        false,
        vec![PluginCommandContribution {
            id: "disabled.command".to_owned(),
            title: "Should Not Show".to_owned(),
            category: None,
        }],
    );

    let plugin_commands = PluginCommandRegistry::from_plugins(&[plugin, disabled]);
    let items = command_palette_items(&root, &[], &[], &[], &plugin_commands, &[]);

    assert!(items.iter().any(|(label, command, chord)| {
        label == "Example: Say Hello"
            && command
                == &Command::RunPluginCommand {
                    plugin_id: "example.plugin".to_owned(),
                    command_id: "example.sayHello".to_owned(),
                }
            && chord.is_empty()
    }));
    assert_eq!(
        items
            .iter()
            .filter(|(_, command, _)| {
                command
                    == &Command::RunPluginCommand {
                        plugin_id: "example.plugin".to_owned(),
                        command_id: "example.sayHello".to_owned(),
                    }
            })
            .count(),
        1
    );
    assert!(!items.iter().any(|(_, command, _)| {
        command
            == &Command::RunPluginCommand {
                plugin_id: "disabled.plugin".to_owned(),
                command_id: "disabled.command".to_owned(),
            }
    }));
}

#[test]
fn command_palette_plugin_command_labels_are_sanitized_and_bounded() {
    let root = PathBuf::from("workspace");
    let plugin = plugin_descriptor(
        "bad.plugin",
        "Bad Plugin",
        true,
        vec![PluginCommandContribution {
            id: "bad.command".to_owned(),
            title: format!("Run\n{}\u{202e}Task", "very-long-title-".repeat(16)),
            category: Some("Tools\u{2028}".to_owned()),
        }],
    );

    let plugin_commands = PluginCommandRegistry::from_plugins(&[plugin]);
    let items = command_palette_items(&root, &[], &[], &[], &plugin_commands, &[]);
    let label = items
        .iter()
        .find_map(|(label, command, _)| {
            matches!(command, Command::RunPluginCommand { .. }).then_some(label)
        })
        .expect("plugin command item");

    assert!(label.starts_with("Tools : Run "));
    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{2028}'));
    assert!(!label.contains('\u{202e}'));
    assert!(label.contains("..."));
}

#[test]
fn command_palette_plugin_commands_are_sorted_for_stable_discovery() {
    let root = PathBuf::from("workspace");
    let plugin_b = plugin_descriptor(
        "beta.plugin",
        "Beta Plugin",
        true,
        vec![
            PluginCommandContribution {
                id: "beta.format".to_owned(),
                title: "Format".to_owned(),
                category: Some("Tools".to_owned()),
            },
            PluginCommandContribution {
                id: "beta.zzz".to_owned(),
                title: "Zeta".to_owned(),
                category: Some("Beta".to_owned()),
            },
        ],
    );
    let plugin_a = plugin_descriptor(
        "alpha.plugin",
        "Alpha Plugin",
        true,
        vec![
            PluginCommandContribution {
                id: "alpha.format".to_owned(),
                title: "Format".to_owned(),
                category: Some("Tools".to_owned()),
            },
            PluginCommandContribution {
                id: "alpha.aaa".to_owned(),
                title: "Alpha".to_owned(),
                category: Some("Alpha".to_owned()),
            },
        ],
    );

    let plugin_commands = PluginCommandRegistry::from_plugins(&[plugin_b, plugin_a]);
    let items = command_palette_items(&root, &[], &[], &[], &plugin_commands, &[]);
    let plugin_items = items
        .iter()
        .filter_map(|(label, command, _)| match command {
            Command::RunPluginCommand {
                plugin_id,
                command_id,
            } => Some((label.as_str(), plugin_id.as_str(), command_id.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        plugin_items,
        vec![
            ("Alpha: Alpha", "alpha.plugin", "alpha.aaa"),
            ("Beta: Zeta", "beta.plugin", "beta.zzz"),
            ("Tools: Format", "alpha.plugin", "alpha.format"),
            ("Tools: Format", "beta.plugin", "beta.format"),
        ]
    );
}

fn plugin_descriptor(
    id: &str,
    name: &str,
    commands: bool,
    command_contributions: Vec<PluginCommandContribution>,
) -> PluginDescriptor {
    PluginDescriptor {
        root: PathBuf::from("workspace/.kuroya/plugins").join(id),
        manifest: PluginManifest {
            api_version: "1".to_owned(),
            id: id.to_owned(),
            name: name.to_owned(),
            version: "0.1.0".to_owned(),
            entry: None,
            activation_events: Vec::new(),
            capabilities: PluginCapabilities {
                commands,
                ..PluginCapabilities::default()
            },
            contributes: PluginContributions {
                commands: command_contributions,
                ..PluginContributions::default()
            },
        },
    }
}

#[test]
fn command_palette_can_match_shortcuts() {
    let matcher = SkimMatcherV2::default();
    assert!(command_palette_match_score(&matcher, "Quick Open", "Ctrl+P", "ctrlp").is_some());
    assert!(command_palette_match_score(&matcher, "Quick Open", "Ctrl+P", "ctrl p").is_some());
    assert!(
        command_palette_match_score(&matcher, "Keyboard Shortcuts", "Ctrl+Alt+K", "ctrl k")
            .is_some()
    );
    assert!(command_palette_match_score(&matcher, "Quick Open", "", "qopen").is_some());
    assert!(command_palette_match_score(&matcher, "Quick Open", "", "zzzzzz").is_none());
}

#[test]
fn command_palette_matches_punctuated_shortcut_queries() {
    let matcher = SkimMatcherV2::default();
    for query in ["ctrl-alt-k", "ctrl/alt/k", "ctrl+alt+k", "ctrl\\alt\\k"] {
        assert!(
            command_palette_match_score(&matcher, "Keyboard Shortcuts", "Ctrl+Alt+K", query)
                .is_some(),
            "shortcut query {query:?} should match"
        );
    }
}

#[test]
fn command_palette_can_match_common_aliases() {
    let matcher = SkimMatcherV2::default();
    let aliases = [
        ("Open Folder", "open workspace"),
        ("Save Active File", "save file"),
        ("Open Latest Local History Snapshot", "local history"),
        ("Keyboard Shortcuts", "keybindings"),
        ("Navigate Back", "go back"),
        ("Navigate Back", "previous location"),
        ("Navigate Forward", "go forward"),
        ("Navigate Forward", "next location"),
        ("Source Control", "scm"),
        ("Switch Git Branch", "checkout branch"),
        ("New Folder", "mkdir"),
        ("Open Blame", "git blame"),
        ("Accept Current Conflict", "use ours"),
        ("Accept Incoming Conflict", "accept theirs"),
        ("Accept Both Conflicts", "resolve both conflicts"),
        ("Show Completions", "autocomplete"),
        ("Signature Help", "parameter hints"),
        ("Code Actions", "quick fix"),
        ("Project Search", "find in files"),
        ("File Symbols", "outline"),
        ("Split Editor Right", "split pane"),
        ("Outdent Lines", "unindent"),
        ("Next Terminal Search Result", "terminal find next"),
        ("Previous Terminal Search Result", "terminal find previous"),
        ("Next Terminal Session", "terminal next"),
    ];

    for (label, query) in aliases {
        assert!(
            command_palette_match_score(&matcher, label, "", query).is_some(),
            "{label:?} should match {query:?}"
        );
    }
}

#[test]
fn command_palette_generated_items_inherit_narrow_base_aliases() {
    let matcher = SkimMatcherV2::default();
    let generated_aliases = [
        (
            "New File in Workspace",
            Command::CreateFileIn(PathBuf::from("workspace")),
            "create file",
        ),
        (
            "New Folder in Workspace",
            Command::CreateFolderIn(PathBuf::from("workspace")),
            "mkdir",
        ),
        (
            "Open Recent project",
            Command::OpenWorkspace(PathBuf::from("project")),
            "open folder",
        ),
        (
            "Go to Recent Location src/main.rs:42:9",
            Command::OpenFileAt {
                path: PathBuf::from("src/main.rs"),
                line: 42,
                column: 9,
            },
            "navigation history",
        ),
        (
            "Run Build Task Build All",
            Command::RunWorkspaceTaskSnapshot {
                index: 0,
                fingerprint: 7,
            },
            "compile",
        ),
        (
            "Run Test Task Test All",
            Command::RunWorkspaceTaskSnapshot {
                index: 1,
                fingerprint: 11,
            },
            "tests",
        ),
        (
            "Run Configuration App",
            Command::RunWorkspaceTaskSnapshot {
                index: 2,
                fingerprint: 13,
            },
            "start",
        ),
    ];

    for (label, command, query) in generated_aliases {
        assert!(
            command_palette_command_match_score(&matcher, label, "", &command, query).is_some(),
            "{label:?} should inherit alias query {query:?}"
        );
    }
}

#[test]
fn command_palette_generated_alias_inheritance_is_command_aware() {
    let matcher = SkimMatcherV2::default();
    assert!(
        command_palette_command_match_score(
            &matcher,
            "Run Build Task Plugin Label",
            "",
            &Command::RunPluginCommand {
                plugin_id: "plugin".to_owned(),
                command_id: "build".to_owned(),
            },
            "compile",
        )
        .is_none()
    );
    assert!(
        command_palette_command_match_score(
            &matcher,
            "Open Recent Plugin Label",
            "",
            &Command::RunPluginCommand {
                plugin_id: "plugin".to_owned(),
                command_id: "open-recent".to_owned(),
            },
            "open folder",
        )
        .is_none()
    );
    assert!(
        command_palette_command_match_score(
            &matcher,
            "New Folder in Workspace",
            "",
            &Command::RunPluginCommand {
                plugin_id: "plugin".to_owned(),
                command_id: "new-folder".to_owned(),
            },
            "mkdir",
        )
        .is_none()
    );
    assert!(
        command_palette_command_match_score(
            &matcher,
            "Run Build Taskforce",
            "",
            &Command::RunWorkspaceTaskSnapshot {
                index: 0,
                fingerprint: 17,
            },
            "compile",
        )
        .is_none()
    );
}

#[test]
fn command_palette_recent_commands_are_deduplicated_and_bounded() {
    let mut recent = VecDeque::new();
    record_recent_palette_command(&mut recent, Command::ToggleQuickOpen, 3);
    record_recent_palette_command(&mut recent, Command::ToggleTerminal, 3);
    record_recent_palette_command(&mut recent, Command::ToggleQuickOpen, 3);
    record_recent_palette_command(&mut recent, Command::ToggleDevtools, 3);
    record_recent_palette_command(&mut recent, Command::ToggleDiagnosticsPanel, 3);

    assert_eq!(
        recent.into_iter().collect::<Vec<_>>(),
        vec![
            Command::ToggleDiagnosticsPanel,
            Command::ToggleDevtools,
            Command::ToggleQuickOpen
        ]
    );
}

#[test]
fn command_palette_recent_commands_normalize_persisted_order() {
    let recent = normalize_recent_palette_commands(
        vec![
            Command::ToggleQuickOpen,
            Command::ToggleTerminal,
            Command::ToggleQuickOpen,
            Command::ToggleDevtools,
            Command::ToggleDiagnosticsPanel,
        ],
        3,
    );

    assert_eq!(
        recent.into_iter().collect::<Vec<_>>(),
        vec![
            Command::ToggleQuickOpen,
            Command::ToggleTerminal,
            Command::ToggleDevtools
        ]
    );
}

#[test]
fn command_palette_rank_score_boosts_recent_commands() {
    let mut recent = VecDeque::new();
    record_recent_palette_command(&mut recent, Command::ToggleQuickOpen, 10);
    let memory = VecDeque::new();

    assert!(
        command_palette_rank_score(100, &recent, &memory, "", &Command::ToggleQuickOpen)
            > command_palette_rank_score(100, &recent, &memory, "", &Command::ToggleTerminal)
    );
}

#[test]
fn command_palette_rank_score_does_not_penalize_older_recent_commands() {
    let mut recent = VecDeque::new();
    for command in command_catalog() {
        record_recent_palette_command(&mut recent, command, MAX_COMMAND_PALETTE_RECENT_COMMANDS);
    }
    let memory = VecDeque::new();

    let oldest = recent.back().expect("recent commands should not be empty");
    assert!(command_palette_rank_score(100, &recent, &memory, "", oldest) >= 100);
}

#[test]
fn command_palette_query_memory_is_deduplicated_bounded_and_counted() {
    let mut memory = VecDeque::new();
    record_command_palette_query_memory(&mut memory, " git ", &Command::ToggleSourceControl, 4);
    record_command_palette_query_memory(&mut memory, "Git", &Command::ToggleSourceControl, 4);
    record_command_palette_query_memory(&mut memory, "git", &Command::ToggleGitHistory, 4);
    record_command_palette_query_memory(&mut memory, "tasks", &Command::ToggleWorkspaceTasks, 4);
    record_command_palette_query_memory(&mut memory, "dev", &Command::ToggleDevtools, 4);

    assert_eq!(
        memory.into_iter().collect::<Vec<_>>(),
        vec![
            CommandPaletteQueryMemoryEntry {
                query: "dev".to_owned(),
                command: Command::ToggleDevtools,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: "tasks".to_owned(),
                command: Command::ToggleWorkspaceTasks,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleGitHistory,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 2,
            },
        ]
    );
}

#[test]
fn command_palette_query_memory_record_normalizes_existing_entries_before_counting() {
    let mut memory = VecDeque::from([
        CommandPaletteQueryMemoryEntry {
            query: " Git ".to_owned(),
            command: Command::ToggleGitHistory,
            uses: 2,
        },
        CommandPaletteQueryMemoryEntry {
            query: "git".to_owned(),
            command: Command::ToggleGitHistory,
            uses: 7,
        },
        CommandPaletteQueryMemoryEntry {
            query: "\u{202e}".to_owned(),
            command: Command::ToggleGitHistory,
            uses: 5,
        },
    ]);

    record_command_palette_query_memory(&mut memory, "GIT", &Command::ToggleGitHistory, 10);

    assert_eq!(
        memory.into_iter().collect::<Vec<_>>(),
        vec![CommandPaletteQueryMemoryEntry {
            query: "git".to_owned(),
            command: Command::ToggleGitHistory,
            uses: 8,
        }]
    );
}

#[test]
fn command_palette_query_memory_collapses_whitespace() {
    let mut memory = VecDeque::new();
    record_command_palette_query_memory(
        &mut memory,
        " terminal   next ",
        &Command::NextTerminalSession,
        10,
    );
    record_command_palette_query_memory(
        &mut memory,
        "terminal\tnext",
        &Command::NextTerminalSession,
        10,
    );

    assert_eq!(
        memory.into_iter().collect::<Vec<_>>(),
        vec![CommandPaletteQueryMemoryEntry {
            query: "terminal next".to_owned(),
            command: Command::NextTerminalSession,
            uses: 2,
        }]
    );
}

#[test]
fn command_palette_query_memory_sanitizes_control_and_bidi_controls() {
    let mut memory = VecDeque::new();
    record_command_palette_query_memory(
        &mut memory,
        " Git\u{0000}History\u{202e}\u{2066} ",
        &Command::ToggleGitHistory,
        10,
    );
    record_command_palette_query_memory(&mut memory, "git history", &Command::ToggleGitHistory, 10);

    assert_eq!(
        memory.into_iter().collect::<Vec<_>>(),
        vec![CommandPaletteQueryMemoryEntry {
            query: "git history".to_owned(),
            command: Command::ToggleGitHistory,
            uses: 2,
        }]
    );
}

#[test]
fn command_palette_query_input_sanitizes_control_and_bidi_controls() {
    assert_eq!(
        sanitize_command_palette_query_input("\u{202e}Git\u{200f}Hi\u{0000}story"),
        "GitHistory"
    );

    let matcher = SkimMatcherV2::default();
    let query = sanitize_command_palette_query_input("git\u{202e} history");
    assert!(command_palette_match_score(&matcher, "Git History", "", &query).is_some());
}

#[test]
fn command_palette_query_input_in_place_skips_clean_queries() {
    let mut query = String::from("git history");
    let ptr = query.as_ptr();

    assert!(!sanitize_command_palette_query_input_in_place(&mut query));

    assert_eq!(query, "git history");
    assert_eq!(query.as_ptr(), ptr);
}

#[test]
fn command_palette_query_input_in_place_sanitizes_dirty_queries() {
    let mut query = String::from("  Git\tHistory\u{202e}\u{0000} ");

    assert!(sanitize_command_palette_query_input_in_place(&mut query));

    assert_eq!(query, "Git History");
}

#[test]
fn command_palette_query_input_is_bounded_before_matching() {
    let long_query = "a".repeat(MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS + 64);
    let query = sanitize_command_palette_query_input(&long_query);
    assert_eq!(
        query.chars().count(),
        MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS
    );

    let matcher = SkimMatcherV2::default();
    let label = "a".repeat(MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS);
    assert!(command_palette_match_score(&matcher, &label, "", &query).is_some());
}

#[test]
fn command_palette_query_memory_normalizes_persisted_entries() {
    let memory = normalize_command_palette_query_memory(
        vec![
            CommandPaletteQueryMemoryEntry {
                query: " Git ".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 0,
            },
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 9,
            },
            CommandPaletteQueryMemoryEntry {
                query: "  ".to_owned(),
                command: Command::ToggleGitHistory,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: "Find\tin   Files".to_owned(),
                command: Command::ToggleProjectSearch,
                uses: 2,
            },
            CommandPaletteQueryMemoryEntry {
                query: "tasks".to_owned(),
                command: Command::ToggleWorkspaceTasks,
                uses: 2,
            },
        ],
        3,
    );

    assert_eq!(
        memory.into_iter().collect::<Vec<_>>(),
        vec![
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 9,
            },
            CommandPaletteQueryMemoryEntry {
                query: "find in files".to_owned(),
                command: Command::ToggleProjectSearch,
                uses: 2,
            },
            CommandPaletteQueryMemoryEntry {
                query: "tasks".to_owned(),
                command: Command::ToggleWorkspaceTasks,
                uses: 2,
            },
        ]
    );
}

#[test]
fn command_palette_query_memory_normalization_keeps_late_duplicate_uses_after_cap() {
    let memory = normalize_command_palette_query_memory(
        vec![
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: "tasks".to_owned(),
                command: Command::ToggleWorkspaceTasks,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: "dev".to_owned(),
                command: Command::ToggleDevtools,
                uses: 1,
            },
            CommandPaletteQueryMemoryEntry {
                query: " Git ".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 9,
            },
        ],
        2,
    );

    assert_eq!(
        memory.into_iter().collect::<Vec<_>>(),
        vec![
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 9,
            },
            CommandPaletteQueryMemoryEntry {
                query: "tasks".to_owned(),
                command: Command::ToggleWorkspaceTasks,
                uses: 1,
            },
        ]
    );
}

#[test]
fn command_palette_query_memory_normalized_duplicates_keep_stronger_rank_boost() {
    let recent = VecDeque::new();
    let strong_memory = normalize_command_palette_query_memory(
        vec![
            CommandPaletteQueryMemoryEntry {
                query: " Git ".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 0,
            },
            CommandPaletteQueryMemoryEntry {
                query: "git".to_owned(),
                command: Command::ToggleSourceControl,
                uses: 9,
            },
        ],
        8,
    );
    let weak_memory = VecDeque::from([CommandPaletteQueryMemoryEntry {
        query: "git".to_owned(),
        command: Command::ToggleSourceControl,
        uses: 1,
    }]);

    assert!(
        command_palette_rank_score(
            100,
            &recent,
            &strong_memory,
            "git",
            &Command::ToggleSourceControl,
        ) > command_palette_rank_score(
            100,
            &recent,
            &weak_memory,
            "git",
            &Command::ToggleSourceControl,
        )
    );
}

#[test]
fn command_palette_rank_score_boosts_remembered_query_choices() {
    let recent = VecDeque::new();
    let mut memory = VecDeque::new();
    record_command_palette_query_memory(&mut memory, "git", &Command::ToggleGitHistory, 10);

    assert!(
        command_palette_rank_score(100, &recent, &memory, "GIT", &Command::ToggleGitHistory)
            > command_palette_rank_score(
                100,
                &recent,
                &memory,
                "GIT",
                &Command::ToggleSourceControl
            )
    );
    assert_eq!(
        command_palette_rank_score(100, &recent, &memory, "", &Command::ToggleGitHistory),
        100
    );
}

#[test]
fn command_palette_rank_score_boosts_refined_remembered_queries() {
    let recent = VecDeque::new();
    let mut memory = VecDeque::new();
    record_command_palette_query_memory(
        &mut memory,
        "terminal search",
        &Command::ToggleTerminalSearch,
        10,
    );

    assert!(
        command_palette_rank_score(
            100,
            &recent,
            &memory,
            "terminal",
            &Command::ToggleTerminalSearch
        ) > command_palette_rank_score(100, &recent, &memory, "terminal", &Command::ToggleTerminal)
    );
}

#[test]
fn command_palette_rank_score_does_not_boost_short_query_prefixes() {
    let recent = VecDeque::new();
    let mut memory = VecDeque::new();
    record_command_palette_query_memory(&mut memory, "gi", &Command::ToggleGitHistory, 10);

    assert_eq!(
        command_palette_rank_score(100, &recent, &memory, "git", &Command::ToggleGitHistory),
        100
    );
}

#[test]
fn command_palette_rank_score_keeps_exact_memory_above_hot_prefix_memory() {
    let recent = VecDeque::new();
    let mut memory = VecDeque::new();
    for _ in 0..8 {
        record_command_palette_query_memory(
            &mut memory,
            "terminal search",
            &Command::ToggleTerminalSearch,
            10,
        );
    }
    record_command_palette_query_memory(&mut memory, "terminal", &Command::ToggleTerminal, 10);

    assert!(
        command_palette_rank_score(100, &recent, &memory, "terminal", &Command::ToggleTerminal)
            > command_palette_rank_score(
                100,
                &recent,
                &memory,
                "terminal",
                &Command::ToggleTerminalSearch
            )
    );
}

#[test]
fn command_palette_rank_score_ignores_stale_query_memory_tail_beyond_cap() {
    let recent = VecDeque::new();
    let mut memory = VecDeque::new();
    for index in 0..MAX_COMMAND_PALETTE_QUERY_MEMORY {
        memory.push_back(CommandPaletteQueryMemoryEntry {
            query: format!("unrelated {index}"),
            command: Command::ToggleTerminal,
            uses: 8,
        });
    }
    memory.push_back(CommandPaletteQueryMemoryEntry {
        query: "tasks".to_owned(),
        command: Command::ToggleWorkspaceTasks,
        uses: 8,
    });

    assert_eq!(
        command_palette_rank_score(
            100,
            &recent,
            &memory,
            "tasks",
            &Command::ToggleWorkspaceTasks
        ),
        100
    );
}

#[test]
fn command_palette_ranker_matches_per_command_rank_score() {
    let mut recent = VecDeque::new();
    record_recent_palette_command(&mut recent, Command::ToggleQuickOpen, 10);
    record_recent_palette_command(&mut recent, Command::ToggleTerminal, 10);

    let mut memory = VecDeque::new();
    record_command_palette_query_memory(
        &mut memory,
        "terminal search",
        &Command::ToggleTerminalSearch,
        10,
    );
    record_command_palette_query_memory(&mut memory, "terminal", &Command::ToggleTerminal, 10);
    record_command_palette_query_memory(&mut memory, "git", &Command::ToggleGitHistory, 10);

    let ranker = CommandPaletteRanker::new(&recent, &memory, "terminal");
    for command in [
        Command::ToggleQuickOpen,
        Command::ToggleTerminal,
        Command::ToggleTerminalSearch,
        Command::ToggleGitHistory,
    ] {
        assert_eq!(
            ranker.rank_score(100, &command),
            command_palette_rank_score(100, &recent, &memory, "terminal", &command)
        );
    }
}
