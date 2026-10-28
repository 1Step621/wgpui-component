use super::*;
use crate::input::TextInput;
use gpui::{
    div, point, px, AppContext, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollDelta, ScrollWheelEvent, Styled as _, TestAppContext, Window,
};
use std::{cell::Cell, rc::Rc};

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
