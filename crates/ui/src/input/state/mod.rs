mod core;
mod events;
mod ime;
mod layout;
mod lsp;

pub(crate) use core::init;
pub use core::{
    Backspace, Copy, Cut, Delete, DeleteToBeginningOfLine, DeleteToEndOfLine, DeleteToNextWordEnd,
    DeleteToPreviousWordStart, Enter, Escape, GoToDefinition, Indent, IndentInline, InputEvent,
    InputState, LineHighlight, MoveDown, MoveEnd, MoveHome, MoveLeft, MovePageDown, MovePageUp,
    MoveRight, MoveToEnd, MoveToEndOfLine, MoveToNextWord, MoveToPreviousWord, MoveToStart,
    MoveToStartOfLine, MoveUp, Outdent, OutdentInline, Paste, Redo, Search, SelectAll, SelectDown,
    SelectLeft, SelectRight, SelectToEnd, SelectToEndOfLine, SelectToNextWordEnd,
    SelectToPreviousWordStart, SelectToStart, SelectToStartOfLine, SelectUp, ShowCharacterPalette,
    ToggleCodeActions, Undo,
};
pub(in crate::input) use core::{LastLayout, CONTEXT};

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        div, point, px, AppContext, Context, Entity, EntityInputHandler, InteractiveElement as _,
        IntoElement, KeyDownEvent, Keystroke, Modifiers, ParentElement as _, Render, ScrollDelta,
        ScrollWheelEvent, Styled as _, TestAppContext, Window, WindowHandle,
    };
    use std::{cell::Cell, rc::Rc};

    use crate::input::TextInput;

    struct FixedHeightInputView {
        input: Entity<InputState>,
        parent_scrolls: Rc<Cell<usize>>,
    }

    impl Render for FixedHeightInputView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let parent_scrolls = self.parent_scrolls.clone();
            div()
                .size_full()
                .on_scroll_wheel(move |_, _, _| {
                    parent_scrolls.set(parent_scrolls.get() + 1);
                })
                .child(TextInput::new(&self.input).h(px(60.)))
        }
    }

    fn input_with_value(cx: &mut TestAppContext) -> WindowHandle<InputState> {
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| InputState::new(window, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |input, window, cx| {
                input.set_value("abcd", window, cx);
            })
            .unwrap();

        window
    }

    #[gpui::test]
    fn set_value_updates_disabled_input_without_enabling_it(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);

        window
            .update(cx, |input, window, cx| {
                input.disabled = true;
                input.set_value("updated", window, cx);
                assert_eq!(input.value().as_ref(), "updated");
                assert!(input.disabled);
            })
            .unwrap();
    }

    fn select(
        cx: &mut TestAppContext,
        window: &WindowHandle<InputState>,
        range: std::ops::Range<usize>,
    ) {
        window
            .update(cx, |input, _, _| {
                input.selected_range = range.into();
            })
            .unwrap();
    }

    #[gpui::test]
    fn typing_with_selection_replaces_selection(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        // This is the path used by both the platform's input-handler routing
        // and the IME commit flow.
        window
            .update(cx, |input, window, cx| {
                input.replace_text_in_range(None, "e", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "aed");
            })
            .unwrap();
    }

    #[gpui::test]
    fn insert_replaces_selection(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        window
            .update(cx, |input, window, cx| {
                input.insert("e", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "aed");
            })
            .unwrap();
    }

    #[gpui::test]
    fn replace_replaces_selection(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        window
            .update(cx, |input, window, cx| {
                input.replace("e", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "aed");
            })
            .unwrap();
    }

    #[gpui::test]
    fn typing_without_selection_inserts_at_cursor(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);

        window
            .update(cx, |input, window, cx| {
                input.replace_text_in_range(None, "e", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "abcde");
            })
            .unwrap();
    }

    #[gpui::test]
    fn non_character_key_does_not_delete_selection(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        window
            .update(cx, |input, window, cx| {
                let event = KeyDownEvent {
                    keystroke: Keystroke::parse("right").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                };
                input.on_key_down(&event, window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "abcd");
            })
            .unwrap();
    }

    #[gpui::test]
    fn typing_with_reversed_selection_replaces_selection(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        // A selection dragged from right to left is stored as (start..end)
        // with `selection_reversed` set; typing must still replace the whole
        // selected range rather than inserting at the range start.
        window
            .update(cx, |input, _, _| {
                input.selection_reversed = true;
            })
            .unwrap();

        window
            .update(cx, |input, window, cx| {
                input.replace_text_in_range(None, "e", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "aed");
            })
            .unwrap();
    }

    #[gpui::test]
    fn cut_is_not_blocked_by_held_shortcut_modifier(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        window
            .update(cx, |input, window, cx| {
                window.set_modifiers(Modifiers {
                    control: true,
                    ..Default::default()
                });
                input.cut(&Cut, window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, cx| {
                assert_eq!(input.value().as_ref(), "ad");
                assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "bc");
            })
            .unwrap();
    }

    #[gpui::test]
    fn shortcut_key_text_is_not_inserted(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        window
            .update(cx, |input, window, cx| {
                window.set_modifiers(Modifiers {
                    control: true,
                    ..Default::default()
                });
                input.replace_text_in_range(None, "x", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "abcd");
            })
            .unwrap();
    }

    #[gpui::test]
    fn paste_is_not_blocked_by_held_shortcut_modifier(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);
        select(cx, &window, 1..3);

        window
            .update(cx, |input, window, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string("XY".to_owned()));
                window.set_modifiers(Modifiers {
                    control: true,
                    ..Default::default()
                });
                input.paste(&Paste, window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "aXYd");
            })
            .unwrap();
    }

    #[gpui::test]
    fn space_is_inserted_through_input_handler(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let window = input_with_value(cx);

        window
            .update(cx, |input, window, cx| {
                input.replace_text_in_range(None, " ", window, cx);
            })
            .unwrap();

        window
            .update(cx, |input, _, _| {
                assert_eq!(input.value().as_ref(), "abcd ");
            })
            .unwrap();
    }

    #[gpui::test]
    fn fixed_height_multiline_input_clips_and_consumes_scroll(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(cx));
        let parent_scrolls = Rc::new(Cell::new(0));
        let parent_scrolls_for_view = parent_scrolls.clone();
        let (view, cx) = cx.add_window_view(move |window, cx| {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .auto_grow(2, 6)
                    .default_value("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight")
            });
            FixedHeightInputView {
                input,
                parent_scrolls: parent_scrolls_for_view,
            }
        });
        let input = cx.update(|_, cx| view.read(cx).input.clone());

        let input_height = cx.update(|_, cx| input.read(cx).input_bounds.size.height);
        assert!(input_height <= px(60.));

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-20.))),
            ..Default::default()
        });

        assert_eq!(parent_scrolls.get(), 0);
        let scroll_offset = cx.update(|_, cx| input.read(cx).scroll_handle.offset());
        assert!(scroll_offset.y < px(0.));
    }
}
