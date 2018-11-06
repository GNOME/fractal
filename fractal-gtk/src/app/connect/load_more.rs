use gtk;
use gtk::prelude::*;

use app::App;

impl App {
    pub fn create_load_more_spn(&self) {
        let messages = self.op.lock().unwrap().message_box.clone();

        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        let btn = self.op.lock().unwrap().load_more_spn.clone();
        btn.set_halign(gtk::Align::Center);
        btn.set_margin_top (12);
        btn.set_margin_bottom (12);
        btn.show();
        row.add(&btn);
        row.show();
        messages.add(&row);
    }
}
