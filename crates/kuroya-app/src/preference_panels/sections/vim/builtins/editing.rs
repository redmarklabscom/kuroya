use super::VimBuiltInBinding;

pub(super) const VIM_EDITING_BINDINGS: &[VimBuiltInBinding] = &[
    VimBuiltInBinding {
        label: "Insert",
        default: "i",
    },
    VimBuiltInBinding {
        label: "Insert line start",
        default: "I",
    },
    VimBuiltInBinding {
        label: "Append",
        default: "a",
    },
    VimBuiltInBinding {
        label: "Append line end",
        default: "A",
    },
    VimBuiltInBinding {
        label: "Open below",
        default: "o",
    },
    VimBuiltInBinding {
        label: "Open above",
        default: "O",
    },
    VimBuiltInBinding {
        label: "Visual character",
        default: "v",
    },
    VimBuiltInBinding {
        label: "Delete operator",
        default: "d",
    },
    VimBuiltInBinding {
        label: "Change operator",
        default: "c",
    },
    VimBuiltInBinding {
        label: "Yank operator",
        default: "y",
    },
    VimBuiltInBinding {
        label: "Yank line",
        default: "Y",
    },
    VimBuiltInBinding {
        label: "Delete char",
        default: "x",
    },
    VimBuiltInBinding {
        label: "Delete previous char",
        default: "X",
    },
    VimBuiltInBinding {
        label: "Substitute char",
        default: "s",
    },
    VimBuiltInBinding {
        label: "Substitute line",
        default: "S",
    },
    VimBuiltInBinding {
        label: "Delete to line end",
        default: "D",
    },
    VimBuiltInBinding {
        label: "Change to line end",
        default: "C",
    },
    VimBuiltInBinding {
        label: "Replace char",
        default: "r",
    },
    VimBuiltInBinding {
        label: "Repeat change",
        default: ".",
    },
    VimBuiltInBinding {
        label: "Put after",
        default: "p",
    },
    VimBuiltInBinding {
        label: "Put before",
        default: "P",
    },
    VimBuiltInBinding {
        label: "Toggle case",
        default: "~",
    },
    VimBuiltInBinding {
        label: "Join lines",
        default: "J",
    },
    VimBuiltInBinding {
        label: "Indent",
        default: ">",
    },
    VimBuiltInBinding {
        label: "Outdent",
        default: "<",
    },
    VimBuiltInBinding {
        label: "Undo",
        default: "u",
    },
    VimBuiltInBinding {
        label: "Redo",
        default: "<C-r>",
    },
];
