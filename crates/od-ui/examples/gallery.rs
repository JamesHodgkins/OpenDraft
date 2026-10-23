//! Widget gallery: a "New Document" dialog used as a visual benchmark
//! against the Adobe reference. Run with: cargo run -p od-ui --example gallery

use eframe::egui::{self, Color32, Margin};
use od_ui::{icons, widgets, Theme};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenDraft widget gallery")
            .with_inner_size([1090.0, 730.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenDraft widget gallery",
        options,
        Box::new(|cc| {
            let theme = match std::env::var("OD_PALETTE").as_deref() {
                Ok("slate") => Theme::slate(),
                _ => Theme::dark(),
            };
            theme.apply(&cc.egui_ctx);
            Ok(Box::new(Gallery::new(theme)))
        }),
    )
}

const TABS: &[&str] = &[
    "Recent",
    "Saved",
    "Photo",
    "Print",
    "Art & Illustration",
    "Web",
    "Mobile",
    "Film & Video",
];
const PRESETS: &[(&str, &str)] = &[
    ("Default Size", "7 x 5 in @ 300 ppi"),
    ("Landscape, 3 x 2", "3 x 2 in @ 300 ppi"),
    ("Landscape, 6 x 4", "6 x 4 in @ 300 ppi"),
    ("Landscape, 7 x 5", "7 x 5 in @ 300 ppi"),
];
const TEMPLATES: &[(&str, u32)] = &[
    ("Sunny and Natural...", 0x6b5a4a),
    ("Minimalistic Moodb...", 0x8a9aa6),
    ("Poster Mockup Hang...", 0xb89a7a),
    ("Open Vertical Photo...", 0xcfcfcf),
    ("Gallery Wall", 0x35505a),
    ("Monochrome Frames", 0x4a4a4a),
    ("Polaroid Collage", 0x9a8f86),
    ("Plant Studio", 0x8a5e55),
];
const UNITS: &[&str] = &["Inches", "Millimeters", "Pixels"];
const RES_UNITS: &[&str] = &["Pixels/Inch", "Pixels/Centimeter"];
const COLOR_MODES: &[&str] = &["RGB Color", "Grayscale", "CMYK Color"];
const DEPTHS: &[&str] = &["8 bit", "16 bit"];
const BACKGROUNDS: &[&str] = &["White", "Black", "Transparent"];
const PROFILES: &[&str] = &["Working RGB: sRGB IEC61966-2.1", "Adobe RGB (1998)"];
const ASPECTS: &[&str] = &["Square Pixels", "D1/DV NTSC (0.91)"];

struct Gallery {
    theme: Theme,
    tab: usize,
    preset: usize,
    name: String,
    width: f64,
    height: f64,
    resolution: f64,
    unit: usize,
    res_unit: usize,
    color_mode: usize,
    depth: usize,
    background: usize,
    profile: usize,
    aspect: usize,
    landscape: bool,
    artboards: bool,
    search: String,
}

impl Gallery {
    fn new(theme: Theme) -> Self {
        Self {
            theme,
            tab: 2,
            preset: 1,
            name: "Untitled-4".into(),
            width: 3.0,
            height: 2.0,
            resolution: 300.0,
            unit: 0,
            res_unit: 0,
            color_mode: 0,
            depth: 0,
            background: 0,
            profile: 0,
            aspect: 0,
            landscape: true,
            artboards: false,
            search: String::new(),
        }
    }
}

/// A labelled dropdown that fills the row.
fn choice(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: &str,
    options: &[&str],
    selected: &mut usize,
    width: f32,
) {
    widgets::dropdown(ui, theme, id, options[*selected], width, |ui| {
        for (i, option) in options.iter().enumerate() {
            ui.selectable_value(selected, i, *option);
        }
    });
}

impl eframe::App for Gallery {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = self.theme.clone();
        let c = theme.colors.clone();

        egui::TopBottomPanel::top("tabs")
            .frame(
                egui::Frame::new()
                    .fill(c.panel_bg)
                    .inner_margin(Margin::symmetric(16, 4)),
            )
            .show(ctx, |ui| {
                widgets::tab_strip(ui, &theme, &mut self.tab, TABS);
            });

        egui::SidePanel::right("details")
            .exact_width(272.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(c.sidebar_bg)
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                widgets::section_header(ui, &theme, "Preset details");
                widgets::text_field(ui, &theme, &mut self.name, "");
                ui.add_space(6.0);

                widgets::field_label(ui, &theme, "Width");
                ui.horizontal(|ui| {
                    widgets::number_field(ui, &theme, &mut self.width, "", 0.1);
                    let w = ui.available_width();
                    choice(ui, &theme, "unit", UNITS, &mut self.unit, w);
                });

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        widgets::field_label(ui, &theme, "Height");
                        widgets::number_field(ui, &theme, &mut self.height, "", 0.1);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        widgets::field_label(ui, &theme, "Orientation");
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if widgets::icon_toggle(
                                ui,
                                &theme,
                                icons::GALLERY_PORTRAIT,
                                !self.landscape,
                                "Portrait",
                            )
                            .clicked()
                            {
                                self.landscape = false;
                            }
                            if widgets::icon_toggle(
                                ui,
                                &theme,
                                icons::GALLERY_LANDSCAPE,
                                self.landscape,
                                "Landscape",
                            )
                            .clicked()
                            {
                                self.landscape = true;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        widgets::field_label(ui, &theme, "Artboards");
                        ui.checkbox(&mut self.artboards, "");
                    });
                });

                widgets::field_label(ui, &theme, "Resolution");
                ui.horizontal(|ui| {
                    widgets::number_field(ui, &theme, &mut self.resolution, "", 1.0);
                    let w = ui.available_width();
                    choice(ui, &theme, "res_unit", RES_UNITS, &mut self.res_unit, w);
                });

                widgets::field_label(ui, &theme, "Color Mode");
                ui.horizontal(|ui| {
                    choice(ui, &theme, "mode", COLOR_MODES, &mut self.color_mode, 140.0);
                    let w = ui.available_width();
                    choice(ui, &theme, "depth", DEPTHS, &mut self.depth, w);
                });

                widgets::field_label(ui, &theme, "Background Contents");
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 48.0;
                    choice(ui, &theme, "bg", BACKGROUNDS, &mut self.background, w);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(theme.control_height, theme.control_height),
                        egui::Sense::click(),
                    );
                    ui.painter().rect_filled(rect, 3.0, Color32::WHITE);
                });

                widgets::section(ui, &theme, "Advanced Options", true, |ui| {
                    widgets::field_label(ui, &theme, "Color Profile");
                    let w = ui.available_width();
                    choice(ui, &theme, "profile", PROFILES, &mut self.profile, w);
                    widgets::field_label(ui, &theme, "Pixel Aspect Ratio");
                    choice(ui, &theme, "aspect", ASPECTS, &mut self.aspect, w);
                });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                    ui.horizontal(|ui| {
                        // bottom_up + horizontal: add right-to-left so Create is rightmost.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            widgets::primary_button(ui, &theme, "Create");
                            widgets::secondary_button(ui, &theme, "Close");
                        });
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(c.window_bg)
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                    widgets::section_header(
                        ui,
                        &theme,
                        &format!("Blank document presets ({})", PRESETS.len()),
                    );
                    ui.horizontal(|ui| {
                        for (i, (title, subtitle)) in PRESETS.iter().enumerate() {
                            if widgets::preset_card(
                                ui,
                                &theme,
                                icons::GALLERY_PRESET,
                                title,
                                subtitle,
                                self.preset == i,
                            )
                            .clicked()
                            {
                                self.preset = i;
                            }
                        }
                    });
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("View All Presets +").strong());
                    });

                    ui.add_space(8.0);
                    widgets::section_header(
                        ui,
                        &theme,
                        &format!("Templates ({})", TEMPLATES.len() * 3),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for (title, tint) in TEMPLATES {
                            let tint = Color32::from_rgb(
                                (tint >> 16) as u8,
                                (tint >> 8) as u8,
                                *tint as u8,
                            );
                            widgets::template_card(ui, &theme, tint, title, "FREE");
                        }
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        widgets::text_field(
                            ui,
                            &theme,
                            &mut self.search,
                            "Find more templates on Adobe Stock",
                        );
                    });
                });
            });
    }
}
