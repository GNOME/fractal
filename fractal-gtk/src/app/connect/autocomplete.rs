use gtk::prelude::*;

use crate::widgets;

use crate::app::App;

impl App {
    pub fn connect_autocomplete(&self) {
        let popover = self
            .ui
            .builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");
        let listbox = self
            .ui
            .builder
            .get_object::<gtk::ListBox>("autocomplete_listbox")
            .expect("Can't find autocomplete_listbox in ui file.");
        let window: gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let op = self.op.clone();
        widgets::Autocomplete::new(op, window, self.ui.sventry.view.clone(), popover, listbox)
            .connect();
    }
}
