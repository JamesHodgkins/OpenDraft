//! Which controls of the Plot and Page Setup dialogs can be used, worked out
//! from the draft alone so every widget agrees. A control that is off keeps
//! its value: the draft is never changed by being disabled.

use opendraft_core::plot_plan::validate_plot;
use opendraft_core::{plot_settings::flags, PlotArea, PlotScale, PlotSettings, StandardScale};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlotControlState {
    pub can_preview: bool,
    pub can_submit: bool,
    pub can_fit: bool,
    pub can_center: bool,
    pub can_edit_scale: bool,
    pub can_edit_offset: bool,
    pub can_paperspace_order: bool,
    pub can_hide_paperspace: bool,
    /// The plot style table and the options that use it. Tables are not
    /// evaluated yet, so these stay as stored.
    pub can_edit_styles: bool,
    pub can_edit_shade_dpi: bool,
    /// Why Plot is unavailable, or what a disabled control is waiting for.
    pub reason: Option<&'static str>,
}

/// Fit and centring need a plot area of their own: a layout is plotted at its
/// configured scale from the printable corner.
pub fn control_state(settings: &PlotSettings, model_space: bool) -> PlotControlState {
    let has_area = !matches!(settings.area, PlotArea::Layout);
    let fit = matches!(
        settings.scale,
        PlotScale::Standard {
            kind: StandardScale::Fit,
            ..
        }
    );
    let centred = settings.flags & flags::PLOT_CENTERED != 0;
    let valid = validate_plot(settings);
    let reason = match valid {
        Err(error) => Some(match error {
            opendraft_core::plot_plan::PlotValidationError::InvalidPaper => {
                "The paper size is not usable."
            }
            opendraft_core::plot_plan::PlotValidationError::InvalidMargins => {
                "The margins leave no room."
            }
            opendraft_core::plot_plan::PlotValidationError::InvalidScale => {
                "The scale is not a valid ratio."
            }
            opendraft_core::plot_plan::PlotValidationError::InvalidWindow => {
                "The window has no area."
            }
            _ => "The page setup has values that cannot be plotted.",
        }),
        Ok(()) if matches!(settings.area, PlotArea::Display | PlotArea::View(_)) => {
            Some("Display and named views cannot be plotted yet.")
        }
        Ok(())
            if model_space
                && !matches!(settings.area, PlotArea::Extents | PlotArea::Window { .. }) =>
        {
            Some("The Model tab plots its Extents or a Window.")
        }
        Ok(()) => None,
    };
    let ready = reason.is_none();
    PlotControlState {
        can_preview: ready,
        can_submit: ready,
        can_fit: has_area,
        can_center: has_area,
        can_edit_scale: !(fit && has_area),
        can_edit_offset: !(centred && has_area),
        can_paperspace_order: !model_space,
        // Hiding is for 3D solids; a 2D drawing has no hidden lines.
        can_hide_paperspace: false,
        can_edit_styles: false,
        // Shaded viewports do not exist in 2D, so the quality stays as stored.
        can_edit_shade_dpi: false,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::Point;

    fn base() -> PlotSettings {
        PlotSettings::for_layout(297.0, 210.0)
    }

    fn fit() -> PlotScale {
        PlotScale::Standard {
            kind: StandardScale::Fit,
            value: 1.0,
        }
    }

    #[test]
    fn a_layout_plots_at_its_scale_so_fit_and_centre_are_off() {
        let mut s = base();
        s.scale = fit();
        s.flags |= flags::PLOT_CENTERED;
        let c = control_state(&s, false);
        assert!(!c.can_fit && !c.can_center);
        assert!(
            c.can_edit_scale && c.can_edit_offset,
            "nothing is hidden by a disabled fit"
        );
        assert_eq!(
            (s.scale, s.flags & flags::PLOT_CENTERED),
            (fit(), flags::PLOT_CENTERED)
        );
    }

    #[test]
    fn fit_and_centre_turn_off_the_numbers_they_replace() {
        let mut s = base();
        s.area = PlotArea::Extents;
        assert!(control_state(&s, false).can_fit);
        s.scale = fit();
        s.flags |= flags::PLOT_CENTERED;
        let c = control_state(&s, false);
        assert!(c.can_fit && c.can_center);
        assert!(!c.can_edit_scale && !c.can_edit_offset);
        s.scale = PlotScale::Custom {
            paper_units: 1.0,
            drawing_units: 2.0,
        };
        s.flags &= !flags::PLOT_CENTERED;
        let c = control_state(&s, false);
        assert!(c.can_edit_scale && c.can_edit_offset);
    }

    #[test]
    fn the_model_tab_has_no_paperspace_order() {
        assert!(control_state(&base(), false).can_paperspace_order);
        assert!(!control_state(&base(), true).can_paperspace_order);
    }

    #[test]
    fn what_the_plot_does_not_apply_is_not_editable() {
        let c = control_state(&base(), false);
        assert!(!c.can_edit_styles, "plot style tables are not evaluated");
        assert!(!c.can_hide_paperspace, "2D has no hidden lines");
        assert!(!c.can_edit_shade_dpi);
    }

    #[test]
    fn a_setup_that_cannot_plot_says_why_and_blocks_preview_and_plot() {
        let mut s = base();
        s.area = PlotArea::Window {
            first: Point::new(0.0, 0.0),
            second: Point::new(0.0, 5.0),
        };
        let c = control_state(&s, false);
        assert!(!c.can_preview && !c.can_submit);
        assert_eq!(c.reason, Some("The window has no area."));
        s.area = PlotArea::Display;
        let c = control_state(&s, false);
        assert!(!c.can_submit && c.reason.is_some());
        assert!(control_state(&base(), false).can_submit);
    }

    #[test]
    fn a_model_plot_needs_extents_or_a_window() {
        let mut s = base();
        s.area = PlotArea::Extents;
        assert!(control_state(&s, true).can_submit);
        s.area = PlotArea::Layout;
        let c = control_state(&s, true);
        assert!(!c.can_submit);
        assert_eq!(
            c.reason,
            Some("The Model tab plots its Extents or a Window.")
        );
        s.area = PlotArea::Window {
            first: Point::new(0.0, 0.0),
            second: Point::new(5.0, 5.0),
        };
        assert!(control_state(&s, true).can_submit);
    }

    #[test]
    fn shaded_quality_is_never_editable_in_2d() {
        assert!(!control_state(&base(), false).can_edit_shade_dpi);
    }
}
