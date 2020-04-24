use crate::i18n::i18n;
use fractal_api::backend::user;
use fractal_api::identifiers::UserId;
use fractal_api::r0::AccessToken;
use fractal_api::url::Url;
use fractal_api::util::ResultExpectLog;
use gio::prelude::*;
use gio::SimpleAction;
use gio::SimpleActionGroup;
use gtk;
use std::sync::mpsc::Sender;
use std::thread;

use crate::app::App;
use crate::backend::{BKCommand, BKResponse};

use crate::widgets::FileDialog::open;

use crate::actions::ButtonState;

// This creates all actions a user can perform in the account settings
pub fn new(
    window: &gtk::Window,
    tx: Sender<BKCommand>,
    server_url: Url,
    access_token: AccessToken,
    uid: UserId,
) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    // TODO create two stats loading interaction and connect it to the avatar box
    let change_avatar =
        SimpleAction::new_stateful("change-avatar", None, &ButtonState::Sensitive.into());

    actions.add_action(&change_avatar);

    let window_weak = window.downgrade();
    change_avatar.connect_activate(move |a, _| {
        let window = upgrade_weak!(window_weak);
        let filter = gtk::FileFilter::new();
        filter.add_mime_type("image/*");
        filter.set_name(Some(i18n("Images").as_str()));
        if let Some(path) = open(&window, i18n("Select a new avatar").as_str(), &[filter]) {
            a.change_state(&ButtonState::Insensitive.into());
            let tx = tx.clone();
            let server_url = server_url.clone();
            let access_token = access_token.clone();
            let uid = uid.clone();
            thread::spawn(move || {
                match user::set_user_avatar(server_url, access_token, uid, path) {
                    Ok(path) => {
                        APPOP!(show_new_avatar, (path));
                    }
                    Err(err) => {
                        tx.send(BKCommand::SendBKResponse(BKResponse::SetUserAvatarError(
                            err,
                        )))
                        .expect_log("Connection closed");
                    }
                }
            });
        }
    });

    actions
}
