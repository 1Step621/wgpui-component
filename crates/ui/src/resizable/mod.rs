use std::ops::Range;

use gpui::{
    px, Along, App, AppContext, Axis, Bounds, Context, ElementId, Entity, EventEmitter, Pixels,
};

use crate::PixelsExt;

mod panel;
mod resize_handle;
pub use panel::*;
pub(crate) use resize_handle::*;

pub(crate) const PANEL_MIN_SIZE: Pixels = px(100.);

/// Create a [`ResizablePanelGroup`] with horizontal resizing
pub fn h_resizable(id: impl Into<ElementId>) -> ResizablePanelGroup {
    ResizablePanelGroup::new(id).axis(Axis::Horizontal)
}

/// Create a [`ResizablePanelGroup`] with vertical resizing
pub fn v_resizable(id: impl Into<ElementId>) -> ResizablePanelGroup {
    ResizablePanelGroup::new(id).axis(Axis::Vertical)
}

/// Create a [`ResizablePanel`].
pub fn resizable_panel() -> ResizablePanel {
    ResizablePanel::new()
}

#[derive(Debug, Clone)]
/// State for a [`ResizablePanel`]
pub struct ResizableState {
    /// The `axis` will sync to actual axis of the ResizablePanelGroup in use.
    axis: Axis,
    panels: Vec<ResizablePanelState>,
    sizes: Vec<Pixels>,
    pub(crate) resizing_panel_ix: Option<usize>,
}

impl ResizableState {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            axis: Axis::Horizontal,
            panels: vec![],
            sizes: vec![],
            resizing_panel_ix: None,
        })
    }

    pub fn insert_panel(
        &mut self,
        size: Option<Pixels>,
        ix: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let panel_state = ResizablePanelState {
            size,
            ..Default::default()
        };

        if let Some(ix) = ix {
            self.panels.insert(ix, panel_state);
            self.sizes.insert(ix, size.unwrap_or(PANEL_MIN_SIZE));
        } else {
            self.panels.push(panel_state);
            self.sizes.push(size.unwrap_or(PANEL_MIN_SIZE));
        };
        cx.notify();
    }

    pub(crate) fn sync_panels_count(&mut self, axis: Axis, panels_count: usize) {
        self.axis = axis;
        if panels_count > self.panels.len() {
            let diff = panels_count - self.panels.len();
            self.panels
                .extend(vec![ResizablePanelState::default(); diff]);
            self.sizes.extend(vec![PANEL_MIN_SIZE; diff]);
        }
    }

    pub(crate) fn update_panel_size(
        &mut self,
        panel_ix: usize,
        bounds: Bounds<Pixels>,
        size_range: Range<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let size = bounds.size.along(self.axis);
        let panel = &mut self.panels[panel_ix];
        // Stable layout measurements must not schedule another redraw.
        if self.sizes[panel_ix] == size && panel.bounds == bounds && panel.size_range == size_range
        {
            return;
        }
        self.sizes[panel_ix] = size;
        panel.bounds = bounds;
        panel.size_range = size_range;
        cx.notify();
    }

    pub(crate) fn remove_panel(&mut self, panel_ix: usize, cx: &mut Context<Self>) {
        self.panels.remove(panel_ix);
        self.sizes.remove(panel_ix);
        if let Some(resizing_panel_ix) = self.resizing_panel_ix {
            if resizing_panel_ix > panel_ix {
                self.resizing_panel_ix = Some(resizing_panel_ix - 1);
            }
        }
        cx.notify();
    }

    pub(crate) fn replace_panel(
        &mut self,
        panel_ix: usize,
        panel: ResizablePanelState,
        cx: &mut Context<Self>,
    ) {
        let old_size = self.sizes[panel_ix];

        self.panels[panel_ix] = panel;
        self.sizes[panel_ix] = old_size;
        cx.notify();
    }

    pub(crate) fn clear(&mut self) {
        self.panels.clear();
        self.sizes.clear();
    }

    /// Get the size of the panels.
    pub fn sizes(&self) -> &Vec<Pixels> {
        &self.sizes
    }

    pub(crate) fn total_size(&self) -> Pixels {
        self.sizes.iter().map(|s| s.as_f32()).sum::<f32>().into()
    }

    pub(crate) fn done_resizing(&mut self, cx: &mut Context<Self>) {
        self.resizing_panel_ix = None;
        cx.emit(ResizablePanelEvent::Resized);
    }

    fn panel_size_range(&self, ix: usize) -> Range<Pixels> {
        let Some(panel) = self.panels.get(ix) else {
            return PANEL_MIN_SIZE..Pixels::MAX;
        };

        panel.size_range.clone()
    }

    fn sync_real_panel_sizes(&mut self) {
        for (i, panel) in self.panels.iter().enumerate() {
            self.sizes[i] = panel.bounds.size.along(self.axis);
        }
    }

    /// The `ix` is the index of the panel to resize,
    /// and the `size` is the new size for the panel.
    fn resize_panel(&mut self, ix: usize, size: Pixels, cx: &mut Context<Self>) {
        self.sync_real_panel_sizes();
        let old_sizes = self.sizes.clone();

        if ix + 1 >= old_sizes.len() {
            return;
        }
        let delta = size.floor() - old_sizes[ix];
        if delta == px(0.) {
            return;
        }

        let expanding = delta > px(0.);
        let available = |i: usize, grow: bool| {
            let range = self.panel_size_range(i);
            if grow {
                (range.end - old_sizes[i]).max(px(0.))
            } else {
                (old_sizes[i] - range.start).max(px(0.))
            }
        };
        let left_available = px((0..=ix).map(|i| available(i, expanding).as_f32()).sum());
        let right_available = px((ix + 1..old_sizes.len())
            .map(|i| available(i, !expanding).as_f32())
            .sum());
        // Transfer only the space both sides can accommodate. Keep the total
        // size unchanged, even when a panel reaches its minimum or maximum.
        let change = px(delta.as_f32().abs())
            .min(left_available)
            .min(right_available);
        if change == px(0.) {
            return;
        }
        let mut new_sizes = old_sizes.clone();
        let mut remaining = change;
        for i in (0..=ix).rev() {
            let amount = remaining.min(available(i, expanding));
            new_sizes[i] += if expanding { amount } else { -amount };
            remaining -= amount;
        }
        remaining = change;
        for i in ix + 1..old_sizes.len() {
            let amount = remaining.min(available(i, !expanding));
            new_sizes[i] += if expanding { -amount } else { amount };
            remaining -= amount;
        }

        for (panel, size) in self.panels.iter_mut().zip(&new_sizes) {
            panel.size = Some(*size);
        }

        self.sizes = new_sizes;
        cx.notify();
    }
}

impl EventEmitter<ResizablePanelEvent> for ResizableState {}

#[derive(Debug, Clone, Default)]
pub(crate) struct ResizablePanelState {
    /// Preferred size; layout measurements are stored in `bounds`.
    pub size: Option<Pixels>,
    pub size_range: Range<Pixels>,
    bounds: Bounds<Pixels>,
}
