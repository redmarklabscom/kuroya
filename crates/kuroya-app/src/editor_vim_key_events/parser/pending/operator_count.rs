use eframe::egui::{Key, Modifiers};

use super::super::super::{EditorVimCaseConversion, EditorVimPendingKey};
use super::super::{vim_count_digit, vim_push_count_digit};

pub(in crate::editor_vim_key_events) fn vim_pending_key_next_operator_count(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    match pending {
        Some(EditorVimPendingKey::Go(operator_count))
            if key == Key::Backtick
                && modifiers.shift
                && !modifiers.command
                && !modifiers.alt
                && !modifiers.ctrl =>
        {
            Some(EditorVimPendingKey::ToggleCaseOperator(
                operator_count.unwrap_or(1),
            ))
        }
        Some(EditorVimPendingKey::Go(operator_count))
            if key == Key::U && !modifiers.command && !modifiers.alt && !modifiers.ctrl =>
        {
            Some(EditorVimPendingKey::ConvertCaseOperator {
                operator_count: operator_count.unwrap_or(1),
                conversion: if modifiers.shift {
                    EditorVimCaseConversion::Upper
                } else {
                    EditorVimCaseConversion::Lower
                },
            })
        }
        Some(EditorVimPendingKey::ChangeLine(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::ChangeMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::ChangeMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            }
        }),
        Some(EditorVimPendingKey::DeleteLine(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::DeleteMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::DeleteMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            }
        }),
        Some(EditorVimPendingKey::YankLine(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::YankMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::YankMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            }
        }),
        Some(EditorVimPendingKey::ToggleCaseOperator(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::ToggleCaseMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::ConvertCaseMotionCount {
                operator_count,
                motion_count,
                conversion,
            }
        }),
        Some(EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ChangeMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ChangeMotionCountIntoRegister {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                register,
            }
        }),
        Some(EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::DeleteMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::DeleteMotionCountIntoRegister {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                register,
            }
        }),
        Some(EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::YankMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::YankMotionCountIntoRegister {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                register,
            }
        }),
        Some(EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ToggleCaseMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ConvertCaseMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                conversion,
            }
        }),
        _ => None,
    }
}
