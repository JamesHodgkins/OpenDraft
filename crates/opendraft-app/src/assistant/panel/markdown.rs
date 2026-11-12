//! The little of Markdown a model's answer uses: `**bold**` and `- ` bullets.
//! Anything else is shown as typed.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId};

/// `text` as a laid-out paragraph in `normal`, with bold runs in `strong`.
pub fn job(text: &str, font: &FontId, normal: Color32, strong: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let body = match line.trim_start().strip_prefix("- ") {
            Some(rest) => {
                append(&mut job, "  \u{2022} ", font, normal);
                rest
            }
            None => line,
        };
        for (n, part) in body.split("**").enumerate() {
            let colour = if n % 2 == 1 { strong } else { normal };
            append(&mut job, part, font, colour);
        }
        if lines.peek().is_some() {
            append(&mut job, "\n", font, normal);
        }
    }
    job
}

fn append(job: &mut LayoutJob, text: &str, font: &FontId, colour: Color32) {
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: colour,
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> String {
        job(
            text,
            &FontId::default(),
            Color32::WHITE,
            Color32::RED,
            100.0,
        )
        .text
    }

    #[test]
    fn bold_markers_are_dropped_and_bullets_become_bullets() {
        assert_eq!(plain("a **b** c"), "a b c");
        assert_eq!(plain("- one\n- two"), "  \u{2022} one\n  \u{2022} two");
    }

    #[test]
    fn an_unclosed_marker_does_not_lose_text() {
        assert_eq!(plain("a **b"), "a b");
    }

    #[test]
    fn bold_runs_get_the_strong_colour() {
        let j = job(
            "a **b** c",
            &FontId::default(),
            Color32::WHITE,
            Color32::RED,
            10.0,
        );
        let colours: Vec<Color32> = j.sections.iter().map(|s| s.format.color).collect();
        assert_eq!(colours, [Color32::WHITE, Color32::RED, Color32::WHITE]);
    }
}
