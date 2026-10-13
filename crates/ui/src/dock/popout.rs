//! Pop-out windows: what a tab dragged out of a dock area becomes.
//!
//! A pop-out window has no operating-system decorations (it is created with
//! client-side decorations), so something has to draw its title bar, border
//! and window controls. That something is a [`PopoutDecorations`]: it is
//! handed the window's content and the [`PopoutContext`] of the pop-out, and
//! returns the element the window shows. Install one for the whole app with
//! [`set_popout_decorations`]; without one, [`DefaultPopoutDecorations`]
//! draws the standard title bar and border.
//!
//! ```ignore
//! ui::dock::set_popout_decorations(
//!     |content: AnyElement, popout: &PopoutContext, window: &mut Window, cx: &mut App| {
//!         v_flex()
//!             .size_full()
//!             .child(MyChrome::new(popout.title(window, cx)).on_close({
//!                 let popout = popout.clone();
//!                 move |window, cx| popout.close(window, cx)
//!             }))
//!             .child(content)
//!             .into_any_element()
//!     },
//!     cx,
//! );
//! ```
//!
//! Decorations are looked up every time a pop-out window renders, so they can
//! be replaced (a theme or settings change) and apply to windows that are
//! already open.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, px, size, AnyElement, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity,
    Global, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, Styled as _,
    Window, WindowBounds, WindowDecorations, WindowKind, WindowOptions,
};

use super::tab_panel::DockChannel;
use super::{DockArea, DockItem, PanelSource, PanelView, TabPanel};
use crate::{v_flex, window_border, ActiveTheme as _, TitleBar};

/// Starting size of a pop-out window.
const POPOUT_SIZE: gpui::Size<Pixels> = size(px(800.), px(600.));
/// Smallest size a pop-out window can be resized to.
const POPOUT_MIN_SIZE: gpui::Size<Pixels> = size(px(400.), px(300.));

// ---------------------------------------------------------------------------
// Decorations
// ---------------------------------------------------------------------------

/// What a decoration may know about, and do to, the pop-out it wraps.
#[derive(Clone)]
pub struct PopoutContext {
    panel: Arc<dyn PanelView>,
    source: Option<PanelSource>,
}

impl PopoutContext {
    /// The panel the window was opened for.
    pub fn panel(&self) -> &Arc<dyn PanelView> {
        &self.panel
    }

    /// The panel's title, as the dock's tab shows it.
    pub fn title(&self, window: &Window, cx: &App) -> AnyElement {
        self.panel.title(window, cx)
    }

    /// Whether closing the window puts the panel back where it came from.
    pub fn restores_on_close(&self) -> bool {
        self.source.is_some()
    }

    /// Close the window. A panel that was dragged out of a tab panel goes
    /// back to its place there, if that tab panel still exists.
    pub fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(source) = &self.source {
            restore_panel(source, &self.panel, cx);
        }
        window.remove_window();
    }
}

fn restore_panel(source: &PanelSource, panel: &Arc<dyn PanelView>, cx: &mut App) {
    let panel = panel.clone();
    let tab_index = source.tab_index;
    let tab_panel = source.tab_panel.clone();
    let _ = cx.update_window(source.parent_window, move |_root, window, cx| {
        let Some(tab_panel) = tab_panel.upgrade() else {
            tracing::debug!("pop-out source tab panel is gone; the panel is not restored");
            return;
        };
        tab_panel.update(cx, |tab_panel, cx| match tab_index {
            Some(index) => tab_panel.insert_panel_at(panel, index, window, cx),
            None => tab_panel.add_panel(panel, window, cx),
        });
    });
}

/// Draws the chrome of pop-out windows: title bar, border, window controls.
///
/// `content` is the window's dock area; the returned element is what the
/// window shows, so it must contain `content` and fill the window.
pub trait PopoutDecorations: 'static {
    fn decorate(
        &self,
        content: AnyElement,
        popout: &PopoutContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement;
}

impl<F> PopoutDecorations for F
where
    F: Fn(AnyElement, &PopoutContext, &mut Window, &mut App) -> AnyElement + 'static,
{
    fn decorate(
        &self,
        content: AnyElement,
        popout: &PopoutContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self(content, popout, window, cx)
    }
}

struct InstalledDecorations(Rc<dyn PopoutDecorations>);

impl Global for InstalledDecorations {}

/// Use `decorations` for every pop-out window, open or future.
pub fn set_popout_decorations(decorations: impl PopoutDecorations, cx: &mut App) {
    cx.set_global(InstalledDecorations(Rc::new(decorations)));
}

/// Go back to [`DefaultPopoutDecorations`].
pub fn reset_popout_decorations(cx: &mut App) {
    if cx.has_global::<InstalledDecorations>() {
        cx.remove_global::<InstalledDecorations>();
    }
}

/// The decorations pop-out windows have unless the app installs its own: the
/// standard [`TitleBar`] showing the panel's title, inside a [`window_border`].
pub struct DefaultPopoutDecorations;

impl PopoutDecorations for DefaultPopoutDecorations {
    fn decorate(
        &self,
        content: AnyElement,
        popout: &PopoutContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let background = cx.theme().background;
        let popout_for_close = popout.clone();
        window_border()
            .child(
                v_flex()
                    .size_full()
                    .bg(background)
                    .child(
                        TitleBar::new()
                            .child(popout.title(window, cx))
                            .on_close_window(move |_, window, cx| {
                                popout_for_close.close(window, cx)
                            }),
                    )
                    .child(div().flex_1().overflow_hidden().child(content)),
            )
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------

/// The root view of a pop-out window: a dock area wrapped in whatever
/// decorations are installed.
struct PopoutWindow {
    dock: Entity<DockArea>,
    context: PopoutContext,
}

impl Render for PopoutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = div().size_full().child(self.dock.clone()).into_any_element();
        match cx.try_global::<InstalledDecorations>().map(|d| d.0.clone()) {
            Some(decorations) => decorations.decorate(content, &self.context, window, cx),
            None => DefaultPopoutDecorations.decorate(content, &self.context, window, cx),
        }
    }
}

/// Open a pop-out window showing `panel`, with `origin` as the window's
/// top-left corner in logical screen coordinates.
///
/// `source` says where the panel came from, so closing the window can put it
/// back; `channel` keeps the window's tabs on the dock channel they were
/// dragged from. Returns the new window's handle.
pub(crate) fn open_popout_window(
    panel: Arc<dyn PanelView>,
    origin: gpui::Point<Pixels>,
    channel: DockChannel,
    source: Option<PanelSource>,
    cx: &mut App,
) -> Option<AnyWindowHandle> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, POPOUT_SIZE))),
        // The decorations draw the title bar; there is no system one.
        titlebar: None,
        window_min_size: Some(POPOUT_MIN_SIZE),
        kind: WindowKind::Normal,
        is_resizable: true,
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    };

    cx.open_window(options, move |window, cx| {
        let dock = cx.new(|cx| DockArea::new_with_channel("detached-dock", Some(1), channel, window, cx));
        let weak = dock.downgrade();
        let tab_panel = cx.new(|cx| {
            let mut tab_panel = TabPanel::new(None, weak.clone(), channel, window, cx);
            tab_panel.closable = true;
            tab_panel
        });
        tab_panel.update(cx, |tab_panel, cx| tab_panel.add_panel(panel.clone(), window, cx));
        dock.update(cx, |dock, cx| {
            dock.set_center(
                DockItem::Tabs {
                    view: tab_panel.clone(),
                    active_ix: 0,
                    items: vec![panel.clone()],
                },
                window,
                cx,
            );
        });

        let popout = cx.new(|_| PopoutWindow {
            dock,
            context: PopoutContext { panel, source },
        });
        cx.new(|cx| crate::Root::new(popout.into(), window, cx))
    })
    .ok()
    .map(Into::into)
}

/// Where a pop-out window's top-left corner goes so that its title bar sits
/// under the cursor that just dragged the tab out, at `cursor` in logical
/// screen coordinates.
pub(crate) fn origin_under_cursor(cursor: gpui::Point<Pixels>) -> gpui::Point<Pixels> {
    gpui::point(cursor.x - px(120.0), cursor.y - px(18.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorations_are_installed_replaced_and_reset_app_wide() {
        let test_context = gpui::TestAppContext::single();
        test_context.update(|cx| {
            assert!(!cx.has_global::<InstalledDecorations>());

            set_popout_decorations(
                |content: AnyElement, _: &PopoutContext, _: &mut Window, _: &mut App| content,
                cx,
            );
            assert!(cx.has_global::<InstalledDecorations>());

            // Installing again replaces; there is only ever one.
            set_popout_decorations(DefaultPopoutDecorations, cx);
            assert!(cx.has_global::<InstalledDecorations>());

            reset_popout_decorations(cx);
            assert!(!cx.has_global::<InstalledDecorations>());
            // Resetting twice is fine.
            reset_popout_decorations(cx);
        });
    }
}
