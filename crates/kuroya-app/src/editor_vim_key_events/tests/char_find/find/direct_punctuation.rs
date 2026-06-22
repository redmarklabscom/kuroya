use super::*;

#[test]
fn normal_mode_f_accepts_direct_punctuation_key_targets() {
    for (key, target, cursor) in [
        (Key::Colon, ':', 1),
        (Key::Plus, '+', 3),
        (Key::Questionmark, '?', 5),
        (Key::Exclamationmark, '!', 7),
        (Key::OpenCurlyBracket, '{', 9),
        (Key::CloseCurlyBracket, '}', 11),
    ] {
        let mut buffer = TextBuffer::from_text(1, None, "a:b+c?d!e{f}".to_owned());
        let mut mode = EditorVimMode::Normal;
        let mut pending = None;

        let find = handle_vim_editor_key_event(
            &mut buffer,
            Key::F,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
        );
        let found =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);

        assert!(find.handled);
        assert!(!find.changed);
        assert_eq!(find.suppress_text, Some('f'));
        assert!(found.handled);
        assert!(!found.changed);
        assert_eq!(found.suppress_text, Some(target));
        assert_eq!(buffer.cursor(), cursor);
        assert!(pending.is_none());
        assert!(!vim_events_include_mutation(
            &[
                key_event(Key::F, Modifiers::NONE),
                key_event(key, Modifiers::NONE)
            ],
            EditorVimMode::Normal,
            None,
        ));
    }
}
