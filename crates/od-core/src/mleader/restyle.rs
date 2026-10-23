//! A multileader changed to match its style when the style changes.

use super::entity::{MLeader, MLeaderContent, MLeaderLook};
use super::place::place_text;
use super::style::{MLeaderContentType, MLeaderStyle};
use crate::MText;

impl MLeader {
    /// The multileader as it would be made in `style`: the style's look, its
    /// text style, height, frame and attachment, the landing and the text
    /// placed again against the landing's end. The points, the side the text
    /// is on and what the text says stay. A style with no text keeps the
    /// text it has, and a multileader with none stays without.
    pub fn restyled(&self, style: &MLeaderStyle) -> MLeader {
        let mut changed = MLeader {
            style_name: style.name.clone(),
            look: MLeaderLook::of(style),
            ..self.clone()
        };
        let c = &style.content;
        let end = changed.landing_end();
        if let (MLeaderContent::MText(t), MLeaderContentType::MText) =
            (&mut changed.content, c.kind)
        {
            let on_left = self.landing_dir.x >= 0.0;
            let attachment = if on_left {
                c.left_attachment
            } else {
                c.right_attachment
            };
            let height = c.text_height * style.overall_scale;
            let (kind, at) = place_text(attachment, on_left, end, changed.look.landing_gap, height);
            t.text = MText {
                insertion: at,
                attachment: kind,
                nominal_height: height,
                style: c.text_style,
                ..t.text.clone()
            };
            t.frame = c.frame;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mleader::LeaderAttachment;
    use crate::Point;

    fn made() -> MLeader {
        MLeader::build(
            &MLeaderStyle::standard(),
            vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
            "Note",
        )
        .unwrap()
    }

    #[test]
    fn the_same_style_changes_nothing() {
        let m = made();
        assert_eq!(m.restyled(&MLeaderStyle::standard()), m);
    }

    #[test]
    fn a_changed_style_replaces_the_look_and_places_the_text_again() {
        let mut style = MLeaderStyle::standard();
        style.name = "Renamed".into();
        style.dogleg_length = 20.0;
        style.content.text_height = 5.0;
        style.content.frame = true;
        style.content.left_attachment = LeaderAttachment::MiddleOfText;
        let m = made().restyled(&style);
        assert_eq!(m.style_name, "Renamed");
        assert_eq!(m.look.dogleg_length, 20.0);
        let t = m.text().unwrap();
        assert!(t.frame);
        assert_eq!(t.text.nominal_height, 5.0);
        assert_eq!(t.text.contents, "Note");
        assert_eq!(t.text.insertion.x, 10.0 + 20.0 + 2.0);
        assert_eq!(t.text.insertion.y, 10.0);
        assert_eq!(m.lines, made().lines);
        assert!(m.validate().is_ok());
    }

    #[test]
    fn a_style_without_text_leaves_the_text_alone_and_a_bare_multileader_bare() {
        let mut style = MLeaderStyle::standard();
        style.content.kind = MLeaderContentType::None;
        let m = made();
        assert_eq!(m.restyled(&style).text(), m.text());
        let mut bare = made();
        bare.content = MLeaderContent::None;
        assert!(bare.restyled(&MLeaderStyle::standard()).text().is_none());
    }
}
