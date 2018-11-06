use gtk;
use gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_login_view(&self) {
        let advbtn: gtk::Button = self.ui.builder
            .get_object("login_advanced_button")
            .expect("Couldn't find login_advanced_button in ui file.");
        let adv: gtk::Revealer = self.ui.builder
            .get_object("login_advanced")
            .expect("Couldn't find login_advanced in ui file.");
        advbtn.connect_clicked(move |_| {
            adv.set_reveal_child(!adv.get_child_revealed());
        });

        self.connect_login_button();
        self.set_login_focus_chain();
    }
    pub fn set_login_focus_chain(&self) {
        let focus_chain = [
            "login_username",
            "login_password",
            "login_button",
            "login_advanced_button",
            "login_server",
            "login_idp",
        ];

        let mut v: Vec<gtk::Widget> = vec![];
        for i in focus_chain.iter() {
            let w = self.ui.builder.get_object(i).expect("Couldn't find widget");
            v.push(w);
        }

        let grid: gtk::Grid = self.ui.builder
            .get_object("login_grid")
            .expect("Couldn't find login_grid widget");
        grid.set_focus_chain(&v);
    }

    pub fn connect_login_button(&self) {
        // Login click
        let btn: gtk::Button = self.ui.builder
            .get_object("login_button")
            .expect("Couldn't find login_button in ui file.");
        let username: gtk::Entry = self.ui.builder
            .get_object("login_username")
            .expect("Couldn't find login_username in ui file.");
        let password: gtk::Entry = self.ui.builder
            .get_object("login_password")
            .expect("Couldn't find login_password in ui file.");

        let op = self.op.clone();
        btn.connect_clicked(move |_| op.lock().unwrap().login());
        let op = self.op.clone();
        username.connect_activate(move |_| op.lock().unwrap().login());
        let op = self.op.clone();
        password.connect_activate(move |_| op.lock().unwrap().login());

        self.ui.builder
            .get_object::<gtk::Label>("login_error_msg")
            .expect("Can't find login_error_msg in ui file.").hide();
    }
}
