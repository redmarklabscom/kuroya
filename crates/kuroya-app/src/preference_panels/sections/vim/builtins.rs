mod editing;
mod motion;
mod navigation;
mod scroll;
mod search;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VimBuiltInBinding {
    pub(super) label: &'static str,
    pub(super) default: &'static str,
}

const VIM_BUILTIN_BINDING_GROUPS: &[&[VimBuiltInBinding]] = &[
    motion::VIM_MOTION_BINDINGS,
    editing::VIM_EDITING_BINDINGS,
    search::VIM_SEARCH_BINDINGS,
    navigation::VIM_NAVIGATION_BINDINGS,
    scroll::VIM_SCROLL_BINDINGS,
];

pub(super) fn vim_builtin_bindings() -> impl Iterator<Item = VimBuiltInBinding> {
    VIM_BUILTIN_BINDING_GROUPS
        .iter()
        .flat_map(|bindings| bindings.iter().copied())
}
