use super::VimBuiltInBinding;

pub(super) const VIM_SEARCH_BINDINGS: &[VimBuiltInBinding] = &[
    VimBuiltInBinding {
        label: "Find char forward",
        default: "f",
    },
    VimBuiltInBinding {
        label: "Find char backward",
        default: "F",
    },
    VimBuiltInBinding {
        label: "Till char forward",
        default: "t",
    },
    VimBuiltInBinding {
        label: "Till char backward",
        default: "T",
    },
    VimBuiltInBinding {
        label: "Repeat char find",
        default: ";",
    },
    VimBuiltInBinding {
        label: "Reverse char find",
        default: ",",
    },
    VimBuiltInBinding {
        label: "Search forward",
        default: "/",
    },
    VimBuiltInBinding {
        label: "Search backward",
        default: "?",
    },
    VimBuiltInBinding {
        label: "Next search",
        default: "n",
    },
    VimBuiltInBinding {
        label: "Previous search",
        default: "N",
    },
    VimBuiltInBinding {
        label: "Search word",
        default: "*",
    },
    VimBuiltInBinding {
        label: "Search word backward",
        default: "#",
    },
];
