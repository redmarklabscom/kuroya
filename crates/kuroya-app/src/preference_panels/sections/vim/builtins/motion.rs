use super::VimBuiltInBinding;

pub(super) const VIM_MOTION_BINDINGS: &[VimBuiltInBinding] = &[
    VimBuiltInBinding {
        label: "Move left",
        default: "h",
    },
    VimBuiltInBinding {
        label: "Move down",
        default: "j",
    },
    VimBuiltInBinding {
        label: "Move up",
        default: "k",
    },
    VimBuiltInBinding {
        label: "Move right",
        default: "l",
    },
    VimBuiltInBinding {
        label: "Next word",
        default: "w",
    },
    VimBuiltInBinding {
        label: "Previous word",
        default: "b",
    },
    VimBuiltInBinding {
        label: "Word end",
        default: "e",
    },
    VimBuiltInBinding {
        label: "Next WORD",
        default: "W",
    },
    VimBuiltInBinding {
        label: "Previous WORD",
        default: "B",
    },
    VimBuiltInBinding {
        label: "WORD end",
        default: "E",
    },
    VimBuiltInBinding {
        label: "Count prefix 1",
        default: "1",
    },
    VimBuiltInBinding {
        label: "Count prefix 2",
        default: "2",
    },
    VimBuiltInBinding {
        label: "Count prefix 3",
        default: "3",
    },
    VimBuiltInBinding {
        label: "Count prefix 4",
        default: "4",
    },
    VimBuiltInBinding {
        label: "Count prefix 5",
        default: "5",
    },
    VimBuiltInBinding {
        label: "Count prefix 6",
        default: "6",
    },
    VimBuiltInBinding {
        label: "Count prefix 7",
        default: "7",
    },
    VimBuiltInBinding {
        label: "Count prefix 8",
        default: "8",
    },
    VimBuiltInBinding {
        label: "Count prefix 9",
        default: "9",
    },
    VimBuiltInBinding {
        label: "Line start",
        default: "0",
    },
    VimBuiltInBinding {
        label: "Line start Home",
        default: "<Home>",
    },
    VimBuiltInBinding {
        label: "First non-blank",
        default: "^",
    },
    VimBuiltInBinding {
        label: "Line end",
        default: "$",
    },
    VimBuiltInBinding {
        label: "Line end End",
        default: "<End>",
    },
    VimBuiltInBinding {
        label: "Next line non-blank",
        default: "+",
    },
    VimBuiltInBinding {
        label: "Previous line non-blank",
        default: "-",
    },
    VimBuiltInBinding {
        label: "Counted line non-blank",
        default: "_",
    },
    VimBuiltInBinding {
        label: "Column",
        default: "|",
    },
    VimBuiltInBinding {
        label: "Matching bracket",
        default: "%",
    },
    VimBuiltInBinding {
        label: "Next paragraph",
        default: "}",
    },
    VimBuiltInBinding {
        label: "Previous paragraph",
        default: "{",
    },
    VimBuiltInBinding {
        label: "Move space forward",
        default: "<Space>",
    },
    VimBuiltInBinding {
        label: "Move space backward",
        default: "<Backspace>",
    },
    VimBuiltInBinding {
        label: "Enter line non-blank",
        default: "<Enter>",
    },
];
