//! Layouts on the canvas: the sheet, its viewports, and moving around them.
//!
//! In a layout the canvas shows paper millimetres. `camera` always maps the
//! space being worked in to the screen: the sheet in paper space, and in a
//! viewport the model through that viewport (the sheet camera composed with
//! the viewport's scale and offset). Everything that already works on `camera`
//! (picking, snapping, grips, ghosts) therefore works unchanged inside the
//! active viewport. `sheet` is the sheet camera, kept apart for the frames
//! that use the viewport camera.

use od_core::{Bounds, LayoutId, Viewport, ViewportId};

use super::*;

/// Where editing happens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Space {
    /// The model tab.
    #[default]
    Model,
    /// A layout's paper space.
    Paper(LayoutId),
    /// Model space reached through one viewport of a layout (`MSPACE`).
    Viewport(LayoutId, ViewportId),
}

impl Space {
    pub fn layout(self) -> Option<LayoutId> {
        match self {
            Space::Model => None,
            Space::Paper(l) | Space::Viewport(l, _) => Some(l),
        }
    }

    pub fn viewport(self) -> Option<ViewportId> {
        match self {
            Space::Viewport(_, v) => Some(v),
            _ => None,
        }
    }
}

/// Click distance from a viewport border, in screen points.
const BORDER_PICK_PX: f64 = 6.0;

/// What a layout frame's input asks the app to do.
#[derive(Default)]
pub(super) struct LayoutInput {
    /// The active viewport as navigation changed it (its view, not its place).
    pub viewport_edit: Option<Viewport>,
    pub enter_viewport: Option<ViewportId>,
    pub leave_viewport: bool,
}

impl View {
    pub fn space(&self) -> Space {
        self.space
    }

    /// The viewport picked in paper space, if any.
    pub fn selected_viewport(&self) -> Option<ViewportId> {
        self.selected_viewport
    }

    pub fn pick_viewport(&mut self, id: ViewportId) {
        self.selected_viewport = Some(id);
    }

    pub fn clear_selected_viewport(&mut self) {
        self.selected_viewport = None;
    }

    /// Switch between the model and a layout's spaces. Each keeps its own camera.
    pub fn set_space(&mut self, space: Space) {
        if space == self.space {
            return;
        }
        if self.space == Space::Model {
            self.model_camera = self.camera;
        }
        if space.layout() != self.space.layout() {
            self.sheet_fit = true;
            self.selected_viewport = None;
        }
        self.space = space;
        self.camera = match space {
            Space::Model => self.model_camera,
            _ => self.sheet,
        };
        self.clip = None;
    }

    /// Point the cameras at this frame's space, before any input is read.
    pub(super) fn begin_layout_frame(&mut self, document: &Document, rect: Rect) {
        self.clip = None;
        let Some(layout) = self.space.layout().and_then(|l| document.layout(l)) else {
            return;
        };
        if self.sheet_fit {
            self.sheet.fit(Some(layout.sheet_bounds()), rect);
            self.sheet_fit = false;
        }
        self.camera = self.sheet;
        if let Some(vp) = self.space.viewport().and_then(|v| layout.viewport(v)) {
            self.use_viewport_camera(vp, rect);
        }
    }

    /// The model camera seen through `vp` on the current sheet camera.
    fn viewport_camera(&self, vp: &Viewport) -> Camera {
        Camera {
            zoom: self.sheet.zoom * vp.scale,
            center: Point::new(
                vp.view_center.x - (vp.center.x - self.sheet.center.x) / vp.scale,
                vp.view_center.y - (vp.center.y - self.sheet.center.y) / vp.scale,
            ),
        }
    }

    fn sheet_rect(&self, b: Bounds, rect: Rect) -> Rect {
        Rect::from_two_pos(
            self.sheet.world_to_screen(b.min, rect),
            self.sheet.world_to_screen(b.max, rect),
        )
    }

    fn use_viewport_camera(&mut self, vp: &Viewport, rect: Rect) {
        if vp.is_valid() {
            self.camera = self.viewport_camera(vp);
            self.clip = Some(self.sheet_rect(vp.paper_bounds(), rect));
        }
    }

    /// A screen position the active space accepts input at.
    pub(super) fn blocked(&self, p: Pos2) -> bool {
        self.input_exclusions.iter().any(|r| r.contains(p))
            || self.clip.is_some_and(|c| !c.contains(p))
    }

    /// Wheel, middle-drag and zoom commands in a layout. Over an unlocked
    /// active viewport they change its view; elsewhere (or when it is locked)
    /// they move the sheet.
    pub(super) fn navigate_layout(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        document: &Document,
        wheel_rate: f32,
    ) -> LayoutInput {
        let mut out = LayoutInput::default();
        let Some(layout) = self.space.layout().and_then(|l| document.layout(l)) else {
            return out;
        };
        let mut vp = self
            .space
            .viewport()
            .and_then(|v| layout.viewport(v))
            .cloned();
        let through = self.wheel_through;
        let pointer = ui.input(|i| i.pointer.latest_pos()).filter(|p| {
            rect.contains(*p) && (through || !self.input_exclusions.iter().any(|r| r.contains(*p)))
        });
        let mut edited = false;
        match self.request.take() {
            Some(Request::Extents) => {
                let fit = vp
                    .as_ref()
                    .filter(|v| !v.locked)
                    .zip(document.model_bounds());
                if let Some((v, b)) = fit {
                    let framed = Viewport::framing(v.id, v.center, v.width, v.height, Some(b));
                    vp = Some(Viewport {
                        view_center: framed.view_center,
                        scale: framed.scale,
                        ..v.clone()
                    });
                    edited = true;
                } else {
                    self.sheet.fit(Some(layout.sheet_bounds()), rect);
                }
            }
            Some(Request::Zoom(factor)) => {
                let at = self
                    .clip
                    .filter(|_| vp.is_some())
                    .map_or(rect.center(), |c| c.center());
                self.zoom_view(&mut vp, &mut edited, rect, at, factor);
            }
            None => {}
        }
        if !response.dragged_by(PointerButton::Middle) {
            self.pan_viewport = None;
        } else {
            let delta = response.drag_delta();
            let origin = ui.input(|i| i.pointer.press_origin());
            let to_viewport = match self.pan_viewport {
                Some(v) => v,
                None => {
                    let v = self.drives_viewport(&vp, origin);
                    self.pan_viewport = Some(v);
                    v
                }
            };
            if to_viewport {
                let z = self.sheet.zoom;
                vp = vp.map(|v| v.panned_by_paper(f64::from(delta.x) / z, -f64::from(delta.y) / z));
                edited = true;
            } else {
                self.sheet.pan_by_pixels(delta);
            }
        }
        if let (true, Some(at)) = (response.hovered(), pointer) {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = f64::from((scroll * wheel_rate).exp() * pinch);
            if (factor - 1.0).abs() > 1e-9 {
                self.zoom_view(&mut vp, &mut edited, rect, at, factor);
            }
        }

        // The view the rest of the frame (and the app) sees.
        self.camera = self.sheet;
        if let Some(v) = &vp {
            self.use_viewport_camera(v, rect);
        }
        if edited {
            out.viewport_edit = vp;
        }
        out
    }

    /// Whether a view change at `at` goes to the viewport (unlocked, pointer
    /// inside it) rather than the sheet.
    fn drives_viewport(&self, vp: &Option<Viewport>, at: Option<Pos2>) -> bool {
        vp.as_ref().is_some_and(|v| !v.locked)
            && at.zip(self.clip).is_some_and(|(p, c)| c.contains(p))
    }

    fn zoom_view(
        &mut self,
        vp: &mut Option<Viewport>,
        edited: &mut bool,
        rect: Rect,
        at: Pos2,
        factor: f64,
    ) {
        if self.drives_viewport(vp, Some(at)) {
            let paper = self.sheet.screen_to_world(at, rect);
            *vp = vp.as_ref().map(|v| v.zoomed_about_paper(paper, factor));
            *edited = true;
        } else {
            self.sheet.zoom_at(at, rect, factor);
        }
    }

    /// Double-click and click handling in a layout, when no command is asking.
    pub(super) fn layout_clicks(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        document: &Document,
        out: &mut LayoutInput,
    ) {
        let Some(layout) = self.space.layout().and_then(|l| document.layout(l)) else {
            return;
        };
        let at = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| rect.contains(*p) && !self.input_exclusions.iter().any(|r| r.contains(*p)));
        let Some(at) = at else { return };
        let paper = self.sheet.screen_to_world(at, rect);
        match self.space {
            Space::Paper(_) if response.double_clicked() => {
                out.enter_viewport = layout.viewport_at(paper);
            }
            Space::Paper(_) if response.clicked_by(PointerButton::Primary) => {
                let tolerance = BORDER_PICK_PX / self.sheet.zoom;
                let on_object = crate::hit::pick_here(document, paper, tolerance).is_some();
                self.selected_viewport = layout
                    .viewports
                    .iter()
                    .rev()
                    .find(|v| !on_object && near_border(&v.paper_bounds(), paper, tolerance))
                    .map(|v| v.id);
            }
            Space::Viewport(..) if response.double_clicked() => {
                out.leave_viewport = self.clip.is_some_and(|c| !c.contains(at));
            }
            _ => {}
        }
    }

    /// The sheet and every viewport's model content, then the active
    /// viewport's grid.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_layout(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        grid: &GridSettings,
        document: &Document,
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let Some(layout) = self.space.layout().and_then(|l| document.layout(l)) else {
            return;
        };
        let page = self.sheet_rect(layout.sheet_bounds(), rect);
        painter.rect_filled(
            page.translate(egui::vec2(4.0, 4.0)),
            0.0,
            Color32::from_black_alpha(90),
        );
        painter.rect_filled(page, 0.0, style.background);
        let active = self.space.viewport();
        let live = self.camera;
        let none = BTreeSet::new();
        for vp in layout.viewports.iter().filter(|v| v.is_valid()) {
            let is_active = active == Some(vp.id);
            let frame = match (is_active, self.clip) {
                (true, Some(clip)) => clip,
                _ => self.sheet_rect(vp.paper_bounds(), rect),
            };
            let clipped = painter.with_clip_rect(frame.intersect(rect));
            if vp.on {
                self.camera = if is_active {
                    live
                } else {
                    self.viewport_camera(vp)
                };
                if is_active {
                    if grid.visible {
                        self.paint_grid(&clipped, rect, style, grid);
                    }
                    self.paint_axes(&clipped, rect, style);
                    self.paint_entities(
                        &clipped, rect, style, document, None, &vp.frozen, selection, hover,
                    );
                } else {
                    self.paint_entities(
                        &clipped, rect, style, document, None, &vp.frozen, &none, None,
                    );
                }
                self.camera = live;
            }
            let (width, color) = if self.selected_viewport == Some(vp.id) {
                (2.0_f32, style.selected)
            } else if is_active {
                (2.5, style.window)
            } else if active.is_some() {
                // Working inside another viewport: these are sheet furniture.
                (1.0, style.entity.gamma_multiply(0.2))
            } else {
                (1.0, style.entity.gamma_multiply(0.6))
            };
            let stroke = Stroke::new(width, color);
            if vp.on {
                painter.rect_stroke(frame, 0.0, stroke, StrokeKind::Middle);
            } else {
                painter.extend(egui::Shape::dashed_line(
                    &[
                        frame.left_top(),
                        frame.right_top(),
                        frame.right_bottom(),
                        frame.left_bottom(),
                        frame.left_top(),
                    ],
                    stroke,
                    6.0,
                    4.0,
                ));
            }
        }
        // What is drawn on the sheet itself goes over the viewports.
        self.camera = self.sheet;
        let (selected, hovered) = if active.is_none() {
            (selection, hover)
        } else {
            (&none, None)
        };
        self.paint_entities(
            painter,
            rect,
            style,
            document,
            Some(layout.id),
            &[],
            selected,
            hovered,
        );
        self.camera = live;
    }
}

/// Whether `p` is within `tolerance` of the outline of `b`.
fn near_border(b: &Bounds, p: Point, tolerance: f64) -> bool {
    let outer = p.x >= b.min.x - tolerance
        && p.x <= b.max.x + tolerance
        && p.y >= b.min.y - tolerance
        && p.y <= b.max.y + tolerance;
    let inner = p.x > b.min.x + tolerance
        && p.x < b.max.x - tolerance
        && p.y > b.min.y + tolerance
        && p.y < b.max.y - tolerance;
    outer && !inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_viewport_camera_agrees_with_the_viewport_mapping() {
        let view = View {
            sheet: Camera {
                center: Point::new(120.0, 90.0),
                zoom: 3.0,
            },
            ..View::default()
        };
        let vp = Viewport {
            id: ViewportId(1),
            center: Point::new(150.0, 100.0),
            width: 200.0,
            height: 120.0,
            view_center: Point::new(5000.0, 3000.0),
            scale: 0.02,
            locked: false,
            on: true,
            frozen: Vec::new(),
        };
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), egui::vec2(900.0, 600.0));
        let camera = view.viewport_camera(&vp);
        for p in [Point::new(5000.0, 3000.0), Point::new(-200.0, 8000.0)] {
            let via_paper = view.sheet.world_to_screen(vp.model_to_paper(p), rect);
            let direct = camera.world_to_screen(p, rect);
            assert!((via_paper.x - direct.x).abs() < 1e-2 && (via_paper.y - direct.y).abs() < 1e-2);
        }
    }

    #[test]
    fn border_hits_are_on_the_outline_not_inside() {
        let b = Bounds::from_points(Point::new(0.0, 0.0), Point::new(100.0, 50.0));
        assert!(near_border(&b, Point::new(0.0, 25.0), 2.0));
        assert!(near_border(&b, Point::new(101.0, 25.0), 2.0));
        assert!(!near_border(&b, Point::new(50.0, 25.0), 2.0));
        assert!(!near_border(&b, Point::new(110.0, 25.0), 2.0));
    }

    #[test]
    fn switching_space_keeps_each_camera() {
        let mut view = View {
            camera: Camera {
                center: Point::new(7.0, 7.0),
                zoom: 9.0,
            },
            ..View::default()
        };
        view.set_space(Space::Paper(LayoutId(1)));
        view.camera.zoom = 1.0;
        view.sheet = view.camera;
        view.set_space(Space::Model);
        assert_eq!(view.camera.zoom, 9.0, "the model view comes back");
        view.set_space(Space::Paper(LayoutId(1)));
        assert_eq!(view.camera.zoom, 1.0, "and so does the sheet");
    }
}
