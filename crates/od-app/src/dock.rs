//! Dockable panel layout: which panels exist, where they sit, and how the
//! layout is saved. The panels themselves are drawn by `app.rs`.

use eframe::egui;
use egui_dock::{DockState, NodeIndex};
use od_ui::Theme;
use serde::{Deserialize, Serialize};

// Bumped when a new default panel is added, so saved layouts pick it up.
const LAYOUT_KEY: &str = "dock_layout_v4";

/// One dockable panel. The canvas is a panel too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelTab {
    Canvas,
    Properties,
    Layers,
    CommandLine,
}

impl PanelTab {
    pub fn title(self) -> &'static str {
        match self {
            PanelTab::Canvas => "Drawing",
            PanelTab::Properties => "Properties",
            PanelTab::Layers => "Layers",
            PanelTab::CommandLine => "Command Line",
        }
    }

    /// The drawing view cannot be closed or floated; everything else can.
    pub fn closeable(self) -> bool {
        self != PanelTab::Canvas
    }
}

/// A change to the layout asked for by a command, applied by the app next
/// frame (commands cannot reach the dock state directly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutRequest {
    Reset,
    /// Show the panel if hidden, hide it if shown.
    Toggle(PanelTab),
}

/// Share of the window width kept by the Properties column on the left.
const PROPERTIES_SPLIT: f32 = 0.13;

/// Share of the rest kept by the drawing, beside the right-hand column.
const CANVAS_SPLIT: f32 = 0.8;

/// Share of the right-hand column kept by the command line above Layers.
const SIDE_SPLIT: f32 = 0.3;

/// Share of a column kept by whatever sits above a command line docked
/// under it (where the user may have moved it).
const BELOW_SPLIT: f32 = 0.74;

/// Properties on the left, the drawing in the middle, and the command line
/// above Layers on the right.
pub fn default_layout() -> DockState<PanelTab> {
    let mut dock = DockState::new(vec![PanelTab::Canvas]);
    let surface = dock.main_surface_mut();
    let [canvas, _] = surface.split_left(
        NodeIndex::root(),
        PROPERTIES_SPLIT,
        vec![PanelTab::Properties],
    );
    let [_, side] = surface.split_right(canvas, CANVAS_SPLIT, vec![PanelTab::CommandLine]);
    surface.split_below(side, SIDE_SPLIT, vec![PanelTab::Layers]);
    dock
}

/// Height in points of the command line strip when only the input shows
/// (tab bar plus one input row).
const COLLAPSED_HEIGHT: f32 = 68.0;

/// Collapse the command line to its input row, or restore it. `saved`
/// remembers the expanded share between the two. Does nothing if the panel
/// is not one half of a vertical split.
pub fn resize_command_line(
    dock: &mut DockState<PanelTab>,
    collapse: bool,
    saved: &mut Option<f32>,
) {
    let Some((surface, node, _)) = dock.find_tab(&PanelTab::CommandLine) else {
        return;
    };
    let (true, Some(parent)) = (surface == egui_dock::SurfaceIndex::main(), node.parent()) else {
        return;
    };
    // The fraction is the top child's share; odd indices are top children.
    let on_top = node.0 % 2 == 1;
    if let egui_dock::Node::Vertical { rect, fraction, .. } = &mut dock.main_surface_mut()[parent] {
        if collapse {
            if rect.height() > COLLAPSED_HEIGHT {
                saved.get_or_insert(*fraction);
                let share = COLLAPSED_HEIGHT / rect.height();
                *fraction = (if on_top { share } else { 1.0 - share }).clamp(0.02, 0.98);
            }
        } else {
            let default = if on_top { SIDE_SPLIT } else { BELOW_SPLIT };
            *fraction = saved.take().unwrap_or(default);
        }
    }
}

pub fn is_open(dock: &DockState<PanelTab>, panel: PanelTab) -> bool {
    dock.find_tab(&panel).is_some()
}

pub fn apply(dock: &mut DockState<PanelTab>, request: LayoutRequest) {
    match request {
        LayoutRequest::Reset => *dock = default_layout(),
        LayoutRequest::Toggle(panel) => {
            if let Some(found) = dock.find_tab(&panel) {
                if panel.closeable() {
                    dock.remove_tab(found);
                }
            } else {
                restore(dock, panel);
            }
        }
    }
}

/// Put a hidden panel back where the default layout keeps it: Properties
/// down the left; the command line and Layers sharing the right-hand
/// column, the command line on top.
fn restore(dock: &mut DockState<PanelTab>, panel: PanelTab) {
    let partner = match panel {
        PanelTab::CommandLine => Some((PanelTab::Layers, egui_dock::Split::Above)),
        PanelTab::Layers => Some((PanelTab::CommandLine, egui_dock::Split::Below)),
        _ => None,
    };
    if let Some((tab, split)) = partner {
        if let Some((surface, node, _)) = dock.find_tab(&tab) {
            // SIDE_SPLIT is the top child's share either way.
            dock.split(
                (surface, node),
                split,
                SIDE_SPLIT,
                egui_dock::Node::leaf(panel),
            );
            return;
        }
    }
    let surface = dock.main_surface_mut();
    if panel == PanelTab::Properties {
        surface.split_left(NodeIndex::root(), PROPERTIES_SPLIT, vec![panel]);
    } else {
        surface.split_right(NodeIndex::root(), CANVAS_SPLIT, vec![panel]);
    }
}

/// A saved layout is only usable if the drawing view is in it exactly once.
fn is_valid(dock: &DockState<PanelTab>) -> bool {
    dock.iter_all_tabs()
        .filter(|(_, tab)| **tab == PanelTab::Canvas)
        .count()
        == 1
}

pub fn load(storage: Option<&dyn eframe::Storage>) -> DockState<PanelTab> {
    storage
        .and_then(|s| s.get_string(LAYOUT_KEY))
        .and_then(|text| ron::from_str::<DockState<PanelTab>>(&text).ok())
        .filter(is_valid)
        .unwrap_or_else(default_layout)
}

pub fn save(dock: &DockState<PanelTab>, storage: &mut dyn eframe::Storage) {
    if let Ok(text) = ron::to_string(dock) {
        storage.set_string(LAYOUT_KEY, text);
    }
}

/// Dock chrome in the app's flat dark style.
pub fn style(theme: &Theme, egui_style: &egui::Style) -> egui_dock::Style {
    let c = &theme.colors;
    let mut style = egui_dock::Style::from_egui(egui_style);
    style.main_surface_border_stroke = egui::Stroke::NONE;
    style.separator.color_idle = c.divider;
    style.separator.color_hovered = c.accent;
    style.separator.color_dragged = c.accent;
    style.separator.width = 1.0;
    style.tab_bar.bg_fill = c.window_bg;
    style.tab_bar.hline_color = c.divider;
    style.tab_bar.height = 24.0;
    // Compact drop targets (the default is 100 px per button).
    style.overlay.max_button_size = 34.0;
    style.overlay.button_spacing = 4.0;
    style.overlay.selection_color = c.accent.gamma_multiply(0.45);
    style.overlay.button_color = c.text_weak;
    style.overlay.button_border_stroke = egui::Stroke::new(1.0_f32, c.text_weak);
    let tab = &mut style.tab;
    tab.tab_body.bg_fill = c.sidebar_bg;
    tab.tab_body.stroke = egui::Stroke::NONE;
    tab.tab_body.inner_margin = egui::Margin::ZERO;
    for state in [
        &mut tab.active,
        &mut tab.inactive,
        &mut tab.focused,
        &mut tab.hovered,
        &mut tab.active_with_kb_focus,
        &mut tab.inactive_with_kb_focus,
        &mut tab.focused_with_kb_focus,
    ] {
        state.outline_color = egui::Color32::TRANSPARENT;
        state.corner_radius = egui::CornerRadius::ZERO;
    }
    tab.active.bg_fill = c.sidebar_bg;
    tab.active.text_color = c.text;
    tab.focused.bg_fill = c.sidebar_bg;
    tab.focused.text_color = c.text;
    tab.inactive.bg_fill = c.window_bg;
    tab.inactive.text_color = c.text_weak;
    tab.hovered.bg_fill = c.hover;
    tab.hovered.text_color = c.text;
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_has_all_panels() {
        let dock = default_layout();
        assert!(is_open(&dock, PanelTab::CommandLine));
        assert!(is_open(&dock, PanelTab::Canvas));
        assert!(is_open(&dock, PanelTab::Properties));
        assert!(is_valid(&dock));
    }

    fn node_of(dock: &DockState<PanelTab>, panel: PanelTab) -> NodeIndex {
        dock.find_tab(&panel).unwrap().1
    }

    /// Properties left of everything, the command line above Layers in a
    /// column right of the drawing.
    fn assert_default_arrangement(dock: &DockState<PanelTab>) {
        let properties = node_of(dock, PanelTab::Properties);
        assert_eq!(properties, NodeIndex::root().left());
        let rest = NodeIndex::root().right();
        assert_eq!(node_of(dock, PanelTab::Canvas), rest.left());
        let side = rest.right();
        assert_eq!(node_of(dock, PanelTab::CommandLine), side.left());
        assert_eq!(node_of(dock, PanelTab::Layers), side.right());
        let surface = dock.main_surface();
        assert!(matches!(surface[side], egui_dock::Node::Vertical { .. }));
    }

    #[test]
    fn default_layout_puts_properties_left_and_the_command_line_above_layers() {
        assert_default_arrangement(&default_layout());
    }

    #[test]
    fn hidden_side_panels_come_back_where_they_were() {
        let mut dock = default_layout();
        for panel in [
            PanelTab::CommandLine,
            PanelTab::Layers,
            PanelTab::Properties,
        ] {
            apply(&mut dock, LayoutRequest::Toggle(panel));
            apply(&mut dock, LayoutRequest::Toggle(panel));
            assert_default_arrangement(&dock);
        }
    }

    #[test]
    fn the_command_line_collapses_from_the_top_of_its_column() {
        let mut dock = default_layout();
        let side = node_of(&dock, PanelTab::CommandLine).parent().unwrap();
        if let egui_dock::Node::Vertical { rect, .. } = &mut dock.main_surface_mut()[side] {
            *rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 680.0));
        }
        let fraction = |dock: &DockState<PanelTab>| match dock.main_surface()[side] {
            egui_dock::Node::Vertical { fraction, .. } => fraction,
            _ => panic!("not a vertical split"),
        };
        let mut saved = None;
        resize_command_line(&mut dock, true, &mut saved);
        assert!((fraction(&dock) - 0.1).abs() < 1e-6);
        resize_command_line(&mut dock, false, &mut saved);
        assert_eq!(fraction(&dock), SIDE_SPLIT);
    }

    #[test]
    fn toggle_hides_and_restores_a_panel() {
        let mut dock = default_layout();
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::Properties));
        assert!(!is_open(&dock, PanelTab::Properties));
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::Properties));
        assert!(is_open(&dock, PanelTab::Properties));
    }

    #[test]
    fn command_line_can_be_hidden_and_restored() {
        let mut dock = default_layout();
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::CommandLine));
        assert!(!is_open(&dock, PanelTab::CommandLine));
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::CommandLine));
        assert!(is_open(&dock, PanelTab::CommandLine));
        assert!(is_valid(&dock));
    }

    #[test]
    fn the_drawing_view_cannot_be_toggled_away() {
        let mut dock = default_layout();
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::Canvas));
        assert!(is_open(&dock, PanelTab::Canvas));
    }

    #[test]
    fn reset_restores_the_default() {
        let mut dock = default_layout();
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::Properties));
        apply(&mut dock, LayoutRequest::Reset);
        assert!(is_open(&dock, PanelTab::Properties));
    }

    #[test]
    fn layout_round_trips_through_text() {
        let mut dock = default_layout();
        apply(&mut dock, LayoutRequest::Toggle(PanelTab::Properties));
        let text = ron::to_string(&dock).unwrap();
        let back: DockState<PanelTab> = ron::from_str(&text).unwrap();
        assert!(is_valid(&back));
        assert!(!is_open(&back, PanelTab::Properties));
    }

    #[test]
    fn a_layout_without_the_drawing_view_is_rejected() {
        let mut dock = DockState::new(vec![PanelTab::Properties]);
        assert!(!is_valid(&dock));
        dock.push_to_first_leaf(PanelTab::Canvas);
        assert!(is_valid(&dock));
    }
}
