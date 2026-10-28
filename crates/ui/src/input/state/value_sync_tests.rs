use super::*;
use gpui::{AppContext, TestAppContext, WindowHandle};

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
