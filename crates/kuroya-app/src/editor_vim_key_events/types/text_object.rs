#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimTextObjectScope {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimTextObjectKind {
    Word,
    BigWord,
    Block { open: char, close: char },
    Quote { quote: char },
    Paragraph,
    Sentence,
}
