extern crate gtk;
extern crate sourceview;

use widgets;

use app::App;

impl App {
    pub fn connect_autocomplete(&self) {
        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");
        let popover = self.ui.builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");
        let listbox = self.ui.builder
            .get_object::<gtk::ListBox>("autocomplete_listbox")
            .expect("Can't find autocomplete_listbox in ui file.");
        let window: gtk::Window = self.ui.builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let op = self.op.clone();
        widgets::Autocomplete::new(op, window, msg_entry, popover, listbox).connect();
    }
}
