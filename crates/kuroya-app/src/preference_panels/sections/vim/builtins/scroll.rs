use super::VimBuiltInBinding;

pub(super) const VIM_SCROLL_BINDINGS: &[VimBuiltInBinding] = &[
    VimBuiltInBinding {
        label: "Scroll half page down",
        default: "<C-d>",
    },
    VimBuiltInBinding {
        label: "Scroll line down",
        default: "<C-e>",
    },
    VimBuiltInBinding {
        label: "Scroll page down",
        default: "<C-f>",
    },
    VimBuiltInBinding {
        label: "Scroll line down Ctrl-N",
        default: "<C-n>",
    },
    VimBuiltInBinding {
        label: "Scroll page up",
        default: "<C-b>",
    },
    VimBuiltInBinding {
        label: "Scroll line up Ctrl-P",
        default: "<C-p>",
    },
    VimBuiltInBinding {
        label: "Scroll half page up",
        default: "<C-u>",
    },
    VimBuiltInBinding {
        label: "Scroll line up",
        default: "<C-y>",
    },
];
