//! The bootstrapper window: a small dialog that shows what's happening while Roblox is
//! installed and started, and any error that stops it. It comes in three styles
//! (Appearance > Progress window), like Bloxstrap's bootstrapper styles.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, Vec2, ViewportCommand,
};
use rbx_core::settings::{BootstrapperIcon, BootstrapperStyle};
use rbx_core::{LaunchOptions, Paths, Settings, Status};

use super::theme::*;
use rbx_deploy::install::Stage;

const WINDOW_SIZE: Vec2 = Vec2::new(460.0, 290.0);
const COMPACT_SIZE: Vec2 = Vec2::new(400.0, 124.0);
const CLASSIC_SIZE: Vec2 = Vec2::new(500.0, 150.0);

/// How long "Have fun!" stays up after Roblox starts.
const FINISH_LINGER: Duration = Duration::from_millis(900);

#[derive(Default)]
struct Shared {
    status: Option<Status>,
    /// Set once the bootstrap task returns.
    outcome: Option<Result<(), String>>,
}

enum Phase {
    /// Roblox is already open; waiting for the user to confirm.
    Confirm,
    Running,
    Finished(Instant),
    Failed(String),
}

struct BootstrapperApp {
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
    logs_dir: std::path::PathBuf,
    phase: Phase,
    speed: SpeedMeter,
    started: Instant,
    /// The launch, held back while [`Phase::Confirm`] is showing.
    pending: Option<(Paths, LaunchOptions)>,
    preview: Option<String>,
    style: BootstrapperStyle,
    title: String,
    icon: Icon,
    /// Grown to the full size to show a question or an error in a small style.
    expanded: bool,
}

/// What the progress window shows as its logo.
pub enum Icon {
    Mark(BootstrapperIcon),
    Custom(egui::TextureHandle),
}

impl Icon {
    /// The chosen icon, falling back to our mark if a custom image can't be loaded.
    pub fn load(ctx: &egui::Context, settings: &Settings) -> Icon {
        if settings.bootstrapper_icon == BootstrapperIcon::Custom
            && let Some(path) = &settings.custom_icon
        {
            match load_image(path) {
                Ok(image) => {
                    return Icon::Custom(ctx.load_texture(
                        "custom-icon",
                        image,
                        Default::default(),
                    ));
                }
                Err(e) => tracing::warn!(error = %e, path, "could not load the custom icon"),
            }
        }
        match settings.bootstrapper_icon {
            BootstrapperIcon::Custom => Icon::Mark(BootstrapperIcon::Rusticean),
            other => Icon::Mark(other),
        }
    }
}

pub fn load_image(path: &str) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::open(path)?.into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, &image))
}

/// Show the window and run the bootstrapper behind it.
///
/// Returns `Ok(false)` when the bootstrap failed (the error was already shown in the
/// window and logged), and `Err` only if the window itself couldn't be opened.
/// `preview` plays a fake install instead (`"1"`, or `"error"` for the error screen), for
/// the Preview button in settings.
pub fn run(paths: Paths, opts: LaunchOptions, preview: Option<String>) -> anyhow::Result<bool> {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    let settings = Settings::load(&paths.settings_file);
    let style = settings.bootstrapper_style;
    let title = match settings.bootstrapper_title.trim() {
        "" => "Rusticean".to_owned(),
        t => t.to_owned(),
    };
    // the classic style imitates a light Windows dialog
    set_light(style == BootstrapperStyle::Classic || wants_light(settings.theme));

    let size = match style {
        BootstrapperStyle::Rusticean => WINDOW_SIZE,
        BootstrapperStyle::Compact => COMPACT_SIZE,
        BootstrapperStyle::Classic => CLASSIC_SIZE,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size(size)
            .with_resizable(false)
            .with_maximize_button(false)
            .with_decorations(style == BootstrapperStyle::Classic)
            .with_icon(app_icon()),
        centered: true,
        ..Default::default()
    };

    let app_shared = shared.clone();
    let app_cancel = cancel.clone();
    let logs_dir = paths.logs.clone();

    eframe::run_native(
        "Rusticean",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            apply_visuals(&cc.egui_ctx);
            // launching while Roblox is open closes the running game, so ask first;
            // with multi-instance on, the running game stays open, so there's nothing to ask
            let confirm = !opts.no_launch
                && settings.confirm_launches
                && !settings.multi_instance
                && rbx_win::sync::is_roblox_running();
            let mut app = BootstrapperApp {
                shared: app_shared,
                cancel: app_cancel,
                logs_dir,
                phase: Phase::Confirm,
                speed: SpeedMeter::default(),
                started: Instant::now(),
                pending: Some((paths, opts)),
                preview,
                style,
                title,
                icon: Icon::load(&cc.egui_ctx, &settings),
                expanded: style == BootstrapperStyle::Rusticean,
            };
            if !confirm {
                app.start(&cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("could not open the window: {e}"))?;

    // the window is closed; if the user closed it early, make sure the worker winds down
    cancel.store(true, Ordering::Relaxed);
    let outcome = shared.lock().unwrap().outcome.clone();
    Ok(!matches!(outcome, Some(Err(e)) if !e.is_empty()))
}

fn spawn_worker(
    paths: Paths,
    opts: LaunchOptions,
    preview: Option<String>,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    std::thread::spawn(move || {
        let on_status = {
            let shared = shared.clone();
            let ctx = ctx.clone();
            Arc::new(move |status: Status| {
                shared.lock().unwrap().status = Some(status);
                ctx.request_repaint();
            })
        };

        // debug builds can also preview from the environment, for screenshots
        let preview = preview.or_else(|| {
            cfg!(debug_assertions)
                .then(|| std::env::var("RUSTICEAN_UI_PREVIEW").ok())
                .flatten()
        });
        if let Some(mode) = preview {
            let result = preview::run(&mode, &*on_status, &cancel);
            shared.lock().unwrap().outcome = Some(result);
            ctx.request_repaint();
            return;
        }

        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(anyhow::Error::from)
            .and_then(|rt| {
                rt.block_on(rbx_core::run(&paths, &opts, on_status, cancel))
                    .map_err(anyhow::Error::from)
            });

        let outcome = match result {
            Ok(()) => Ok(()),
            // cancelling is not an error worth showing
            Err(e) if is_cancelled(&e) => {
                tracing::info!("cancelled by user");
                Err(String::new())
            }
            Err(e) => {
                tracing::error!("{e:#}");
                Err(format!("{e:#}"))
            }
        };
        shared.lock().unwrap().outcome = Some(outcome);
        ctx.request_repaint();
    });
}

fn is_cancelled(e: &anyhow::Error) -> bool {
    matches!(
        e.downcast_ref::<rbx_core::Error>(),
        Some(rbx_core::Error::Deploy(rbx_deploy::Error::Cancelled))
    )
}

impl eframe::App for BootstrapperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let (status, outcome) = {
            let shared = self.shared.lock().unwrap();
            (shared.status.clone(), shared.outcome.clone())
        };

        if let (Phase::Running, Some(outcome)) = (&self.phase, outcome) {
            self.phase = match outcome {
                Ok(()) => Phase::Finished(Instant::now()),
                Err(e) if e.is_empty() => {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    Phase::Finished(Instant::now() - FINISH_LINGER)
                }
                Err(e) => Phase::Failed(e),
            };
        }

        if let Phase::Finished(at) = self.phase {
            if at.elapsed() >= FINISH_LINGER {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(FINISH_LINGER - at.elapsed());
            }
        }

        // closing the window mid-install cancels it
        if ui.input(|i| i.viewport().close_requested()) {
            self.cancel.store(true, Ordering::Relaxed);
        }

        let full = ui.max_rect();
        let busy = matches!(self.phase, Phase::Running | Phase::Finished(_));
        if !busy && !self.expanded {
            // questions and errors need the room of the full-size window
            self.expanded = true;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(WINDOW_SIZE));
        }

        match self.style {
            BootstrapperStyle::Compact if busy => self.compact_view(ui, full, status.as_ref()),
            BootstrapperStyle::Classic if busy => self.classic_view(ui, full, status.as_ref()),
            _ => self.full_view(ui, full, status.as_ref()),
        }

        if matches!(self.phase, Phase::Running) {
            ctx.request_repaint(); // keep the logo moving
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(bg()).to_array()
    }
}

impl BootstrapperApp {
    /// The standard window: logo, headline, progress bar; also used for questions and
    /// errors in every style.
    fn full_view(&mut self, ui: &mut egui::Ui, full: Rect, status: Option<&Status>) {
        let classic = self.style == BootstrapperStyle::Classic;
        ui.painter().rect_filled(full, CornerRadius::ZERO, bg());
        let top = if classic {
            12.0
        } else {
            ui.painter().rect_stroke(
                full,
                CornerRadius::ZERO,
                Stroke::new(1.0, border()),
                StrokeKind::Inside,
            );
            self.title_bar(ui, full, 34.0);
            44.0
        };

        let body = Rect::from_min_max(
            full.min + Vec2::new(28.0, top),
            full.max - Vec2::new(28.0, 22.0),
        );
        let mut body_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(Layout::top_down(Align::Center)),
        );
        match &self.phase {
            Phase::Confirm => self.confirm_view(&mut body_ui),
            Phase::Failed(message) => {
                let message = message.clone();
                self.error_view(&mut body_ui, &message);
            }
            _ => self.progress_view(&mut body_ui, status),
        }
    }

    /// A slim bar: small logo on the left, status and progress on the right.
    fn compact_view(&mut self, ui: &mut egui::Ui, full: Rect, status: Option<&Status>) {
        ui.painter().rect_filled(full, CornerRadius::ZERO, bg());
        ui.painter().rect_stroke(
            full,
            CornerRadius::ZERO,
            Stroke::new(1.0, border()),
            StrokeKind::Inside,
        );
        self.title_bar(ui, full, 30.0);

        let (headline, fraction, detail) = self.describe_now(status);
        let finished = matches!(self.phase, Phase::Finished(_));
        let t = self.started.elapsed().as_secs_f32();
        let body = Rect::from_min_max(
            full.min + Vec2::new(18.0, 36.0),
            full.max - Vec2::new(18.0, 14.0),
        );
        let mut body_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(Layout::left_to_right(Align::Center)),
        );
        draw_icon(&mut body_ui, &self.icon, 44.0, t, finished);
        body_ui.add_space(14.0);
        body_ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(RichText::new(headline).font(bold(14.5)).color(text()));
            progress_bar(ui, fraction, t, ui.available_width());
            ui.label(RichText::new(detail).size(11.5).color(text_dim()));
        });
    }

    /// A plain Windows-style dialog with the system title bar.
    fn classic_view(&mut self, ui: &mut egui::Ui, full: Rect, status: Option<&Status>) {
        ui.painter().rect_filled(full, CornerRadius::ZERO, bg());
        let (headline, fraction, detail) = self.describe_now(status);
        let finished = matches!(self.phase, Phase::Finished(_));
        let t = self.started.elapsed().as_secs_f32();
        let body = full.shrink2(Vec2::new(20.0, 18.0));
        let mut body_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(Layout::left_to_right(Align::Min)),
        );
        draw_icon(&mut body_ui, &self.icon, 48.0, t, finished);
        body_ui.add_space(16.0);
        body_ui.vertical(|ui| {
            ui.label(RichText::new(headline).size(15.0).color(text()));
            ui.add_space(4.0);
            classic_bar(ui, fraction, t);
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(detail).size(12.0).color(text_dim()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !finished && ui.button("Cancel").clicked() {
                        self.cancel.store(true, Ordering::Relaxed);
                    }
                });
            });
        });
    }

    fn describe_now(&mut self, status: Option<&Status>) -> (String, Option<f32>, String) {
        let finished = matches!(self.phase, Phase::Finished(_));
        if self.cancel.load(Ordering::Relaxed) && !finished {
            ("Cancelling…".to_owned(), None, String::new())
        } else if finished {
            ("Have fun!".to_owned(), Some(1.0), String::new())
        } else {
            describe(status, &mut self.speed)
        }
    }

    /// A draggable strip across the top with the app name and a close button.
    fn title_bar(&mut self, ui: &mut egui::Ui, full: Rect, height: f32) {
        let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), height));
        let title = self.title.clone();
        if title_bar(ui, bar, &title, false) == TitleAction::Close {
            self.cancel.store(true, Ordering::Relaxed);
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        if let Some((paths, opts)) = self.pending.take() {
            spawn_worker(
                paths,
                opts,
                self.preview.clone(),
                self.shared.clone(),
                self.cancel.clone(),
                ctx.clone(),
            );
        }
        self.phase = Phase::Running;
        self.started = Instant::now();
    }

    fn confirm_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 20.0, blue());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "!",
            bold(24.0),
            Color32::WHITE,
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new("Roblox is already open")
                .font(bold(18.0))
                .color(text()),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new("Launching again will close the game that's running.")
                .size(13.0)
                .color(text_dim()),
        );

        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.horizontal(|ui| {
                let width = 2.0 * 120.0 + ui.spacing().item_spacing.x;
                ui.add_space((ui.available_width() - width).max(0.0) / 2.0);
                if secondary_button(ui, "Cancel").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                if primary_button(ui, "Launch anyway").clicked() {
                    self.start(ui.ctx());
                }
            });
        });
    }

    fn progress_view(&mut self, ui: &mut egui::Ui, status: Option<&Status>) {
        let finished = matches!(self.phase, Phase::Finished(_));
        let (headline, fraction, detail) = self.describe_now(status);

        ui.add_space(6.0);
        let t = self.started.elapsed().as_secs_f32();
        draw_icon(ui, &self.icon, 64.0, t, finished);
        ui.add_space(18.0);

        ui.label(RichText::new(headline).font(bold(19.0)).color(text()));
        ui.add_space(14.0);
        progress_bar(ui, fraction, t, 340.0);
        ui.add_space(8.0);
        ui.label(RichText::new(detail).size(12.5).color(text_dim()));

        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            if !finished && secondary_button(ui, "Cancel").clicked() {
                self.cancel.store(true, Ordering::Relaxed);
            }
        });
    }

    fn error_view(&mut self, ui: &mut egui::Ui, message: &str) {
        ui.add_space(2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 20.0, red());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "!",
            bold(24.0),
            Color32::WHITE,
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new("Roblox couldn't be started")
                .font(bold(18.0))
                .color(text()),
        );
        ui.add_space(8.0);

        egui::Frame::new()
            .fill(surface())
            .corner_radius(CornerRadius::same(8))
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .max_height(70.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(message).size(12.0).color(text_dim()))
                                .wrap()
                                .selectable(true),
                        );
                    });
            });

        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.horizontal(|ui| {
                // center the pair of buttons
                let width = 2.0 * 120.0 + ui.spacing().item_spacing.x;
                ui.add_space((ui.available_width() - width).max(0.0) / 2.0);
                if secondary_button(ui, "Open logs").clicked() {
                    let _ = std::process::Command::new("explorer")
                        .arg(&self.logs_dir)
                        .spawn();
                }
                if primary_button(ui, "Close").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
        });
    }
}

/// Headline, progress fraction (None = indeterminate) and detail line for a status.
fn describe(status: Option<&Status>, speed: &mut SpeedMeter) -> (String, Option<f32>, String) {
    match status {
        None | Some(Status::Connecting) => ("Connecting to Roblox…".into(), None, String::new()),
        Some(Status::CheckingForUpdates) => ("Checking for updates…".into(), None, String::new()),
        Some(Status::Installing {
            upgrading,
            progress,
        }) => match &progress.stage {
            Stage::Downloading { .. } => {
                let headline = if *upgrading {
                    "Updating Roblox…"
                } else {
                    "Installing Roblox…"
                };
                let fraction = (progress.total > 0)
                    .then(|| progress.downloaded as f32 / progress.total as f32);
                let rate = speed.update(progress.downloaded);
                let mut detail = format!(
                    "{} of {}",
                    format_bytes(progress.downloaded),
                    format_bytes(progress.total)
                );
                if let Some(rate) = rate {
                    detail.push_str(&format!("  ·  {}/s", format_bytes(rate as u64)));
                }
                (headline.into(), fraction, detail)
            }
            Stage::Extracting { done, total } => (
                "Unpacking files…".into(),
                (*total > 0).then(|| *done as f32 / *total as f32),
                format!("{done} of {total} packages"),
            ),
            Stage::Done => ("Finishing up…".into(), Some(1.0), String::new()),
        },
        Some(Status::Starting) => ("Starting Roblox…".into(), None, String::new()),
        Some(Status::Finished) => ("Have fun!".into(), Some(1.0), String::new()),
    }
}

/// The logo at `size` pixels: our mark (two counter-rotating squares, settling when
/// done) in the chosen variant, or the user's own image.
fn draw_icon(ui: &mut egui::Ui, icon: &Icon, size: f32, t: f32, finished: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let painter = ui.painter();
    let k = size / 64.0;

    let variant = match icon {
        Icon::Custom(texture) => {
            // a gentle breathing motion instead of the spin
            let scale = if finished {
                1.0
            } else {
                0.94 + 0.06 * (t * 2.4).sin()
            };
            let r = Rect::from_center_size(c, Vec2::splat(size * scale));
            painter.image(
                texture.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            return;
        }
        Icon::Mark(variant) => *variant,
    };

    let spin = if finished { 0.0 } else { t * 1.6 };
    let outer = square(c, 26.0 * k, 0.26 + spin);
    let inner = square(c, 12.0 * k, 0.26 - spin * 1.5);
    match variant {
        BootstrapperIcon::Outline => {
            painter.add(Shape::closed_line(outer, Stroke::new(3.0 * k, blue())));
            painter.add(Shape::closed_line(inner, Stroke::new(3.0 * k, blue())));
        }
        BootstrapperIcon::Mono => {
            painter.add(Shape::convex_polygon(outer, text(), Stroke::NONE));
            painter.add(Shape::convex_polygon(inner, bg(), Stroke::NONE));
        }
        _ => {
            painter.add(Shape::convex_polygon(outer, blue(), Stroke::NONE));
            painter.add(Shape::convex_polygon(inner, bg(), Stroke::NONE));
        }
    }

    // a soft pulse ring while working
    if !finished {
        let pulse = (t * 1.2).fract();
        let alpha = ((1.0 - pulse) * 90.0) as u8;
        let [r, g, b, _] = blue_light().to_array();
        painter.add(Shape::closed_line(
            square(c, (26.0 + pulse * 10.0) * k, 0.26 + spin),
            Stroke::new(1.5, Color32::from_rgba_unmultiplied(r, g, b, alpha)),
        ));
    }
}

/// A square-cornered bar like Windows' own progress control.
fn classic_bar(ui: &mut egui::Ui, fraction: Option<f32>, t: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::ZERO,
        Color32::from_rgb(0xE6, 0xE6, 0xE6),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_rgb(0xBC, 0xBC, 0xBC)),
        StrokeKind::Inside,
    );
    let green = Color32::from_rgb(0x06, 0xB0, 0x25);
    let inner = rect.shrink(1.0);
    match fraction {
        Some(f) => {
            let fill = Rect::from_min_size(
                inner.min,
                Vec2::new(inner.width() * f.clamp(0.0, 1.0), inner.height()),
            );
            painter.rect_filled(fill, CornerRadius::ZERO, green);
        }
        None => {
            // marquee
            let width = inner.width() * 0.25;
            let x = (t * 0.5).fract() * (inner.width() + width) - width;
            let seg = Rect::from_min_size(
                Pos2::new(inner.left() + x, inner.top()),
                Vec2::new(width, inner.height()),
            )
            .intersect(inner);
            painter.rect_filled(seg, CornerRadius::ZERO, green);
        }
    }
}

fn progress_bar(ui: &mut egui::Ui, fraction: Option<f32>, t: f32, max_width: f32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(max_width), 6.0),
        Sense::hover(),
    );
    let radius = CornerRadius::same(3);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, surface());

    match fraction {
        Some(f) => {
            let fill = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * f.clamp(0.0, 1.0), rect.height()),
            );
            painter.rect_filled(fill, radius, blue());
        }
        None => {
            // a highlight sliding back and forth
            let width = rect.width() * 0.28;
            let x = (t * 0.9).sin() * 0.5 + 0.5;
            let left = rect.left() + (rect.width() - width) * x;
            let seg =
                Rect::from_min_size(Pos2::new(left, rect.top()), Vec2::new(width, rect.height()));
            painter.rect_filled(seg, radius, blue_light());
        }
    }
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let mb = bytes as f64 / MB;
    if mb >= 1000.0 {
        format!("{:.2} GB", mb / 1024.0)
    } else {
        format!("{mb:.1} MB")
    }
}

/// Smoothed download speed from successive byte counts.
#[derive(Default)]
struct SpeedMeter {
    last: Option<(Instant, u64)>,
    rate: Option<f64>,
}

impl SpeedMeter {
    fn update(&mut self, downloaded: u64) -> Option<f64> {
        let now = Instant::now();
        match self.last {
            None => self.last = Some((now, downloaded)),
            Some((then, before)) => {
                let dt = now.duration_since(then).as_secs_f64();
                if dt >= 0.25 {
                    let instant = downloaded.saturating_sub(before) as f64 / dt;
                    self.rate = Some(match self.rate {
                        Some(r) => r * 0.7 + instant * 0.3,
                        None => instant,
                    });
                    self.last = Some((now, downloaded));
                }
            }
        }
        self.rate
    }
}

/// `-preview` (or `RUSTICEAN_UI_PREVIEW=1` / `=error` in debug builds) plays a fake install so the
/// window can be worked on without touching Roblox's servers.
mod preview {
    use super::*;
    use rbx_deploy::install::Progress;

    pub fn run(mode: &str, on_status: &dyn Fn(Status), cancel: &AtomicBool) -> Result<(), String> {
        let pause = |ms| std::thread::sleep(Duration::from_millis(ms));
        let total = 512 * 1024 * 1024;
        let installing = |stage, downloaded| Status::Installing {
            upgrading: false,
            progress: Progress {
                stage,
                downloaded,
                total,
            },
        };

        on_status(Status::Connecting);
        pause(1500);
        if mode == "error" {
            return Err("could not reach any Roblox download mirror (last error: \
                error sending request for url (https://setup.rbxcdn.com/versionStudio))"
                .into());
        }
        on_status(Status::CheckingForUpdates);
        pause(1500);
        for step in 0..=200u64 {
            if cancel.load(Ordering::Relaxed) {
                return Err(String::new());
            }
            let stage = Stage::Downloading {
                package: "RobloxApp.zip".into(),
            };
            on_status(installing(stage, total * step / 200));
            pause(40);
        }
        for done in 0..=20 {
            on_status(installing(Stage::Extracting { done, total: 20 }, total));
            pause(100);
        }
        on_status(Status::Starting);
        pause(1200);
        on_status(Status::Finished);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_bytes(0), "0.0 MB");
        assert_eq!(format_bytes(150 * 1024 * 1024), "150.0 MB");
        assert_eq!(format_bytes(1500 * 1024 * 1024), "1.46 GB");
    }

    #[test]
    fn square_has_four_corners_at_the_right_distance() {
        let pts = square(Pos2::ZERO, 10.0, 0.3);
        assert_eq!(pts.len(), 4);
        for p in pts {
            assert!((p.to_vec2().length() - 10.0 * std::f32::consts::SQRT_2).abs() < 1e-3);
        }
    }
}
