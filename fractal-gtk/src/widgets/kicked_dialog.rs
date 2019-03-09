use gtk;
use gtk::prelude::*;

struct Widgets {
    msg_kicked_window: gtk::MessageDialog,
    confirm_kicked_button: gtk::Button,
}

impl Widgets {
    pub fn new() -> Widgets {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/kicked_room.ui")
            .expect("Can't load ui file: kicked_room.ui OH NO");

        let msg_kicked_window: gtk::MessageDialog = builder
            .get_object("kicked_room_dialog")
            .expect("Can't find kicked_room_dialog in ui file.");

        let confirm_kicked_button: gtk::Button = builder
            .get_object("kicked_room_confirm")
            .expect("Can't find kicked_room_confirm in ui file.");

        Widgets {
            msg_kicked_window,
            confirm_kicked_button,
        }
    }
}

pub struct KickedDialog {
    widgets: Widgets,
}

impl KickedDialog {
    pub fn new() -> KickedDialog {
        let viewer = KickedDialog {
            widgets: Widgets::new(),
        };
        viewer.connect();
        viewer
    }

    pub fn show(&self, room_name: &str, reason: &str, kicker: &str) {
        let text = format!("You have been kicked from {}", room_name);
        self.widgets
            .msg_kicked_window
            .set_property_text(Some(&text));
        let secondary_text = format!("Kicked by: {}\n \"{}\"", kicker, reason);
        self.widgets
            .msg_kicked_window
            .set_property_secondary_text(Some(&secondary_text));

        self.widgets.msg_kicked_window.show();
    }

    /* This sets the transient_for parent */
    pub fn set_parent_window(&self, parent: &gtk::Window) {
        self.widgets.msg_kicked_window.set_transient_for(parent);
    }

    fn connect(&self) {
        let msg_kicked_window = self.widgets.msg_kicked_window.downgrade();
        self.widgets
            .confirm_kicked_button
            .connect_clicked(move |_| {
                upgrade_weak!(msg_kicked_window).close();
            });

        /* Close the window when the user preses ESC */
        self.widgets
            .msg_kicked_window
            .connect_key_press_event(|w, k| {
                if k.get_keyval() == gdk::enums::key::Escape {
                    w.close();
                }

                Inhibit(true)
            });
    }
}
