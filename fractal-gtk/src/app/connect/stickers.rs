use gtk;
use gtk::prelude::*;

use app::App;

impl App {
    #[allow(dead_code)]
    pub fn connect_stickers(&self) {
        let popover_btn: gtk::MenuButton = self.ui.builder
            .get_object("stickers_button")
            .expect("Couldn't find stickers_button in ui file.");

        let popover: gtk::Popover = self.ui.builder
            .get_object("stickers_popover")
            .expect("Couldn't find stickers_popover in ui file.");

        popover_btn.set_popover(Some(&popover));
        let op = self.op.clone();
        popover_btn.connect_clicked(move |_| {
            // redrawing the stickers
            op.lock().unwrap().stickers_draw();
        });
    }
}
