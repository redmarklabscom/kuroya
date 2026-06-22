use super::super::builtins::vim_builtin_bindings;
use crate::editor_vim_key_events::{
    vim_key_sequence_is_normal_mode_supported, vim_key_sequence_is_single_supported,
};
use std::collections::BTreeSet;

#[test]
fn builtin_vim_binding_rows_are_unique_and_supported() {
    let mut seen = BTreeSet::new();

    for binding in vim_builtin_bindings() {
        assert!(
            seen.insert(binding.default),
            "duplicate Vim binding row for {}",
            binding.default
        );
        assert!(
            vim_key_sequence_is_single_supported(binding.default),
            "unsupported Vim binding row for {}",
            binding.default
        );
        assert!(
            vim_key_sequence_is_normal_mode_supported(binding.default),
            "unhandled Vim binding row for {}",
            binding.default
        );
    }
}

#[test]
fn builtin_vim_binding_rows_cover_direct_normal_mode_bindings() {
    let defaults = vim_builtin_bindings()
        .map(|binding| binding.default)
        .collect::<BTreeSet<_>>();

    for expected in [
        "h",
        "j",
        "k",
        "l",
        "w",
        "b",
        "e",
        "W",
        "B",
        "E",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        "0",
        "<Home>",
        "^",
        "$",
        "<End>",
        "+",
        "-",
        "_",
        "|",
        "%",
        "}",
        "{",
        "i",
        "I",
        "a",
        "A",
        "o",
        "O",
        "v",
        "d",
        "c",
        "y",
        "Y",
        "x",
        "X",
        "s",
        "S",
        "D",
        "C",
        "r",
        ".",
        "p",
        "P",
        "~",
        "J",
        ">",
        "<",
        "f",
        "F",
        "t",
        "T",
        ";",
        ",",
        "/",
        "?",
        "n",
        "N",
        "*",
        "#",
        "g",
        "G",
        "m",
        "'",
        "`",
        "\"",
        ":",
        "<Esc>",
        "u",
        "<C-r>",
        "<C-d>",
        "<C-e>",
        "<C-f>",
        "<C-n>",
        "<C-b>",
        "<C-p>",
        "<C-u>",
        "<C-y>",
        "<Space>",
        "<Backspace>",
        "<Enter>",
    ] {
        assert!(
            defaults.contains(expected),
            "missing built-in Vim binding row for {expected}"
        );
    }
}
