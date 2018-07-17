extern crate gtk;
use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_search(&self) {
        let input: gtk::Entry = self.ui.builder
            .get_object("search_input")
            .expect("Couldn't find search_input in ui file.");

        let btn: gtk::Button = self.ui.builder
            .get_object("search")
            .expect("Couldn't find search in ui file.");

        let op = self.op.clone();
        input.connect_activate(move |inp| op.lock().unwrap().search(inp.get_text()));
        let op = self.op.clone();
        btn.connect_clicked(move |_| op.lock().unwrap().search(input.get_text()));
    }
}
