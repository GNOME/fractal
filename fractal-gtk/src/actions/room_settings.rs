use gio::prelude::*;
use gio::SimpleAction;
use gio::SimpleActionGroup;
use glib;
use gtk;
use gtk::prelude::*;
use std::sync::mpsc::Sender;

use crate::backend::BKCommand;
use crate::i18n::i18n;

use crate::widgets::ErrorDialog;
use crate::widgets::FileDialog::open;

use crate::actions::ButtonState;

// This creates all actions a user can perform in the room settings
pub fn new(window: &gtk::Window, backend: &Sender<BKCommand>) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    // TODO create two stats loading interaction and conect it to the avatar box
    let change_avatar = SimpleAction::new_stateful(
        "change-avatar",
        glib::VariantTy::new("s").ok(),
        &ButtonState::Sensitive.into(),
    );

    actions.add_action(&change_avatar);

    let window_weak = window.downgrade();
    let backend = backend.clone();
    change_avatar.connect_activate(move |a, data| {
        if let Some(id) = data.as_ref().map(|x| x.to_string()) {
            let window = upgrade_weak!(window_weak);
            let filter = gtk::FileFilter::new();
            FileFilterExt::set_name(&filter, Some(i18n("Images").as_str()));
            filter.add_mime_type("image/*");
            if let Some(path) = open(&window, i18n("Select a new avatar").as_str(), &[filter]) {
                if let Some(file) = path.to_str() {
                    a.change_state(&ButtonState::Insensitive.into());
                    let _ = backend.send(BKCommand::SetRoomAvatar(id, file.to_string()));
                } else {
                    ErrorDialog::new(false, &i18n("Couldn't open file"));
                }
            }
        }
    });

    actions
}
