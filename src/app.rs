use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use chrono::{Datelike, Days, Local, NaiveDate, Utc};
use eframe::egui::{
    self, Align, Color32, FontId, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2,
    ViewportCommand,
};
use eframe::egui::{ViewportBuilder, ViewportId};
use uuid::Uuid;

use crate::{
    model::{ActiveTimer, Project, TimeEntry, TrackerData, billed_tenths, format_elapsed},
    storage::Storage,
};

// Tenth brand palette: calm enough for an all-day utility, bright at the moments
// that matter. Keep these values aligned with BRAND.md and assets/tenth-mark.svg.
static DARK_MODE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct ThemeColor {
    light: Color32,
    dark: Color32,
}

impl ThemeColor {
    const fn new(light: Color32, dark: Color32) -> Self {
        Self { light, dark }
    }

    fn get(self) -> Color32 {
        if DARK_MODE.load(Ordering::Relaxed) {
            self.dark
        } else {
            self.light
        }
    }
}

impl From<ThemeColor> for Color32 {
    fn from(color: ThemeColor) -> Self {
        color.get()
    }
}

const INK: ThemeColor = ThemeColor::new(
    Color32::from_rgb(24, 48, 65),
    Color32::from_rgb(229, 238, 235),
);
const MUTED: ThemeColor = ThemeColor::new(
    Color32::from_rgb(104, 119, 129),
    Color32::from_rgb(154, 174, 170),
);
const ACCENT: ThemeColor = ThemeColor::new(
    Color32::from_rgb(11, 143, 123),
    Color32::from_rgb(11, 143, 123),
);
const ACCENT_DARK: ThemeColor = ThemeColor::new(
    Color32::from_rgb(8, 103, 89),
    Color32::from_rgb(101, 222, 196),
);
const ACCENT_SOFT: ThemeColor = ThemeColor::new(
    Color32::from_rgb(221, 243, 236),
    Color32::from_rgb(27, 72, 65),
);
const SIGNAL: ThemeColor = ThemeColor::new(
    Color32::from_rgb(198, 227, 107),
    Color32::from_rgb(203, 231, 116),
);
const DANGER: ThemeColor = ThemeColor::new(
    Color32::from_rgb(203, 81, 74),
    Color32::from_rgb(203, 81, 74),
);
const DANGER_SOFT: ThemeColor = ThemeColor::new(
    Color32::from_rgb(252, 234, 231),
    Color32::from_rgb(79, 42, 40),
);
const BG: ThemeColor = ThemeColor::new(
    Color32::from_rgb(244, 246, 242),
    Color32::from_rgb(17, 27, 31),
);
const SURFACE: ThemeColor = ThemeColor::new(
    Color32::from_rgb(255, 255, 252),
    Color32::from_rgb(27, 40, 44),
);
const SURFACE_MUTED: ThemeColor = ThemeColor::new(
    Color32::from_rgb(248, 250, 247),
    Color32::from_rgb(34, 49, 52),
);
const LINE: ThemeColor = ThemeColor::new(
    Color32::from_rgb(216, 223, 218),
    Color32::from_rgb(57, 74, 76),
);
const TOOLBAR_CONTROL_HEIGHT: f32 = 36.0;
const REMINDER_SIZE: Vec2 = Vec2::new(400.0, 124.0);
const REMINDER_EDGE_GAP: f32 = 20.0;

#[derive(PartialEq, Eq)]
enum View {
    Track,
    Week,
}

pub struct HourTrackerApp {
    data: TrackerData,
    storage: Storage,
    selected_project: Option<Uuid>,
    new_project_name: String,
    adding_project: bool,
    view: View,
    week_start: NaiveDate,
    manual_day: usize,
    manual_tenths: i64,
    manual_note: String,
    timer_note: String,
    editing_entry: Option<Uuid>,
    edit_project: Option<Uuid>,
    edit_day: usize,
    edit_week_start: NaiveDate,
    edit_tenths: i64,
    edit_note: String,
    entry_picker_cell: Option<(Uuid, u64)>,
    status: Option<(String, bool)>,
    last_window_rect: Option<egui::Rect>,
    reminder_origin: Option<egui::Pos2>,
    compact_requested: bool,
    #[cfg(target_os = "macos")]
    menu_bar: Option<crate::macos_menu_bar::MacMenuBar>,
    #[cfg(target_os = "macos")]
    menu_bar_initialized: bool,
}

impl HourTrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let storage = Storage::new();
        let (data, status) = match storage.load() {
            Ok(data) => (data, None),
            Err(error) => (TrackerData::default(), Some((error, true))),
        };
        let selected_project = data
            .active_timer
            .as_ref()
            .map(|timer| timer.project_id)
            .or_else(|| data.projects.first().map(|project| project.id));
        let today = Local::now().date_naive();
        let timer_note = data
            .active_timer
            .as_ref()
            .map(|timer| timer.note.clone())
            .unwrap_or_default();
        configure_style(&cc.egui_ctx, data.dark_mode);

        Self {
            data,
            storage,
            selected_project,
            new_project_name: String::new(),
            adding_project: false,
            view: View::Track,
            week_start: saturday_of(today),
            manual_day: days_from_saturday(today) as usize,
            manual_tenths: 1,
            manual_note: String::new(),
            timer_note,
            editing_entry: None,
            edit_project: None,
            edit_day: 0,
            edit_week_start: saturday_of(today),
            edit_tenths: 1,
            edit_note: String::new(),
            entry_picker_cell: None,
            status,
            last_window_rect: None,
            reminder_origin: None,
            compact_requested: false,
            #[cfg(target_os = "macos")]
            menu_bar: None,
            #[cfg(target_os = "macos")]
            menu_bar_initialized: false,
        }
    }

    fn save(&mut self, message: impl Into<String>) {
        self.status = Some(match self.storage.save(&self.data) {
            Ok(()) => (message.into(), false),
            Err(error) => (error, true),
        });
    }

    fn add_project(&mut self) {
        let name = self.new_project_name.trim();
        if name.is_empty() {
            return;
        }
        if self
            .data
            .projects
            .iter()
            .any(|project| project.name.eq_ignore_ascii_case(name))
        {
            self.status = Some(("That project already exists.".into(), true));
            return;
        }
        let project = Project::new(name.to_owned());
        self.selected_project = Some(project.id);
        self.data.projects.push(project);
        self.data
            .projects
            .sort_by_key(|project| project.name.to_lowercase());
        self.new_project_name.clear();
        self.adding_project = false;
        self.save("Project added");
    }

    fn start_timer(&mut self) {
        if self.data.active_timer.is_some() {
            return;
        }
        let Some(project_id) = self.selected_project else {
            return;
        };
        self.data.active_timer = Some(ActiveTimer {
            project_id,
            started_at: Utc::now(),
            note: self.timer_note.trim().to_owned(),
        });
        self.save("Timer started");
    }

    fn stop_timer(&mut self) {
        let Some(timer) = self.data.active_timer.take() else {
            return;
        };
        let mut entry = TimeEntry::from_session(timer.project_id, timer.started_at, Utc::now());
        entry.note = timer.note;
        self.data.entries.push(entry);
        self.timer_note.clear();
        self.compact_requested = false;
        self.sort_entries();
        self.save("Entry saved");
    }

    fn minimized_reminder(&mut self, ctx: &egui::Context) {
        // macOS suppresses native redraws when a window is miniaturized.
        // Create its floating viewport through the explicit Compact timer action
        // before minimizing; the native yellow button retains menu-bar tracking.
        let minimized = !cfg!(target_os = "macos")
            && ctx.input(|input| input.viewport().minimized.unwrap_or(false));
        if !minimized && let Some(rect) = ctx.input(|input| input.viewport().outer_rect) {
            self.last_window_rect = Some(rect);
        }
        let Some(timer) = self
            .data
            .active_timer
            .clone()
            .filter(|_| minimized || self.compact_requested)
        else {
            self.reminder_origin = None;
            return;
        };

        // Anchor in the main window's desktop coordinates, including negative
        // origins on displays to the left/above the primary display. Keep the
        // initial builder position stable so repainting never undoes a drag.
        let position = *self.reminder_origin.get_or_insert_with(|| {
            reminder_position(self.last_window_rect, REMINDER_SIZE, REMINDER_EDGE_GAP)
        });
        let project_name = self.project_name(timer.project_id).to_owned();
        let elapsed = (Utc::now() - timer.started_at).num_seconds().max(0);
        let reminder_id = ViewportId::from_hash_of("active_timer_reminder");
        let builder = ViewportBuilder::default()
            .with_title("Tenth · Tracking")
            .with_inner_size(REMINDER_SIZE)
            .with_min_inner_size(REMINDER_SIZE)
            .with_max_inner_size(REMINDER_SIZE)
            .with_position(position)
            .with_active(false)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_taskbar(false)
            .with_always_on_top();
        let reminder_fill = SURFACE.get();
        let reminder_stroke = ACCENT.get();

        ctx.show_viewport_immediate(reminder_id, builder, |reminder_ctx, _class| {
            reminder_ctx.request_repaint_after(Duration::from_secs(1));
            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(
                            reminder_fill.r(),
                            reminder_fill.g(),
                            reminder_fill.b(),
                            if reminder_ctx
                                .input(|input| input.focused || input.pointer.hover_pos().is_some())
                            {
                                248
                            } else {
                                185
                            },
                        ))
                        .stroke(Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(
                                reminder_stroke.r(),
                                reminder_stroke.g(),
                                reminder_stroke.b(),
                                110,
                            ),
                        ))
                        .corner_radius(14.0)
                        .inner_margin(Margin::symmetric(16, 12)),
                )
                .show(reminder_ctx, |ui| {
                    let drag = ui.interact(
                        ui.max_rect(),
                        ui.id().with("reminder_drag_surface"),
                        Sense::click_and_drag(),
                    );
                    if drag.drag_started() {
                        reminder_ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }

                    ui.horizontal(|ui| {
                        brand_mark(ui, 18.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(compact_message(&project_name, 22))
                                    .size(13.0)
                                    .strong()
                                    .color(INK),
                            );
                            ui.label(
                                RichText::new("TRACKING NOW")
                                    .size(10.0)
                                    .strong()
                                    .color(ACCENT),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format_elapsed(elapsed))
                                    .font(FontId::monospace(22.0))
                                    .strong()
                                    .color(INK),
                            );
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new("Restore app").color(ACCENT_DARK))
                            .clicked()
                        {
                            self.compact_requested = false;
                            reminder_ctx.send_viewport_cmd_to(
                                ViewportId::ROOT,
                                ViewportCommand::Minimized(false),
                            );
                            reminder_ctx
                                .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
                        }
                        if ui
                            .button(RichText::new("■  Stop & save").color(DANGER))
                            .clicked()
                        {
                            self.stop_timer();
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{:.1} h billable",
                                    billed_tenths(elapsed) as f64 / 10.0
                                ))
                                .size(11.0)
                                .color(MUTED),
                            );
                        });
                    });
                });
        });
    }

    #[cfg(target_os = "macos")]
    fn macos_menu_bar(&mut self, ctx: &egui::Context) {
        use crate::macos_menu_bar::{MacMenuBar, MenuBarAction};

        if !self.menu_bar_initialized {
            self.menu_bar_initialized = true;
            match MacMenuBar::new(ctx) {
                Ok(menu_bar) => self.menu_bar = Some(menu_bar),
                Err(error) => self.status = Some((error, true)),
            }
        }

        let selected_project_name = self
            .selected_project
            .map(|id| self.project_name(id).to_owned());
        let active_timer = self.data.active_timer.as_ref().map(|timer| {
            let elapsed = (Utc::now() - timer.started_at).num_seconds().max(0);
            (
                self.project_name(timer.project_id).to_owned(),
                elapsed,
                billed_tenths(elapsed),
            )
        });

        if let Some(menu_bar) = &self.menu_bar {
            menu_bar.update(
                selected_project_name.as_deref(),
                active_timer
                    .as_ref()
                    .map(|(project, elapsed, billable)| (project.as_str(), *elapsed, *billable)),
            );
        }

        let action = self.menu_bar.as_ref().and_then(MacMenuBar::next_action);
        match action {
            Some(MenuBarAction::Start) if self.data.active_timer.is_none() => self.start_timer(),
            Some(MenuBarAction::Stop) => self.stop_timer(),
            Some(MenuBarAction::Open) => {
                self.compact_requested = false;
                ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            Some(MenuBarAction::Quit) => ctx.send_viewport_cmd(ViewportCommand::Close),
            _ => {}
        }
    }

    fn add_manual_entry(&mut self) {
        let Some(project_id) = self.selected_project else {
            return;
        };
        let date = self.week_start + Days::new(self.manual_day as u64);
        let mut entry = TimeEntry::manual(project_id, date, self.manual_tenths);
        entry.note = self.manual_note.trim().to_owned();
        self.data.entries.push(entry);
        self.manual_note.clear();
        self.sort_entries();
        self.save(format!(
            "Added {:.1} h on {}",
            self.manual_tenths as f64 / 10.0,
            date.format("%a %b %-d")
        ));
    }

    fn sort_entries(&mut self) {
        self.data
            .entries
            .sort_by(|a, b| b.started_at.cmp(&a.started_at));
    }

    fn begin_edit_entry(&mut self, entry_id: Uuid) {
        let Some(entry) = self.data.entries.iter().find(|entry| entry.id == entry_id) else {
            return;
        };
        let date = entry.started_at.with_timezone(&Local).date_naive();
        self.editing_entry = Some(entry.id);
        self.edit_project = Some(entry.project_id);
        self.edit_week_start = saturday_of(date);
        self.edit_day = days_from_saturday(date) as usize;
        self.edit_tenths = entry.billed_tenths;
        self.edit_note = entry.note.clone();
        self.entry_picker_cell = None;
    }

    fn save_entry_edit(&mut self) {
        let (Some(entry_id), Some(project_id)) = (self.editing_entry, self.edit_project) else {
            return;
        };
        let date = self.edit_week_start + Days::new(self.edit_day as u64);
        let Some(entry) = self
            .data
            .entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
        else {
            self.editing_entry = None;
            return;
        };
        entry.project_id = project_id;
        entry.update_manual_values(date, self.edit_tenths);
        entry.note = self.edit_note.trim().to_owned();
        self.editing_entry = None;
        self.sort_entries();
        self.save("Entry updated");
    }

    fn project_name(&self, id: Uuid) -> &str {
        self.data
            .projects
            .iter()
            .find(|project| project.id == id)
            .map(|project| project.name.as_str())
            .unwrap_or("Unknown project")
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        let mut toggle_theme = false;
        egui::TopBottomPanel::top("top")
            .exact_height(64.0)
            .frame(
                Frame::new()
                    .fill(SURFACE.into())
                    .inner_margin(Margin::symmetric(18, 10))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ctx, |ui| {
                let compact = ui.available_width() < 720.0;
                ui.horizontal(|ui| {
                    brand_mark(ui, 30.0);
                    if !compact {
                        ui.label(RichText::new("Tenth").size(20.0).strong().color(INK));
                        ui.add_space(22.0);
                    } else {
                        ui.add_space(4.0);
                    }
                    if tab(ui, "Timer", self.view == View::Track).clicked() {
                        self.view = View::Track;
                    }
                    if tab(ui, "Timesheet", self.view == View::Week).clicked() {
                        self.view = View::Week;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let theme_label = if self.data.dark_mode { "Light" } else { "Dark" };
                        if ui
                            .button(theme_label)
                            .on_hover_text(if self.data.dark_mode {
                                "Switch to light theme"
                            } else {
                                "Switch to dark theme"
                            })
                            .clicked()
                        {
                            toggle_theme = true;
                        }
                        if !compact && let Some((message, is_error)) = &self.status {
                            let display_message = compact_message(message, 46);
                            let (fill, color) = if *is_error {
                                (DANGER_SOFT, DANGER)
                            } else {
                                (ACCENT_SOFT, ACCENT_DARK)
                            };
                            Frame::new()
                                .fill(fill.into())
                                .corner_radius(99.0)
                                .inner_margin(Margin::symmetric(12, 6))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(display_message).size(12.0).color(color),
                                    );
                                });
                        }
                    });
                });
            });
        if toggle_theme {
            self.data.dark_mode = !self.data.dark_mode;
            configure_style(ctx, self.data.dark_mode);
            self.save(if self.data.dark_mode {
                "Dark theme enabled"
            } else {
                "Light theme enabled"
            });
        }
    }

    fn project_picker(&mut self, ui: &mut egui::Ui) {
        let current = self
            .selected_project
            .map(|id| self.project_name(id).to_owned())
            .unwrap_or_else(|| "Choose a project".into());
        let compact = ui.available_width() < 560.0;
        if compact {
            ui.label(RichText::new("PROJECT").size(11.0).strong().color(MUTED));
            ui.add_space(3.0);
            ui.add_enabled_ui(self.data.active_timer.is_none(), |ui| {
                egui::ComboBox::from_id_salt("project_picker")
                    .selected_text(&current)
                    .width((ui.available_width() - 4.0).clamp(160.0, 360.0))
                    .show_ui(ui, |ui| {
                        for project in &self.data.projects {
                            ui.selectable_value(
                                &mut self.selected_project,
                                Some(project.id),
                                &project.name,
                            );
                        }
                    });
            });
            if ui
                .button(RichText::new("+ New project").color(ACCENT_DARK))
                .clicked()
            {
                self.adding_project = !self.adding_project;
            }
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new("PROJECT").size(11.0).strong().color(MUTED));
                ui.add_enabled_ui(self.data.active_timer.is_none(), |ui| {
                    egui::ComboBox::from_id_salt("project_picker")
                        .selected_text(&current)
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for project in &self.data.projects {
                                ui.selectable_value(
                                    &mut self.selected_project,
                                    Some(project.id),
                                    &project.name,
                                );
                            }
                        });
                });
                if ui
                    .button(RichText::new("+ New project").color(ACCENT_DARK))
                    .clicked()
                {
                    self.adding_project = !self.adding_project;
                }
            });
        }
        if self.adding_project {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.new_project_name)
                        .hint_text("Project name")
                        .desired_width((ui.available_width() - 145.0).clamp(150.0, 260.0)),
                );
                let enter =
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Create project")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(ACCENT)
                        .corner_radius(7.0),
                    )
                    .clicked()
                    || enter
                {
                    self.add_project();
                }
            });
        }
    }

    fn track_view(&mut self, ui: &mut egui::Ui) {
        let compact = ui.available_width() < 560.0;
        ui.set_width(ui.available_width().min(820.0));
        ui.label(RichText::new("Track time").size(30.0).strong().color(INK));
        ui.label(
            RichText::new("Keep the timer focused. We’ll handle the billing math.")
                .size(14.0)
                .color(MUTED),
        );
        self.compact_status(ui);
        ui.add_space(16.0);

        let timer = self.data.active_timer.clone();
        let elapsed = timer
            .as_ref()
            .map(|timer| (Utc::now() - timer.started_at).num_seconds().max(0))
            .unwrap_or(0);
        Frame::new()
            .fill(SURFACE.into())
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(14.0)
            .inner_margin(Margin::same(if compact { 18 } else { 22 }))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.project_picker(ui);
                ui.add_space(14.0);
                ui.label(
                    RichText::new("NOTE · OPTIONAL")
                        .size(11.0)
                        .strong()
                        .color(MUTED),
                );
                let note_response = ui.add_enabled(
                    self.selected_project.is_some(),
                    egui::TextEdit::multiline(&mut self.timer_note)
                        .hint_text("What are you working on?")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if note_response.changed()
                    && let Some(active_timer) = self.data.active_timer.as_mut()
                {
                    active_timer.note = self.timer_note.clone();
                }
                if note_response.lost_focus() && self.data.active_timer.is_some() {
                    self.save("Timer note saved");
                }
                ui.add_space(if compact { 18.0 } else { 22.0 });
                ui.vertical_centered(|ui| {
                    if let Some(timer) = &timer {
                        ui.label(
                            RichText::new(format!(
                                "Tracking {}",
                                self.project_name(timer.project_id)
                            ))
                            .size(13.0)
                            .strong()
                            .color(ACCENT),
                        );
                    } else {
                        ui.label(
                            RichText::new("READY TO TRACK")
                                .size(11.0)
                                .strong()
                                .color(MUTED),
                        );
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format_elapsed(elapsed))
                            .font(FontId::monospace(if compact { 42.0 } else { 56.0 }))
                            .strong()
                            .color(INK),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{:.1} billable hours · rounded to 0.1 h",
                            billed_tenths(elapsed) as f64 / 10.0
                        ))
                        .size(13.0)
                        .color(MUTED),
                    );
                    ui.add_space(16.0);
                    if timer.is_some() {
                        if primary_button(ui, "■  Stop & save", DANGER.into(), true).clicked() {
                            self.stop_timer();
                        }
                    } else if primary_button(
                        ui,
                        "▶  Start timer",
                        ACCENT.into(),
                        self.selected_project.is_some(),
                    )
                    .clicked()
                    {
                        self.start_timer();
                    }
                    if timer.is_some() {
                        ui.add_space(8.0);
                        if ui
                            .button("Compact timer ↗")
                            .on_hover_text(
                                "Show a translucent floating timer. Drag it to any screen.",
                            )
                            .clicked()
                        {
                            self.compact_requested = true;
                            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    }
                    if self.selected_project.is_none() && timer.is_none() {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Create or select a project to begin.")
                                .size(12.0)
                                .color(MUTED),
                        );
                    }
                });
            });

        ui.add_space(16.0);
        let today = Local::now().date_naive();
        let today_entries: Vec<_> = self
            .data
            .entries
            .iter()
            .filter(|entry| entry.started_at.with_timezone(&Local).date_naive() == today)
            .collect();
        let today_tenths: i64 = today_entries.iter().map(|entry| entry.billed_tenths).sum();
        let project_tenths: i64 = today_entries
            .iter()
            .filter(|entry| Some(entry.project_id) == self.selected_project)
            .map(|entry| entry.billed_tenths)
            .sum();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("TODAY · SAVED")
                    .size(11.0)
                    .strong()
                    .color(MUTED),
            );
            ui.label(
                RichText::new(format!(
                    "{:.1} h across all projects",
                    today_tenths as f64 / 10.0
                ))
                .strong()
                .color(INK),
            );
            if self.selected_project.is_some() {
                ui.label(
                    RichText::new(format!(
                        "{:.1} h on this project",
                        project_tenths as f64 / 10.0
                    ))
                    .color(MUTED),
                );
            }
        });
        ui.add_space(16.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Recent entries")
                    .size(20.0)
                    .strong()
                    .color(INK),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(RichText::new("View timesheet  →").color(ACCENT_DARK))
                    .clicked()
                {
                    self.view = View::Week;
                }
            });
        });
        ui.add_space(10.0);
        let entries: Vec<_> = self.data.entries.iter().take(8).cloned().collect();
        if entries.is_empty() {
            Frame::new()
                .fill(SURFACE.into())
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(12.0)
                .inner_margin(Margin::same(24))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Your tracked time will appear here.").color(MUTED));
                    });
                });
        }
        if !entries.is_empty() {
            Frame::new()
                .fill(SURFACE.into())
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(12.0)
                .inner_margin(Margin::symmetric(18, 8))
                .show(ui, |ui| {
                    let compact_rows = ui.available_width() < 470.0;
                    for (index, entry) in entries.iter().enumerate() {
                        ui.add_space(7.0);
                        ui.horizontal(|ui| {
                            ui.set_height(34.0);
                            if !compact_rows {
                                ui.label(RichText::new(entry.local_date()).size(12.0).color(MUTED));
                                ui.add_space(16.0);
                            }
                            ui.label(RichText::new(self.project_name(entry.project_id)).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .small_button("View / edit")
                                    .on_hover_text("Review this entry and its note")
                                    .clicked()
                                {
                                    let entry_date =
                                        entry.started_at.with_timezone(&Local).date_naive();
                                    self.week_start = saturday_of(entry_date);
                                    self.begin_edit_entry(entry.id);
                                    self.view = View::Week;
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "{:.1} h",
                                        entry.billed_tenths as f64 / 10.0
                                    ))
                                    .strong()
                                    .color(INK),
                                );
                            });
                        });
                        if compact_rows {
                            ui.label(RichText::new(entry.local_date()).size(12.0).color(MUTED));
                        }
                        if !entry.note.is_empty() {
                            ui.label(
                                RichText::new(format!("Note: {}", entry.note))
                                    .size(12.0)
                                    .color(MUTED),
                            );
                        }
                        ui.add_space(7.0);
                        if index + 1 < entries.len() {
                            ui.separator();
                        }
                    }
                });
        }
    }

    fn week_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let compact = ui.available_width() < 760.0;
        ui.set_width(ui.available_width().min(1120.0));
        ui.label(
            RichText::new("Weekly timesheet")
                .size(30.0)
                .strong()
                .color(INK),
        );
        ui.label(
            RichText::new("Your work week runs Saturday through Friday.")
                .size(14.0)
                .color(MUTED),
        );
        self.compact_status(ui);
        ui.add_space(22.0);
        let total: i64 = self
            .week_entries()
            .iter()
            .map(|entry| entry.billed_tenths)
            .sum();
        ui.horizontal(|ui| {
            ui.set_height(TOOLBAR_CONTROL_HEIGHT);
            if calendar_nav_button(ui, "‹", "Previous week").clicked() {
                self.week_start = self.week_start - Days::new(7);
            }
            ui.label(
                RichText::new(week_label(self.week_start))
                    .size(19.0)
                    .strong()
                    .color(INK),
            );
            if calendar_nav_button(ui, "›", "Next week").clicked() {
                self.week_start = self.week_start + Days::new(7);
            }
            if secondary_button(ui, "This week").clicked() {
                self.week_start = saturday_of(Local::now().date_naive());
            }
            if !compact {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    week_total_badge(ui, total);
                });
            }
        });
        if compact {
            ui.add_space(8.0);
            week_total_badge(ui, total);
        }
        ui.add_space(16.0);
        ui.label(
            RichText::new("Select any logged-hours value to review the entries behind it.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(6.0);
        Frame::new()
            .fill(SURFACE.into())
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(12.0)
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::horizontal()
                    .id_salt("week_grid_scroll")
                    .show(ui, |ui| self.week_grid(ui));
            });
        ui.add_space(24.0);

        self.week_entry_list(ui);
        ui.add_space(24.0);

        Frame::new()
            .fill(SURFACE.into())
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(12.0)
            .inner_margin(Margin::same(if compact { 16 } else { 20 }))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("Add time manually")
                        .size(17.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    RichText::new("Log work you completed without the timer.")
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().interact_size.y = TOOLBAR_CONTROL_HEIGHT;
                    ui.set_height(TOOLBAR_CONTROL_HEIGHT);
                    self.compact_project_picker(ui);
                    let selected_date = self.week_start + Days::new(self.manual_day as u64);
                    egui::ComboBox::from_id_salt("manual_day")
                        .selected_text(selected_date.format("%a %-m/%-d").to_string())
                        .show_ui(ui, |ui| {
                            for day in 0..7 {
                                let date = self.week_start + Days::new(day);
                                ui.selectable_value(
                                    &mut self.manual_day,
                                    day as usize,
                                    date.format("%a %-m/%-d").to_string(),
                                );
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut self.manual_tenths)
                            .range(1..=240)
                            .speed(1)
                            .custom_formatter(|value, _| format!("{:.1} h", value / 10.0))
                            .custom_parser(|text| {
                                text.trim_end_matches('h')
                                    .trim()
                                    .parse::<f64>()
                                    .ok()
                                    .map(|hours| hours * 10.0)
                            }),
                    );
                });
                ui.add_space(10.0);
                ui.label(
                    RichText::new("NOTE · OPTIONAL")
                        .size(11.0)
                        .strong()
                        .color(MUTED),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.manual_note)
                        .hint_text("Add context for this work entry")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(10.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(self.selected_project.is_some(), action_button("Add entry"))
                        .clicked()
                    {
                        self.add_manual_entry();
                    }
                });
            });

        ui.add_space(16.0);
        ui.horizontal_wrapped(|ui| {
            ui.set_height(TOOLBAR_CONTROL_HEIGHT);
            if secondary_button(ui, "Copy summary").clicked() {
                ctx.copy_text(self.week_summary());
                self.status = Some(("Weekly summary copied".into(), false));
            }
            if secondary_button(ui, "Export CSV").clicked() {
                let name = format!("week-{}.csv", self.week_start.format("%Y-%m-%d"));
                self.status = Some(match self.storage.export_report(&name, &self.week_csv()) {
                    Ok(path) => (format!("Exported to {}", path.display()), false),
                    Err(error) => (error, true),
                });
            }
            ui.label(
                RichText::new("Each entry rounds up to the nearest 0.1 hour.")
                    .size(12.0)
                    .color(MUTED),
            );
        });

        self.entry_picker_window(ctx);
        self.edit_entry_window(ctx);
    }

    fn week_entry_list(&mut self, ui: &mut egui::Ui) {
        let entries: Vec<_> = self.week_entries().into_iter().cloned().collect();
        ui.label(
            RichText::new("Entries this week")
                .size(17.0)
                .strong()
                .color(INK),
        );
        ui.label(
            RichText::new("Review notes or choose an entry to make changes.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(10.0);
        Frame::new()
            .fill(SURFACE.into())
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(12.0)
            .inner_margin(Margin::symmetric(18, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if entries.is_empty() {
                    ui.label(RichText::new("No entries in this week yet.").color(MUTED));
                    return;
                }
                for (index, entry) in entries.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    self.project_name(entry.project_id),
                                    entry.local_date()
                                ))
                                .strong()
                                .color(INK),
                            );
                            let note = RichText::new(if entry.note.is_empty() {
                                "No note".to_owned()
                            } else {
                                entry.note.clone()
                            })
                            .size(12.0)
                            .color(MUTED);
                            ui.label(if entry.note.is_empty() {
                                note.italics()
                            } else {
                                note
                            });
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("View / edit  →").clicked() {
                                self.begin_edit_entry(entry.id);
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{:.1} h",
                                    entry.billed_tenths as f64 / 10.0
                                ))
                                .strong()
                                .color(ACCENT_DARK),
                            );
                        });
                    });
                    if index + 1 < entries.len() {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                    }
                }
            });
    }

    fn compact_project_picker(&mut self, ui: &mut egui::Ui) {
        let current = self
            .selected_project
            .map(|id| self.project_name(id).to_owned())
            .unwrap_or_else(|| "Project".into());
        egui::ComboBox::from_id_salt("manual_project")
            .selected_text(current)
            .width((ui.available_width() - 8.0).clamp(150.0, 220.0))
            .show_ui(ui, |ui| {
                for project in &self.data.projects {
                    ui.selectable_value(
                        &mut self.selected_project,
                        Some(project.id),
                        &project.name,
                    );
                }
            });
    }

    fn compact_status(&self, ui: &mut egui::Ui) {
        if ui.available_width() >= 720.0 {
            return;
        }
        let Some((message, is_error)) = &self.status else {
            return;
        };
        let (fill, color) = if *is_error {
            (DANGER_SOFT, DANGER)
        } else {
            (ACCENT_SOFT, ACCENT_DARK)
        };
        ui.add_space(8.0);
        Frame::new()
            .fill(fill.into())
            .corner_radius(8.0)
            .inner_margin(Margin::symmetric(10, 7))
            .show(ui, |ui| {
                ui.label(RichText::new(message).size(12.0).color(color));
            });
    }

    fn week_grid(&mut self, ui: &mut egui::Ui) {
        let totals = self.week_totals();
        let mut entry_ids: BTreeMap<(Uuid, u64), Vec<Uuid>> = BTreeMap::new();
        for entry in self.week_entries() {
            let date = entry.started_at.with_timezone(&Local).date_naive();
            let day = (date - self.week_start).num_days() as u64;
            entry_ids
                .entry((entry.project_id, day))
                .or_default()
                .push(entry.id);
        }
        let active_projects: Vec<_> = self
            .data
            .projects
            .iter()
            .filter(|project| {
                (0..7).any(|day| totals.get(&(project.id, day)).copied().unwrap_or(0) > 0)
            })
            .map(|project| (project.id, project.name.clone()))
            .collect();
        let mut selected_cell = None;
        egui::Grid::new("week_grid")
            .striped(false)
            .min_col_width(68.0)
            .spacing([16.0, 13.0])
            .show(ui, |ui| {
                ui.label(RichText::new("PROJECT").size(11.0).strong().color(MUTED));
                for day in 0..7 {
                    let date = self.week_start + Days::new(day);
                    let is_today = date == Local::now().date_naive();
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(date.format("%a").to_string().to_uppercase())
                                .size(10.0)
                                .strong()
                                .color(if is_today { ACCENT } else { MUTED }),
                        );
                        ui.label(
                            RichText::new(date.format("%-m/%-d").to_string())
                                .strong()
                                .color(if is_today { ACCENT_DARK } else { INK }),
                        );
                    });
                }
                ui.label(RichText::new("TOTAL").size(11.0).strong().color(MUTED));
                ui.end_row();
                ui.separator();
                for _ in 0..8 {
                    ui.separator();
                }
                ui.end_row();

                if active_projects.is_empty() {
                    ui.label(RichText::new("No time logged").color(MUTED));
                    for _ in 0..8 {
                        ui.label(RichText::new("—").color(LINE));
                    }
                    ui.end_row();
                }
                for (project_id, project_name) in &active_projects {
                    ui.label(RichText::new(project_name).strong().color(INK));
                    let mut row_total = 0;
                    for day in 0..7 {
                        let tenths = *totals.get(&(*project_id, day)).unwrap_or(&0);
                        row_total += tenths;
                        if tenths == 0 {
                            ui.label(RichText::new("—").color(LINE));
                        } else {
                            let count = entry_ids.get(&(*project_id, day)).map_or(0, Vec::len);
                            let response = ui
                                .small_button(
                                    RichText::new(format!("{:.1}", tenths as f64 / 10.0))
                                        .color(ACCENT_DARK),
                                )
                                .on_hover_text(if count == 1 {
                                    "Edit this entry".to_owned()
                                } else {
                                    format!("Choose one of {count} entries to edit")
                                });
                            if response.clicked() {
                                selected_cell = Some((*project_id, day));
                            }
                        }
                    }
                    ui.label(
                        RichText::new(format!("{:.1}", row_total as f64 / 10.0))
                            .strong()
                            .color(ACCENT_DARK),
                    );
                    ui.end_row();
                }
                ui.separator();
                for _ in 0..8 {
                    ui.separator();
                }
                ui.end_row();
                ui.label(RichText::new("TOTAL").size(11.0).strong().color(MUTED));
                let mut grand_total = 0;
                for day in 0..7 {
                    let day_total: i64 = self
                        .data
                        .projects
                        .iter()
                        .map(|project| totals.get(&(project.id, day)).copied().unwrap_or(0))
                        .sum();
                    grand_total += day_total;
                    ui.label(
                        RichText::new(format!("{:.1}", day_total as f64 / 10.0))
                            .strong()
                            .color(INK),
                    );
                }
                ui.label(
                    RichText::new(format!("{:.1}", grand_total as f64 / 10.0))
                        .strong()
                        .color(ACCENT),
                );
                ui.end_row();
            });

        if let Some(cell) = selected_cell {
            let ids = entry_ids.get(&cell).cloned().unwrap_or_default();
            if ids.len() == 1 {
                self.begin_edit_entry(ids[0]);
            } else if !ids.is_empty() {
                self.entry_picker_cell = Some(cell);
            }
        }
    }

    fn entry_picker_window(&mut self, ctx: &egui::Context) {
        let Some((project_id, day)) = self.entry_picker_cell else {
            return;
        };
        let date = self.week_start + Days::new(day);
        let entries: Vec<_> = self
            .data
            .entries
            .iter()
            .filter(|entry| {
                entry.project_id == project_id
                    && entry.started_at.with_timezone(&Local).date_naive() == date
            })
            .cloned()
            .collect();
        let mut open = true;
        let mut selected = None;
        egui::Window::new("Choose an entry to edit")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        self.project_name(project_id),
                        date.format("%A, %b %-d")
                    ))
                    .color(MUTED),
                );
                ui.add_space(8.0);
                for entry in &entries {
                    Frame::new()
                        .fill(SURFACE_MUTED.into())
                        .corner_radius(8.0)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{:.1} billable hours",
                                            entry.billed_tenths as f64 / 10.0
                                        ))
                                        .strong(),
                                    );
                                    ui.label(
                                        RichText::new(if entry.note.is_empty() {
                                            "No note".to_owned()
                                        } else {
                                            entry.note.clone()
                                        })
                                        .size(12.0)
                                        .color(MUTED),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("View / edit").clicked() {
                                        selected = Some(entry.id);
                                    }
                                });
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        if let Some(entry_id) = selected {
            self.begin_edit_entry(entry_id);
        } else if !open {
            self.entry_picker_cell = None;
        }
    }

    fn edit_entry_window(&mut self, ctx: &egui::Context) {
        if self.editing_entry.is_none() {
            return;
        }
        let mut open = true;
        let mut save_requested =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter));
        let mut cancel_requested = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        egui::Window::new("Review & edit entry")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.set_max_width(460.0);
                ui.label(
                    RichText::new("Update the details below, then save your changes.")
                        .size(13.0)
                        .color(MUTED),
                );
                ui.add_space(14.0);
                Frame::new()
                    .fill(SURFACE_MUTED.into())
                    .corner_radius(9.0)
                    .inner_margin(Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("PROJECT").size(11.0).strong().color(MUTED));
                        let project_name = self
                            .edit_project
                            .map(|id| self.project_name(id).to_owned())
                            .unwrap_or_else(|| "Choose a project".into());
                        egui::ComboBox::from_id_salt("edit_entry_project")
                            .selected_text(project_name)
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for project in &self.data.projects {
                                    ui.selectable_value(
                                        &mut self.edit_project,
                                        Some(project.id),
                                        &project.name,
                                    );
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(RichText::new("DAY").size(11.0).strong().color(MUTED));
                        let selected_date = self.edit_week_start + Days::new(self.edit_day as u64);
                        egui::ComboBox::from_id_salt("edit_entry_day")
                            .selected_text(selected_date.format("%A, %b %-d").to_string())
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for day in 0..7 {
                                    let date = self.edit_week_start + Days::new(day);
                                    ui.selectable_value(
                                        &mut self.edit_day,
                                        day as usize,
                                        date.format("%A, %b %-d").to_string(),
                                    );
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("BILLABLE HOURS")
                                .size(11.0)
                                .strong()
                                .color(MUTED),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.edit_tenths)
                                .range(1..=240)
                                .speed(1)
                                .custom_formatter(|value, _| format!("{:.1} h", value / 10.0))
                                .custom_parser(|text| {
                                    text.trim_end_matches('h')
                                        .trim()
                                        .parse::<f64>()
                                        .ok()
                                        .map(|hours| hours * 10.0)
                                }),
                        );
                        ui.label(
                            RichText::new("Use 0.1-hour increments (6 minutes).")
                                .size(11.0)
                                .color(MUTED),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("NOTE · OPTIONAL")
                                .size(11.0)
                                .strong()
                                .color(MUTED),
                        );
                        ui.add(
                            egui::TextEdit::multiline(&mut self.edit_note)
                                .hint_text("Add context about the work completed")
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        );
                    });
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, "Cancel").clicked() {
                        cancel_requested = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(self.edit_project.is_some(), action_button("Save changes"))
                            .on_hover_text("Save changes (⌘/Ctrl + Enter)")
                            .clicked()
                        {
                            save_requested = true;
                        }
                    });
                });
                ui.label(
                    RichText::new("Esc cancels · ⌘/Ctrl + Enter saves")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        if save_requested {
            self.save_entry_edit();
        } else if cancel_requested || !open {
            self.editing_entry = None;
        }
    }

    fn week_entries(&self) -> Vec<&TimeEntry> {
        let end = self.week_start + Days::new(7);
        self.data
            .entries
            .iter()
            .filter(|entry| {
                let date = entry.started_at.with_timezone(&Local).date_naive();
                date >= self.week_start && date < end
            })
            .collect()
    }

    fn week_totals(&self) -> BTreeMap<(Uuid, u64), i64> {
        let mut totals = BTreeMap::new();
        for entry in self.week_entries() {
            let date = entry.started_at.with_timezone(&Local).date_naive();
            let day = (date - self.week_start).num_days() as u64;
            *totals.entry((entry.project_id, day)).or_default() += entry.billed_tenths;
        }
        totals
    }

    fn week_summary(&self) -> String {
        let totals = self.week_totals();
        let mut lines = vec![format!("Week of {}", self.week_start.format("%B %-d, %Y"))];
        let mut grand_total = 0;
        for project in &self.data.projects {
            let total: i64 = (0..7)
                .map(|day| totals.get(&(project.id, day)).copied().unwrap_or(0))
                .sum();
            if total > 0 {
                lines.push(format!(
                    "{}: {:.1} hours",
                    project.name,
                    total as f64 / 10.0
                ));
                grand_total += total;
            }
        }
        lines.push(format!("Total: {:.1} hours", grand_total as f64 / 10.0));
        let notes: Vec<_> = self
            .week_entries()
            .into_iter()
            .filter(|entry| !entry.note.is_empty())
            .collect();
        if !notes.is_empty() {
            lines.push(String::new());
            lines.push("Notes:".into());
            for entry in notes {
                lines.push(format!(
                    "{} · {} · {:.1} h — {}",
                    entry.started_at.with_timezone(&Local).format("%a %-m/%-d"),
                    self.project_name(entry.project_id),
                    entry.billed_tenths as f64 / 10.0,
                    entry.note
                ));
            }
        }
        lines.join("\n")
    }

    fn week_csv(&self) -> String {
        let totals = self.week_totals();
        let mut csv = String::from(
            "Project,Saturday,Sunday,Monday,Tuesday,Wednesday,Thursday,Friday,Total\n",
        );
        for project in &self.data.projects {
            let values: Vec<i64> = (0..7)
                .map(|day| totals.get(&(project.id, day)).copied().unwrap_or(0))
                .collect();
            let total: i64 = values.iter().sum();
            if total == 0 {
                continue;
            }
            csv.push_str(&csv_cell(&project.name));
            for value in values {
                csv.push_str(&format!(",{:.1}", value as f64 / 10.0));
            }
            csv.push_str(&format!(",{:.1}\n", total as f64 / 10.0));
        }
        let noted_entries: Vec<_> = self
            .week_entries()
            .into_iter()
            .filter(|entry| !entry.note.is_empty())
            .collect();
        if !noted_entries.is_empty() {
            csv.push_str("\nEntry notes\nProject,Date,Hours,Note\n");
            for entry in noted_entries {
                csv.push_str(&format!(
                    "{},{},{:.1},{}\n",
                    csv_cell(self.project_name(entry.project_id)),
                    entry.started_at.with_timezone(&Local).format("%Y-%m-%d"),
                    entry.billed_tenths as f64 / 10.0,
                    csv_cell(&entry.note)
                ));
            }
        }
        csv
    }
}

impl eframe::App for HourTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.data.active_timer.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        #[cfg(target_os = "macos")]
        self.macos_menu_bar(ctx);
        self.minimized_reminder(ctx);
        self.top_bar(ctx);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG.into()).inner_margin(Margin::same(18)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| match self.view {
                            View::Track => self.track_view(ui),
                            View::Week => self.week_view(ui, ctx),
                        });
                    });
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}

// Use the known window rectangle rather than a monitor size without an origin.
fn reminder_position(window: Option<egui::Rect>, size: Vec2, gap: f32) -> egui::Pos2 {
    let window = window.unwrap_or_else(|| egui::Rect::from_min_size(egui::pos2(40.0, 40.0), size));
    egui::pos2(
        (window.right() - size.x - gap).max(window.left()),
        window.top() + gap,
    )
}

fn days_from_saturday(date: NaiveDate) -> u32 {
    (date.weekday().num_days_from_monday() + 2) % 7
}

fn saturday_of(date: NaiveDate) -> NaiveDate {
    date - Days::new(days_from_saturday(date) as u64)
}

fn week_label(start: NaiveDate) -> String {
    let end = start + Days::new(6);
    if start.month() == end.month() {
        format!(
            "{} {}–{}, {}",
            start.format("%b"),
            start.day(),
            end.day(),
            start.year()
        )
    } else {
        format!("{} – {}", start.format("%b %-d"), end.format("%b %-d, %Y"))
    }
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn compact_message(message: &str, max_chars: usize) -> String {
    let mut chars = message.chars();
    let shortened: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

/// Draws Tenth's ten-point dial. The highlighted point and hand represent one
/// billable tenth, while the open center keeps the mark legible at toolbar size.
fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let radius = size * 0.47;
    painter.circle_filled(center, radius, INK);

    let tick_radius = size * 0.35;
    for tick in 0..10 {
        let angle = -std::f32::consts::FRAC_PI_2 + tick as f32 * std::f32::consts::TAU / 10.0;
        let position = center + egui::vec2(angle.cos(), angle.sin()) * tick_radius;
        painter.circle_filled(
            position,
            (size * 0.045).max(1.0),
            if tick == 1 { SIGNAL } else { ACCENT_SOFT },
        );
    }

    let hand_angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU / 10.0;
    let hand_end = center + egui::vec2(hand_angle.cos(), hand_angle.sin()) * size * 0.25;
    painter.line_segment(
        [center, hand_end],
        Stroke::new((size * 0.065).max(1.2), SIGNAL),
    );
    painter.circle_filled(center, size * 0.07, SURFACE);
}

fn tab(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(if selected {
            ACCENT_DARK
        } else {
            MUTED
        }))
        .fill(if selected {
            ACCENT_SOFT.into()
        } else {
            Color32::TRANSPARENT
        })
        .stroke(Stroke::NONE)
        .corner_radius(8.0),
    )
}

fn primary_button(ui: &mut egui::Ui, label: &str, fill: Color32, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .size(15.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(fill)
        .corner_radius(8.0)
        .min_size(egui::vec2(190.0, 48.0)),
    )
}

fn calendar_nav_button(ui: &mut egui::Ui, label: &str, hint: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(20.0).color(INK))
            .corner_radius(7.0)
            .min_size(Vec2::splat(TOOLBAR_CONTROL_HEIGHT)),
    )
    .on_hover_text(hint)
}

fn secondary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(INK))
            .corner_radius(7.0)
            .min_size(egui::vec2(0.0, TOOLBAR_CONTROL_HEIGHT)),
    )
}

fn action_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
        .fill(ACCENT)
        .corner_radius(7.0)
        .min_size(egui::vec2(0.0, TOOLBAR_CONTROL_HEIGHT))
}

fn week_total_badge(ui: &mut egui::Ui, total: i64) {
    Frame::new()
        .fill(ACCENT_SOFT.into())
        .corner_radius(9.0)
        .inner_margin(Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{:.1} hours total", total as f64 / 10.0))
                    .size(16.0)
                    .strong()
                    .color(ACCENT_DARK),
            );
        });
}

fn configure_style(ctx: &egui::Context, dark_mode: bool) {
    DARK_MODE.store(dark_mode, Ordering::Relaxed);
    let mut style = (*ctx.style()).clone();
    style.visuals = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(13.0, 8.0);
    style.spacing.combo_width = 180.0;
    style.visuals.panel_fill = BG.into();
    style.visuals.window_fill = SURFACE.into();
    style.visuals.extreme_bg_color = SURFACE_MUTED.into();
    let inactive = if dark_mode {
        Color32::from_rgb(42, 59, 61)
    } else {
        Color32::from_rgb(235, 240, 236)
    };
    let hovered = if dark_mode {
        Color32::from_rgb(51, 72, 71)
    } else {
        Color32::from_rgb(226, 236, 230)
    };
    style.visuals.widgets.inactive.bg_fill = inactive;
    style.visuals.widgets.inactive.weak_bg_fill = inactive;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
    style.visuals.widgets.hovered.bg_fill = hovered;
    style.visuals.widgets.hovered.weak_bg_fill = hovered;
    style.visuals.widgets.active.bg_fill = ACCENT_SOFT.into();
    style.visuals.widgets.active.weak_bg_fill = ACCENT_SOFT.into();
    style.visuals.selection.bg_fill = ACCENT.into();
    style.visuals.window_corner_radius = 12.0.into();
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_saturday_for_any_date() {
        let wednesday = NaiveDate::from_ymd_opt(2026, 8, 26).unwrap();
        assert_eq!(
            saturday_of(wednesday),
            NaiveDate::from_ymd_opt(2026, 8, 22).unwrap()
        );
        let saturday = NaiveDate::from_ymd_opt(2026, 8, 29).unwrap();
        assert_eq!(saturday_of(saturday), saturday);
        let friday = NaiveDate::from_ymd_opt(2026, 8, 28).unwrap();
        assert_eq!(
            saturday_of(friday),
            NaiveDate::from_ymd_opt(2026, 8, 22).unwrap()
        );
    }

    #[test]
    fn csv_cells_escape_quotes() {
        assert_eq!(csv_cell("Client \"A\""), "\"Client \"\"A\"\"\"");
    }

    #[test]
    fn long_status_messages_are_compacted_without_breaking_unicode() {
        assert_eq!(compact_message("Saved", 8), "Saved");
        assert_eq!(compact_message("Exported résumé", 8), "Exported…");
    }

    #[test]
    fn reminder_uses_the_window_display_including_negative_origins() {
        for origin in [
            egui::pos2(1920.0, 100.0),
            egui::pos2(-1600.0, -900.0),
            egui::pos2(100.0, 80.0),
        ] {
            let window = egui::Rect::from_min_size(origin, Vec2::new(980.0, 680.0));
            let position = reminder_position(Some(window), REMINDER_SIZE, REMINDER_EDGE_GAP);
            assert!(window.contains_rect(egui::Rect::from_min_size(position, REMINDER_SIZE)));
            assert_eq!(position.y, origin.y + REMINDER_EDGE_GAP);
        }
    }
}
