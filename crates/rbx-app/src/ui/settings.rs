//! The settings window: launch options, FastFlags and an About page. Changes save as
//! soon as they're made; "Launch Roblox" closes the window and starts the game.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Sense, Vec2};
use rbx_core::settings::{RenderingApi, parse_flag_value};
use rbx_core::state::State;
use rbx_core::{Paths, Settings};

use super::theme::*;

const WINDOW_SIZE: Vec2 = Vec2::new(760.0, 520.0);
const SIDEBAR_WIDTH: f32 = 200.0;
const REPO_URL: &str = "https://github.com/tacobellerontop-sudo/fictional-garbanzo";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Launch,
    FastFlags,
    About,
}

impl Page {
    const ALL: [Page; 3] = [Page::Launch, Page::FastFlags, Page::About];

    fn label(self) -> &'static str {
        match self {
            Page::Launch => "Launch",
            Page::FastFlags => "FastFlags",
            Page::About => "About",
        }
    }
}

struct SettingsApp {
    paths: Paths,
    settings: Settings,
    saved: Settings,
    page: Page,
    /// When the last save happened, for the "Saved" hint, or the error if it failed.
    save_result: Option<(Instant, Result<(), String>)>,
    channel_text: String,
    new_flag_name: String,
    new_flag_value: String,
    reinstall_requested: bool,
    launch_requested: bool,
}

/// Show the settings window. Returns `true` if the user clicked "Launch Roblox".
pub fn run(paths: Paths) -> anyhow::Result<bool> {
    let settings = Settings::load(&paths.settings_file);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("robloxbootstrapper settings")
            .with_inner_size(WINDOW_SIZE)
            .with_min_inner_size(Vec2::new(640.0, 440.0))
            .with_icon(app_icon()),
        centered: true,
        ..Default::default()
    };

    let launch = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let launch_flag = launch.clone();

    eframe::run_native(
        "robloxbootstrapper-settings",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            apply_visuals(&cc.egui_ctx);
            Ok(Box::new(Wrapper {
                app: SettingsApp {
                    channel_text: settings.channel.clone().unwrap_or_default(),
                    saved: settings.clone(),
                    settings,
                    paths,
                    page: Page::Launch,
                    save_result: None,
                    new_flag_name: String::new(),
                    new_flag_value: String::new(),
                    reinstall_requested: false,
                    launch_requested: false,
                },
                launch: launch_flag,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("could not open the settings window: {e}"))?;

    Ok(launch.load(std::sync::atomic::Ordering::Relaxed))
}

/// Carries the "launch" answer out of eframe, which owns the app.
struct Wrapper {
    app: SettingsApp,
    launch: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl eframe::App for Wrapper {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
        if self.app.launch_requested {
            self.launch
                .store(true, std::sync::atomic::Ordering::Relaxed);
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(BG).to_array()
    }
}

impl SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .exact_size(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(egui::Frame::new().fill(SURFACE).inner_margin(16))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(28, 22)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Launch => self.launch_page(ui),
                        Page::FastFlags => self.fast_flags_page(ui),
                        Page::About => self.about_page(ui),
                    });
            });

        self.autosave();
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            draw_mark(ui.painter(), rect.center(), 11.0, SURFACE);
            ui.label(
                RichText::new("robloxbootstrapper")
                    .font(bold(14.5))
                    .color(TEXT),
            );
        });
        ui.add_space(18.0);

        for page in Page::ALL {
            let selected = self.page == page;
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
            let fill = if selected {
                Color32::from_rgb(0x2E, 0x33, 0x45)
            } else if resp.hovered() {
                Color32::from_rgb(0x2B, 0x2D, 0x30)
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
            if selected {
                let bar =
                    egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 8.0), Vec2::new(3.0, 18.0));
                ui.painter().rect_filled(bar, CornerRadius::same(2), BLUE);
            }
            ui.painter().text(
                rect.left_center() + Vec2::new(14.0, 0.0),
                egui::Align2::LEFT_CENTER,
                page.label(),
                if selected {
                    bold(14.0)
                } else {
                    egui::FontId::proportional(14.0)
                },
                if selected { TEXT } else { TEXT_DIM },
            );
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.page = page;
            }
        }

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            match &self.save_result {
                Some((_, Err(e))) => {
                    ui.label(
                        RichText::new(format!("Couldn't save: {e}"))
                            .size(11.5)
                            .color(RED),
                    );
                }
                Some((at, Ok(()))) if at.elapsed() < Duration::from_secs(2) => {
                    ui.label(RichText::new("Saved").size(12.0).color(TEXT_DIM));
                    ui.ctx().request_repaint_after(Duration::from_millis(250));
                }
                _ => {
                    ui.label(
                        RichText::new("Changes save automatically")
                            .size(11.5)
                            .color(TEXT_DIM),
                    );
                }
            }
            ui.add_space(4.0);
            if wide_primary_button(ui, "Launch Roblox").clicked() {
                self.launch_requested = true;
            }
        });
    }

    fn launch_page(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Launch", "How Roblox is started and kept up to date.");

        card(ui, |ui| {
            setting_row(
                ui,
                "Show progress window",
                "Show the window with download progress while Roblox starts.",
                |ui| {
                    toggle(ui, &mut self.settings.show_progress_window);
                },
            );
        });

        card(ui, |ui| {
            setting_row(
                ui,
                "Channel",
                "Roblox release channel. Leave empty to follow the launch link and Roblox's own setting.",
                |ui| {
                    let edit = egui::TextEdit::singleline(&mut self.channel_text)
                        .hint_text("Automatic")
                        .desired_width(150.0);
                    if ui.add(edit).changed() {
                        let trimmed = self.channel_text.trim();
                        self.settings.channel =
                            (!trimmed.is_empty()).then(|| trimmed.to_ascii_lowercase());
                    }
                },
            );
        });

        card(ui, |ui| {
            setting_row(
                ui,
                "Reinstall Roblox",
                if self.reinstall_requested {
                    "Done. Roblox will be downloaded again the next time it launches."
                } else {
                    "Downloads a fresh copy on the next launch. Useful if Roblox won't start."
                },
                |ui| {
                    if ui
                        .add_enabled(!self.reinstall_requested, egui::Button::new("Reinstall"))
                        .clicked()
                    {
                        let mut state = State::load(&self.paths.state_file);
                        state.player.version_guid = None;
                        match state.save(&self.paths.state_file) {
                            Ok(()) => self.reinstall_requested = true,
                            Err(e) => self.save_result = Some((Instant::now(), Err(e.to_string()))),
                        }
                    }
                },
            );
            ui.separator();
            setting_row(
                ui,
                "Roblox files",
                "The installed versions, downloads cache and logs.",
                |ui| {
                    if ui.button("Open folder").clicked() {
                        open_path(&self.paths.base);
                    }
                },
            );
        });
    }

    fn fast_flags_page(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "FastFlags",
            "Tweak Roblox's engine settings. Applied every time Roblox launches.",
        );

        card(ui, |ui| {
            setting_row(
                ui,
                "Frame rate limit",
                "Roblox caps the frame rate at 60 by default.",
                |ui| {
                    let mut enabled = self.settings.fps_limit.is_some();
                    if let Some(fps) = &mut self.settings.fps_limit {
                        ui.add(
                            egui::DragValue::new(fps)
                                .range(30..=1000)
                                .suffix(" FPS")
                                .speed(1.0),
                        );
                    }
                    if toggle(ui, &mut enabled).changed() {
                        self.settings.fps_limit = enabled.then_some(240);
                    }
                },
            );
            if self.settings.fps_limit.is_some() {
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    for preset in [60, 120, 144, 165, 240, 360] {
                        let selected = self.settings.fps_limit == Some(preset);
                        if ui.selectable_label(selected, preset.to_string()).clicked() {
                            self.settings.fps_limit = Some(preset);
                        }
                    }
                });
            }
            ui.separator();
            setting_row(
                ui,
                "Graphics API",
                "Which rendering backend Roblox should prefer.",
                |ui| {
                    egui::ComboBox::from_id_salt("rendering_api")
                        .selected_text(self.settings.rendering_api.label())
                        .width(140.0)
                        .show_ui(ui, |ui| {
                            for api in RenderingApi::ALL {
                                ui.selectable_value(
                                    &mut self.settings.rendering_api,
                                    api,
                                    api.label(),
                                );
                            }
                        });
                },
            );
        });

        ui.add_space(4.0);
        ui.label(RichText::new("Custom flags").font(bold(15.0)).color(TEXT));
        ui.label(
            RichText::new(
                "Roblox only honours flags on its approved list; anything else is ignored. \
                 Values can be true, false, a whole number or text.",
            )
            .size(12.0)
            .color(TEXT_DIM),
        );
        ui.add_space(4.0);

        card(ui, |ui| {
            let mut remove = None;
            if self.settings.fast_flags.is_empty() {
                ui.label(RichText::new("No custom flags yet.").color(TEXT_DIM));
            }
            for (name, value) in &self.settings.fast_flags {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).monospace().color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if remove_button(ui).on_hover_text("Remove").clicked() {
                            remove = Some(name.clone());
                        }
                        ui.label(
                            RichText::new(value.to_string())
                                .monospace()
                                .color(BLUE_LIGHT),
                        );
                    });
                });
            }
            if let Some(name) = remove {
                self.settings.fast_flags.remove(&name);
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_flag_name)
                        .hint_text("FFlagName")
                        .desired_width(ui.available_width() - 220.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_flag_value)
                        .hint_text("value")
                        .desired_width(120.0),
                );
                let name = self.new_flag_name.trim().to_owned();
                let valid = !name.is_empty() && !self.new_flag_value.trim().is_empty();
                if ui.add_enabled(valid, egui::Button::new("Add")).clicked() {
                    self.settings
                        .fast_flags
                        .insert(name, parse_flag_value(&self.new_flag_value));
                    self.new_flag_name.clear();
                    self.new_flag_value.clear();
                }
            });
        });

        egui::CollapsingHeader::new(RichText::new("What Roblox will see").color(TEXT_DIM))
            .id_salt("preview")
            .show(ui, |ui| {
                let json = serde_json::to_string_pretty(&self.settings.effective_fast_flags())
                    .unwrap_or_default();
                let mut text = json.as_str();
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            });
    }

    fn about_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
            draw_mark(ui.painter(), rect.center(), 30.0, BG);
            ui.add_space(12.0);
            ui.label(RichText::new("robloxbootstrapper").font(bold(22.0)).color(TEXT));
            ui.label(
                RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                    .size(13.0)
                    .color(TEXT_DIM),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new("A Roblox bootstrapper written in Rust, inspired by Bloxstrap.")
                    .color(TEXT_DIM),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let width = 3.0 * 120.0 + 2.0 * ui.spacing().item_spacing.x;
                ui.add_space((ui.available_width() - width).max(0.0) / 2.0);
                if secondary_button(ui, "Data folder").clicked() {
                    open_path(&self.paths.base);
                }
                if secondary_button(ui, "Logs").clicked() {
                    open_path(&self.paths.logs);
                }
                if secondary_button(ui, "GitHub").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(REPO_URL));
                }
            });
            ui.add_space(24.0);
            ui.label(
                RichText::new(
                    "Not affiliated with Roblox Corporation. Based on Bloxstrap (MIT, © pizzaboxer).",
                )
                .size(11.5)
                .color(TEXT_DIM),
            );
        });
    }

    fn autosave(&mut self) {
        if self.settings == self.saved {
            return;
        }
        let result = self
            .settings
            .save(&self.paths.settings_file)
            .map_err(|e| e.to_string());
        if result.is_ok() {
            self.saved = self.settings.clone();
        }
        self.save_result = Some((Instant::now(), result));
    }
}

fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).font(bold(24.0)).color(TEXT));
    ui.label(RichText::new(subtitle).size(13.0).color(TEXT_DIM));
    ui.add_space(14.0);
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner;
    ui.add_space(10.0);
    inner
}

/// A title and description on the left, a control on the right.
fn setting_row(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    control: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        let text_width = (ui.available_width() - 230.0).max(200.0);
        ui.vertical(|ui| {
            ui.set_width(text_width);
            ui.label(RichText::new(title).font(bold(14.5)).color(TEXT));
            ui.add(egui::Label::new(RichText::new(description).size(12.0).color(TEXT_DIM)).wrap());
        });
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
}

/// An iOS-style switch in Roblox blue.
fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = Vec2::new(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = BORDER.lerp_to_gamma(BLUE, t);
    ui.painter().rect_filled(rect, CornerRadius::same(11), bg);
    let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 8.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small square button with a drawn ✕ (not every font has the glyph).
fn remove_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), BORDER);
    }
    let c = rect.center();
    let s = 4.0;
    let stroke = egui::Stroke::new(1.5, if resp.hovered() { RED } else { TEXT_DIM });
    ui.painter()
        .line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], stroke);
    ui.painter()
        .line_segment([c + Vec2::new(-s, s), c + Vec2::new(s, -s)], stroke);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn wide_primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
    let fill = if resp.hovered() { BLUE_LIGHT } else { BLUE };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("▶  {text}"),
        bold(14.5),
        Color32::WHITE,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn open_path(path: &std::path::Path) {
    let opener = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(opener).arg(path).spawn() {
        tracing::warn!(error = %e, path = %path.display(), "could not open folder");
    }
}
