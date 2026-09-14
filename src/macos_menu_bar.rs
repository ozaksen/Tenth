use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

pub enum MenuBarAction {
    Start,
    Stop,
    Open,
    Quit,
}

pub struct MacMenuBar {
    tray_icon: TrayIcon,
    status_item: MenuItem,
    billable_item: MenuItem,
    start_item: MenuItem,
    stop_item: MenuItem,
    open_item: MenuItem,
    quit_item: MenuItem,
    events: Receiver<MenuEvent>,
}

impl MacMenuBar {
    pub fn new(ctx: &egui::Context) -> Result<Self, String> {
        let status_item = MenuItem::new("No timer running", false, None);
        let billable_item = MenuItem::new("Select a project in Tenth to begin", false, None);
        let start_item = MenuItem::new("Start timer", false, None);
        let stop_item = MenuItem::new("Stop & save", false, None);
        let open_item = MenuItem::new("Open Tenth", true, None);
        let quit_item = MenuItem::new("Quit Tenth", true, None);
        let first_separator = PredefinedMenuItem::separator();
        let second_separator = PredefinedMenuItem::separator();
        let menu = Menu::with_items(&[
            &status_item,
            &billable_item,
            &first_separator,
            &start_item,
            &stop_item,
            &second_separator,
            &open_item,
            &quit_item,
        ])
        .map_err(|error| format!("Could not create the macOS menu: {error}"))?;

        let tray_icon = TrayIconBuilder::new()
            .with_title("Tenth")
            .with_tooltip("Tenth time tracker")
            .with_menu(Box::new(menu))
            .build()
            .map_err(|error| format!("Could not add Tenth to the menu bar: {error}"))?;

        let (sender, events) = mpsc::channel();
        let repaint_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = sender.send(event);
            repaint_ctx.request_repaint();
        }));

        Ok(Self {
            tray_icon,
            status_item,
            billable_item,
            start_item,
            stop_item,
            open_item,
            quit_item,
            events,
        })
    }

    pub fn update(&self, project_name: Option<&str>, active_timer: Option<(&str, i64, i64)>) {
        if let Some((project, elapsed_seconds, billable_tenths)) = active_timer {
            let elapsed = crate::model::format_elapsed(elapsed_seconds);
            self.tray_icon.set_title(Some(format!("● {elapsed}")));
            self.status_item
                .set_text(format!("Tracking {project} · {elapsed}"));
            self.billable_item
                .set_text(format!("{:.1} h billable", billable_tenths as f64 / 10.0));
            self.start_item.set_text("Timer is running");
            self.start_item.set_enabled(false);
            self.stop_item.set_enabled(true);
        } else {
            self.tray_icon.set_title(Some("Tenth"));
            self.status_item.set_text("No timer running");
            self.billable_item.set_text("Ready to track");
            self.start_item.set_text(match project_name {
                Some(project) => format!("Start {project}"),
                None => "Start timer".to_owned(),
            });
            self.start_item.set_enabled(project_name.is_some());
            self.stop_item.set_enabled(false);
        }
    }

    pub fn next_action(&self) -> Option<MenuBarAction> {
        while let Ok(event) = self.events.try_recv() {
            let id = event.id;
            if id == *self.start_item.id() {
                return Some(MenuBarAction::Start);
            }
            if id == *self.stop_item.id() {
                return Some(MenuBarAction::Stop);
            }
            if id == *self.open_item.id() {
                return Some(MenuBarAction::Open);
            }
            if id == *self.quit_item.id() {
                return Some(MenuBarAction::Quit);
            }
        }
        None
    }
}

impl Drop for MacMenuBar {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}
