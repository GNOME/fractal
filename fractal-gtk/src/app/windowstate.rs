use gio::SettingsExt;
use gtk;
use gtk::prelude::*;

pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub is_maximized: bool,
}

impl WindowState {
    pub fn load_from_gsettings(settings: &gio::Settings) -> WindowState {
        let x = settings.get_int("main-window-state-x");
        let y = settings.get_int("main-window-state-y");
        let width = settings.get_int("main-window-state-width");
        let height = settings.get_int("main-window-state-height");
        let is_maximized = settings.get_boolean("main-window-state-maximized");

        WindowState {
            x,
            y,
            width,
            height,
            is_maximized,
        }
    }

    pub fn from_window(window: &gtk::ApplicationWindow) -> WindowState {
        let position = window.get_position();
        let size = window.get_size();
        let x = position.0;
        let y = position.1;
        let width = size.0;
        let height = size.1;
        let is_maximized = window.is_maximized();

        WindowState {
            x,
            y,
            width,
            height,
            is_maximized,
        }
    }

    pub fn save_in_gsettings(&self, settings: &gio::Settings) {
        let _ = settings.set_int("main-window-state-x", self.x);
        let _ = settings.set_int("main-window-state-y", self.y);
        let _ = settings.set_int("main-window-state-width", self.width);
        let _ = settings.set_int("main-window-state-height", self.height);
        let _ = settings.set_boolean("main-window-state-maximized", self.is_maximized);
    }
}
