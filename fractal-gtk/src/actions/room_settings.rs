use fractal_api::backend::room;
use fractal_api::identifiers::RoomId;
use fractal_api::r0::AccessToken;
use fractal_api::url::Url;
use gio::prelude::*;
use gio::SimpleAction;
use gio::SimpleActionGroup;
use std::convert::TryFrom;
use std::thread;

use crate::app::dispatch_error;
use crate::app::App;
use crate::backend::BKResponse;
use crate::i18n::i18n;

use crate::widgets::ErrorDialog;
use crate::widgets::FileDialog::open;

use crate::actions::ButtonState;

// This creates all actions a user can perform in the room settings
pub fn new(
    window: &gtk::Window,
    server_url: Url,
    access_token: AccessToken,
) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    // TODO create two stats loading interaction and connect it to the avatar box
    let change_avatar = SimpleAction::new_stateful(
        "change-avatar",
        glib::VariantTy::new("s").ok(),
        &ButtonState::Sensitive.into(),
    );

    actions.add_action(&change_avatar);

    let window_weak = window.downgrade();
    change_avatar.connect_activate(move |a, data| {
        if let Some(room_id) = data
            .and_then(|x| x.get_str())
            .and_then(|rid| RoomId::try_from(rid).ok())
        {
            let window = upgrade_weak!(window_weak);
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(i18n("Images").as_str()));
            filter.add_mime_type("image/*");
            if let Some(path) = open(&window, i18n("Select a new avatar").as_str(), &[filter]) {
                if let Some(file) = path.to_str().map(Into::into) {
                    a.change_state(&ButtonState::Insensitive.into());
                    let server = server_url.clone();
                    let access_token = access_token.clone();
                    thread::spawn(move || {
                        match room::set_room_avatar(server, access_token, room_id, file) {
                            Ok(_) => {
                                APPOP!(show_new_room_avatar);
                            }
                            Err(err) => {
                                dispatch_error(BKResponse::SetRoomAvatarError(err));
                            }
                        }
                    });
                } else {
                    ErrorDialog::new(false, &i18n("Couldn’t open file"));
                }
            }
        }
    });

    actions
}
