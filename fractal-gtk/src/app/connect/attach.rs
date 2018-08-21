extern crate gtk;
use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_attach(&self) {
        let op = self.op.clone();
        self.ui.sventry.attach.connect_clicked(move |_| {
            op.lock().unwrap().attach_file();
        });
    }
}
