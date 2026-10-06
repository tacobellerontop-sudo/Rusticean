//! The bootstrapper window: a small dark dialog in Roblox's style that shows what's
//! happening while Roblox is installed and started, and any error that stops it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, Vec2, ViewportCommand,
};
use rbx_core::{LaunchOptions, Paths, Settings, Status};

use super::theme::*;
use rbx_deploy::install::Stage;

const WINDOW_SIZE: Vec2 = Vec2::new(460.0, 290.0);

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
}

/// Show the window and run the bootstrapper behind it.
///
/// Returns `Ok(false)` when the bootstrap failed (the error was already shown in the
/// window and logged), and `Err` only if the window itself couldn't be opened.
pub fn run(paths: Paths, opts: LaunchOptions) -> anyhow::Result<bool> {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let cancel = Arc::new(AtomicBool::new(false));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("robloxbootstrapper")
            .with_inner_size(WINDOW_SIZE)
            .with_resizable(false)
            .with_decorations(false)
            .with_icon(app_icon()),
        centered: true,
        ..Default::default()
    };

    let app_shared = shared.clone();
    let app_cancel = cancel.clone();
    let logs_dir = paths.logs.clone();

    eframe::run_native(
        "robloxbootstrapper",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            // launching while Roblox is open closes the running game, so ask first
            // with multi-instance on, the running game stays open, so there's nothing to ask
            let settings = Settings::load(&paths.settings_file);
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

        #[cfg(debug_assertions)]
        if let Ok(mode) = std::env::var("RBXB_UI_PREVIEW") {
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
        ui.painter().rect_filled(full, CornerRadius::ZERO, BG);
        ui.painter().rect_stroke(
            full,
            CornerRadius::ZERO,
            Stroke::new(1.0, BORDER),
            StrokeKind::Inside,
        );

        self.title_bar(ui, full);

        let body = Rect::from_min_max(
            full.min + Vec2::new(28.0, 44.0),
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
            _ => self.progress_view(&mut body_ui, status.as_ref()),
        }

        if matches!(self.phase, Phase::Running) {
            ctx.request_repaint(); // keep the logo moving
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(BG).to_array()
    }
}

impl BootstrapperApp {
    /// A draggable strip across the top with the app name and a close button.
    fn title_bar(&mut self, ui: &mut egui::Ui, full: Rect) {
        let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), 34.0));
        if title_bar(ui, bar, "robloxbootstrapper", false) == TitleAction::Close {
            self.cancel.store(true, Ordering::Relaxed);
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        if let Some((paths, opts)) = self.pending.take() {
            spawn_worker(
                paths,
                opts,
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
        ui.painter().circle_filled(rect.center(), 20.0, BLUE);
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
                .color(TEXT),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new("Launching again will close the game that's running.")
                .size(13.0)
                .color(TEXT_DIM),
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
        let cancelling = self.cancel.load(Ordering::Relaxed) && !finished;

        let (headline, fraction, detail) = if cancelling {
            ("Cancelling…".to_owned(), None, String::new())
        } else if finished {
            ("Have fun!".to_owned(), Some(1.0), String::new())
        } else {
            describe(status, &mut self.speed)
        };

        ui.add_space(6.0);
        let t = self.started.elapsed().as_secs_f32();
        logo(ui, t, finished);
        ui.add_space(18.0);

        ui.label(RichText::new(headline).font(bold(19.0)).color(TEXT));
        ui.add_space(14.0);
        progress_bar(ui, fraction, t);
        ui.add_space(8.0);
        ui.label(RichText::new(detail).size(12.5).color(TEXT_DIM));

        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            if !finished && secondary_button(ui, "Cancel").clicked() {
                self.cancel.store(true, Ordering::Relaxed);
            }
        });
    }

    fn error_view(&mut self, ui: &mut egui::Ui, message: &str) {
        ui.add_space(2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 20.0, RED);
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
                .color(TEXT),
        );
        ui.add_space(8.0);

        egui::Frame::new()
            .fill(SURFACE)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .max_height(70.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(message).size(12.0).color(TEXT_DIM))
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

/// Our mark: two counter-rotating squares in Roblox blue, settling square when done.
fn logo(ui: &mut egui::Ui, t: f32, finished: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
    let c = rect.center();
    let painter = ui.painter();

    let spin = if finished { 0.0 } else { t * 1.6 };
    let outer = square(c, 26.0, 0.26 + spin);
    let inner = square(c, 12.0, 0.26 - spin * 1.5);

    painter.add(Shape::convex_polygon(outer, BLUE, Stroke::NONE));
    painter.add(Shape::convex_polygon(inner, BG, Stroke::NONE));

    // a soft pulse ring while working
    if !finished {
        let pulse = (t * 1.2).fract();
        let alpha = ((1.0 - pulse) * 90.0) as u8;
        painter.add(Shape::closed_line(
            square(c, 26.0 + pulse * 10.0, 0.26 + spin),
            Stroke::new(
                1.5,
                Color32::from_rgba_unmultiplied(0x6E, 0x8C, 0xFF, alpha),
            ),
        ));
    }
}

fn progress_bar(ui: &mut egui::Ui, fraction: Option<f32>, t: f32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(340.0), 6.0),
        Sense::hover(),
    );
    let radius = CornerRadius::same(3);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, SURFACE);

    match fraction {
        Some(f) => {
            let fill = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * f.clamp(0.0, 1.0), rect.height()),
            );
            painter.rect_filled(fill, radius, BLUE);
        }
        None => {
            // a highlight sliding back and forth
            let width = rect.width() * 0.28;
            let x = (t * 0.9).sin() * 0.5 + 0.5;
            let left = rect.left() + (rect.width() - width) * x;
            let seg =
                Rect::from_min_size(Pos2::new(left, rect.top()), Vec2::new(width, rect.height()));
            painter.rect_filled(seg, radius, BLUE_LIGHT);
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

/// Debug builds only: `RBXB_UI_PREVIEW=1` (or `=error`) plays a fake install so the
/// window can be worked on without touching Roblox's servers.
#[cfg(debug_assertions)]
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
