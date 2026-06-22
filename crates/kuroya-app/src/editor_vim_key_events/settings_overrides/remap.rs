use eframe::egui::{Key, Modifiers};
use kuroya_core::{Command, EditorVimKeyOverride, EditorVimSettings, TextBuffer};

use super::super::key_tokens::{vim_key_sequence_events, vim_key_token_for_event};
use super::super::{
    EditorVimCharFind, EditorVimLastChange, EditorVimMode, EditorVimPendingKey, EditorVimRegister,
    VimKeyResult, handle_vim_editor_key_event_without_overrides, vim_escape_key,
    vim_printable_key_char,
};

pub(in crate::editor_vim_key_events) enum VimSettingsPreflightAction {
    Handled,
    Command(Command),
    Remap(Vec<(Key, Modifiers)>),
}

enum VimKeyOverrideSequenceMatch {
    Complete(usize),
    Pending(usize),
}

pub(in crate::editor_vim_key_events) fn handle_vim_insert_escape_override(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    vim_settings: &EditorVimSettings,
    indent_unit: &str,
) -> Option<VimKeyResult> {
    if *mode != EditorVimMode::Insert {
        return None;
    }

    let token = vim_key_token_for_event(key, modifiers)?;
    let suppress_text = vim_printable_key_char(key, modifiers);

    if !vim_escape_key(key, modifiers) {
        if !vim_settings
            .disabled_bindings
            .iter()
            .any(|binding| vim_single_key_sequence_matches(binding, "<Esc>"))
        {
            return None;
        }
        vim_settings.key_overrides.iter().find(|binding| {
            binding.command.is_none()
                && vim_single_key_sequence_matches(&binding.before, &token)
                && vim_single_key_sequence_matches(&binding.after, "<Esc>")
        })?;
        *mode = EditorVimMode::Normal;
        *pending = None;
        return Some(VimKeyResult::handled(suppress_text));
    }

    if vim_settings
        .disabled_bindings
        .iter()
        .any(|binding| vim_single_key_sequence_matches(binding, &token))
    {
        return Some(VimKeyResult::handled(suppress_text));
    }

    let override_binding = vim_settings
        .key_overrides
        .iter()
        .find(|binding| vim_single_key_sequence_matches(&binding.before, &token))?;
    if let Some(command) = override_binding.command.clone() {
        return Some(VimKeyResult::command(command, suppress_text));
    }

    let keys = vim_key_sequence_events(&override_binding.after)?;
    if keys.is_empty() {
        return Some(VimKeyResult::handled(suppress_text));
    }

    *mode = EditorVimMode::Normal;
    *pending = None;
    let mut handled = false;
    let mut changed = false;
    for (next_key, next_modifiers) in keys {
        let result = handle_vim_editor_key_event_without_overrides(
            buffer,
            next_key,
            next_modifiers,
            mode,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            indent_unit,
        );
        handled |= result.handled;
        changed |= result.changed;
    }

    Some(VimKeyResult {
        handled,
        changed,
        suppress_text,
        command: None,
    })
}

pub(in crate::editor_vim_key_events) fn handle_vim_key_override(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    vim_settings: &EditorVimSettings,
    indent_unit: &str,
) -> Option<VimKeyResult> {
    if *mode != EditorVimMode::Normal {
        return None;
    }
    if matches!(
        *pending,
        Some(EditorVimPendingKey::CommandInput | EditorVimPendingKey::SearchInput { .. })
    ) {
        return None;
    }

    if matches!(
        *pending,
        Some(EditorVimPendingKey::CustomKeySequence { .. })
    ) {
        return Some(handle_pending_vim_key_override_sequence(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            vim_settings,
            indent_unit,
        ));
    }

    let token = vim_key_token_for_event(key, modifiers)?;
    let suppress_text = vim_printable_key_char(key, modifiers);
    if vim_settings
        .disabled_bindings
        .iter()
        .any(|binding| vim_single_key_sequence_matches(binding, &token))
    {
        return Some(VimKeyResult::handled(suppress_text));
    }

    if let Some(override_binding) = vim_settings
        .key_overrides
        .iter()
        .find(|binding| vim_single_key_sequence_matches(&binding.before, &token))
    {
        let allow_command_override = pending.is_none();
        return apply_vim_key_override_binding(
            buffer,
            override_binding,
            mode,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            indent_unit,
            suppress_text,
            allow_command_override,
        );
    }

    handle_vim_key_override_prefix(key, modifiers, pending, vim_settings)
}

pub(in crate::editor_vim_key_events) fn handle_vim_key_override_prefix(
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    vim_settings: &EditorVimSettings,
) -> Option<VimKeyResult> {
    if pending.is_some() {
        return None;
    }
    let suppress_text = vim_printable_key_char(key, modifiers);
    let prefix = [(key, modifiers)];
    let VimKeyOverrideSequenceMatch::Pending(binding_index) =
        vim_key_override_sequence_match(vim_settings, &prefix)?
    else {
        return None;
    };
    *pending = Some(EditorVimPendingKey::CustomKeySequence {
        binding_index,
        matched: 1,
    });
    Some(VimKeyResult::handled(suppress_text))
}

fn handle_pending_vim_key_override_sequence(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    vim_settings: &EditorVimSettings,
    indent_unit: &str,
) -> VimKeyResult {
    let suppress_text = vim_printable_key_char(key, modifiers);
    if vim_escape_key(key, modifiers) {
        *pending = None;
        return VimKeyResult::handled(None);
    }

    let Some(EditorVimPendingKey::CustomKeySequence {
        binding_index,
        matched,
    }) = *pending
    else {
        return VimKeyResult::ignored();
    };
    let Some(override_binding) = vim_settings.key_overrides.get(binding_index) else {
        *pending = None;
        return VimKeyResult::handled(suppress_text);
    };
    let Some(mut prefix) = vim_pending_key_override_prefix_events(override_binding, matched) else {
        *pending = None;
        return VimKeyResult::handled(suppress_text);
    };
    prefix.push((key, modifiers));

    match vim_key_override_sequence_match(vim_settings, &prefix) {
        Some(VimKeyOverrideSequenceMatch::Pending(binding_index)) => {
            *pending = Some(EditorVimPendingKey::CustomKeySequence {
                binding_index,
                matched: prefix.len(),
            });
            VimKeyResult::handled(suppress_text)
        }
        Some(VimKeyOverrideSequenceMatch::Complete(binding_index)) => {
            let Some(override_binding) = vim_settings.key_overrides.get(binding_index) else {
                *pending = None;
                return VimKeyResult::handled(suppress_text);
            };
            *pending = None;
            apply_vim_key_override_binding(
                buffer,
                override_binding,
                mode,
                pending,
                last_char_find,
                unnamed_register,
                last_change,
                indent_unit,
                suppress_text,
                true,
            )
            .unwrap_or_else(|| VimKeyResult::handled(suppress_text))
        }
        None => {
            *pending = None;
            VimKeyResult::handled(suppress_text)
        }
    }
}

fn vim_pending_key_override_prefix_events(
    binding: &EditorVimKeyOverride,
    matched: usize,
) -> Option<Vec<(Key, Modifiers)>> {
    let keys = vim_key_sequence_events(&binding.before)?;
    if matched == 0 || matched > keys.len() {
        return None;
    }
    Some(keys.into_iter().take(matched).collect())
}

fn vim_key_override_sequence_match(
    vim_settings: &EditorVimSettings,
    prefix: &[(Key, Modifiers)],
) -> Option<VimKeyOverrideSequenceMatch> {
    if prefix.is_empty() {
        return None;
    }

    let mut pending_match = None;
    for (index, binding) in vim_settings.key_overrides.iter().enumerate() {
        let Some(before_keys) = vim_key_sequence_events(&binding.before) else {
            continue;
        };
        if before_keys.len() < prefix.len()
            || !prefix.iter().enumerate().all(|(prefix_index, event)| {
                vim_key_event_matches(before_keys[prefix_index], event.0, event.1)
            })
        {
            continue;
        }
        if before_keys.len() == prefix.len() {
            return Some(VimKeyOverrideSequenceMatch::Complete(index));
        }
        if pending_match.is_none() {
            pending_match = Some(VimKeyOverrideSequenceMatch::Pending(index));
        }
    }

    pending_match
}

fn vim_key_override_prefix_index(
    vim_settings: &EditorVimSettings,
    key: Key,
    modifiers: Modifiers,
) -> Option<usize> {
    let prefix = [(key, modifiers)];
    match vim_key_override_sequence_match(vim_settings, &prefix)? {
        VimKeyOverrideSequenceMatch::Pending(index) => Some(index),
        VimKeyOverrideSequenceMatch::Complete(_) => None,
    }
}

fn apply_vim_key_override_binding(
    buffer: &mut TextBuffer,
    override_binding: &EditorVimKeyOverride,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    indent_unit: &str,
    suppress_text: Option<char>,
    allow_command_override: bool,
) -> Option<VimKeyResult> {
    if let Some(command) = override_binding.command.clone() {
        return allow_command_override.then_some(VimKeyResult::command(command, suppress_text));
    }

    let keys = vim_key_sequence_events(&override_binding.after)?;
    if keys.is_empty() {
        return Some(VimKeyResult::handled(suppress_text));
    }

    let mut handled = false;
    let mut changed = false;
    for (next_key, next_modifiers) in keys {
        let result = handle_vim_editor_key_event_without_overrides(
            buffer,
            next_key,
            next_modifiers,
            mode,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            indent_unit,
        );
        handled |= result.handled;
        changed |= result.changed;
    }

    Some(VimKeyResult {
        handled,
        changed,
        suppress_text,
        command: None,
    })
}

fn vim_key_event_matches(expected: (Key, Modifiers), key: Key, modifiers: Modifiers) -> bool {
    let Some(expected_token) = vim_key_token_for_event(expected.0, expected.1) else {
        return false;
    };
    vim_key_token_for_event(key, modifiers).as_deref() == Some(expected_token.as_str())
}

pub(in crate::editor_vim_key_events) fn vim_settings_preflight_action(
    key: Key,
    modifiers: Modifiers,
    vim_settings: &EditorVimSettings,
    pending: &mut Option<EditorVimPendingKey>,
    allow_command_override: bool,
) -> Option<VimSettingsPreflightAction> {
    if let Some(action) =
        vim_pending_key_override_sequence_preflight_action(key, modifiers, vim_settings, pending)
    {
        return Some(action);
    }

    let token = vim_key_token_for_event(key, modifiers)?;
    if vim_settings
        .disabled_bindings
        .iter()
        .any(|binding| vim_single_key_sequence_matches(binding, &token))
    {
        return Some(VimSettingsPreflightAction::Handled);
    }

    if let Some(override_binding) = vim_settings
        .key_overrides
        .iter()
        .find(|binding| vim_single_key_sequence_matches(&binding.before, &token))
    {
        if let Some(command) = override_binding.command.clone() {
            return allow_command_override.then_some(VimSettingsPreflightAction::Command(command));
        }
        let keys = vim_key_sequence_events(&override_binding.after)?;
        return if keys.is_empty() {
            Some(VimSettingsPreflightAction::Handled)
        } else {
            Some(VimSettingsPreflightAction::Remap(keys))
        };
    }

    if !allow_command_override {
        return None;
    }

    let binding_index = vim_key_override_prefix_index(vim_settings, key, modifiers)?;
    *pending = Some(EditorVimPendingKey::CustomKeySequence {
        binding_index,
        matched: 1,
    });
    Some(VimSettingsPreflightAction::Handled)
}

pub(in crate::editor_vim_key_events) fn vim_command_override_can_mutate(command: &Command) -> bool {
    matches!(
        command,
        Command::AcceptCurrentConflict
            | Command::AcceptIncomingConflict
            | Command::AcceptBothConflicts
            | Command::FormatDocument
            | Command::ToggleLineComment
            | Command::Undo
            | Command::Redo
            | Command::IndentLines
            | Command::OutdentLines
            | Command::DeleteLines
            | Command::JoinLines
            | Command::DuplicateLines
            | Command::MoveLineUp
            | Command::MoveLineDown
    )
}

fn vim_single_key_sequence_matches(sequence: &str, token: &str) -> bool {
    let Some(keys) = vim_key_sequence_events(sequence) else {
        return false;
    };
    let [(key, modifiers)] = keys.as_slice() else {
        return false;
    };
    vim_key_token_for_event(*key, *modifiers).as_deref() == Some(token)
}

fn vim_pending_key_override_sequence_preflight_action(
    key: Key,
    modifiers: Modifiers,
    vim_settings: &EditorVimSettings,
    pending: &mut Option<EditorVimPendingKey>,
) -> Option<VimSettingsPreflightAction> {
    let EditorVimPendingKey::CustomKeySequence {
        binding_index,
        matched,
    } = (*pending)?
    else {
        return None;
    };
    if vim_escape_key(key, modifiers) {
        *pending = None;
        return Some(VimSettingsPreflightAction::Handled);
    }

    let Some(binding) = vim_settings.key_overrides.get(binding_index) else {
        *pending = None;
        return Some(VimSettingsPreflightAction::Handled);
    };
    let Some(mut prefix) = vim_pending_key_override_prefix_events(binding, matched) else {
        *pending = None;
        return Some(VimSettingsPreflightAction::Handled);
    };
    prefix.push((key, modifiers));

    match vim_key_override_sequence_match(vim_settings, &prefix) {
        Some(VimKeyOverrideSequenceMatch::Pending(binding_index)) => {
            *pending = Some(EditorVimPendingKey::CustomKeySequence {
                binding_index,
                matched: prefix.len(),
            });
            Some(VimSettingsPreflightAction::Handled)
        }
        Some(VimKeyOverrideSequenceMatch::Complete(binding_index)) => {
            *pending = None;
            let binding = vim_settings.key_overrides.get(binding_index)?;
            if let Some(command) = binding.command.clone() {
                return Some(VimSettingsPreflightAction::Command(command));
            }
            let keys = vim_key_sequence_events(&binding.after)?;
            if keys.is_empty() {
                Some(VimSettingsPreflightAction::Handled)
            } else {
                Some(VimSettingsPreflightAction::Remap(keys))
            }
        }
        None => {
            *pending = None;
            Some(VimSettingsPreflightAction::Handled)
        }
    }
}
