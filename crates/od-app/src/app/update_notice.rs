//! The text at the right of the title bar. It is the program's name, and
//! says when the user has not yet decided about update checks or a newer
//! version exists; a click on either goes to Home, where the Updates card has
//! the details and the choices.

use eframe::egui;

use crate::updates::View;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Plain,
    Muted,
    Accent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Notice {
    pub text: String,
    /// Shown on hover, in full (the text may be cut short in a narrow window).
    pub tip: Option<String>,
    pub tone: Tone,
    /// A click goes to Home.
    pub clickable: bool,
}

/// What to say for `view`. `name` is the program's name (`OpenDraft`, or
/// `OpenDraft Dev` for a development build) and `version` how this build
/// calls its version.
pub(super) fn notice(view: &View, name: &str, version: &str) -> Notice {
    let plain = |text: String, tone| Notice {
        text,
        tip: None,
        tone,
        clickable: false,
    };
    match view {
        View::Disabled | View::Off => plain(name.to_owned(), Tone::Plain),
        View::Checking | View::Current { .. } | View::Offline { .. } => {
            plain(format!("{name} {version}"), Tone::Muted)
        }
        View::Unasked => Notice {
            text: "Allow internet access to stay up to date".to_owned(),
            tip: Some(
                "Nothing is sent until you allow it. Click to open Home, where you can turn \
                 on update checks."
                    .to_owned(),
            ),
            tone: Tone::Muted,
            clickable: true,
        },
        View::Available { version: newer } => Notice {
            text: format!("Version {newer} is available. See Home"),
            tip: Some(format!(
                "OpenDraft {newer} is available; you are running {version}. Click to open Home."
            )),
            tone: Tone::Accent,
            clickable: true,
        },
    }
}

impl super::OpenDraftApp {
    /// Collect a finished check, and start one if the user allowed it and it
    /// is due. Never waits: the check runs on its own thread.
    pub(super) fn tick_updates(&mut self, ctx: &egui::Context) {
        let allowed = self.state.settings.update_checks;
        self.updates.tick(ctx, allowed, crate::updates::now_secs());
    }
}

/// Draw the notice; true if it was clicked.
pub(super) fn update_notice(ui: &mut egui::Ui, theme: &od_ui::Theme, view: &View) -> bool {
    let n = notice(view, crate::profile::name(), &crate::build_info::display());
    let c = &theme.colors;
    let colour = match n.tone {
        Tone::Plain => c.text,
        Tone::Muted => c.text_weak,
        Tone::Accent => c.accent_text,
    };
    let mut label = egui::Label::new(egui::RichText::new(&n.text).color(colour)).truncate();
    if n.clickable {
        label = label.sense(egui::Sense::click());
    }
    let mut response = ui.add(label);
    if let Some(tip) = &n.tip {
        response = response.on_hover_text(tip);
    }
    if n.clickable {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    n.clickable && response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn it_is_the_name_unless_there_is_something_to_say() {
        for view in [View::Disabled, View::Off] {
            let n = notice(&view, "OpenDraft", "0.1.0");
            assert_eq!(n.text, "OpenDraft");
            assert!(!n.clickable && n.tip.is_none());
        }
        let n = notice(&View::Current { checked: Some(5) }, "OpenDraft", "0.1.0");
        assert_eq!(n.text, "OpenDraft 0.1.0");
        assert_eq!(n.tone, Tone::Muted);
        assert!(!n.clickable);
    }

    #[test]
    fn an_offline_check_is_not_a_warning() {
        let n = notice(&View::Offline { checked: None }, "OpenDraft", "0.1.0");
        assert_eq!(n.text, "OpenDraft 0.1.0");
        assert_eq!(n.tone, Tone::Muted);
    }

    #[test]
    fn not_yet_decided_asks_and_goes_to_home() {
        let n = notice(&View::Unasked, "OpenDraft", "0.1.0");
        assert_eq!(n.text, "Allow internet access to stay up to date");
        assert!(n.clickable);
        assert!(n.tip.is_some());
    }

    #[test]
    fn a_newer_version_is_named_in_words_not_only_colour() {
        let view = View::Available {
            version: Version::new(0, 2, 0),
        };
        let n = notice(&view, "OpenDraft", "0.1.0");
        assert_eq!(n.text, "Version 0.2.0 is available. See Home");
        assert_eq!(n.tone, Tone::Accent);
        assert!(n.clickable);
        assert!(n.tip.unwrap().contains("0.1.0"));
    }
}
