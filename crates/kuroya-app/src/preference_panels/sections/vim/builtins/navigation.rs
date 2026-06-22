use super::VimBuiltInBinding;

pub(super) const VIM_NAVIGATION_BINDINGS: &[VimBuiltInBinding] = &[
    VimBuiltInBinding {
        label: "Go prefix",
        default: "g",
    },
    VimBuiltInBinding {
        label: "Go file end",
        default: "G",
    },
    VimBuiltInBinding {
        label: "Set mark",
        default: "m",
    },
    VimBuiltInBinding {
        label: "Jump mark line",
        default: "'",
    },
    VimBuiltInBinding {
        label: "Jump mark exact",
        default: "`",
    },
    VimBuiltInBinding {
        label: "Register prefix",
        default: "\"",
    },
    VimBuiltInBinding {
        label: "Command input",
        default: ":",
    },
    VimBuiltInBinding {
        label: "Escape",
        default: "<Esc>",
    },
];
