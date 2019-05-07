use gio::SimpleAction;
use glib::Cast;
use glib::ObjectExt;
use glib::ToVariant;
use gtk::WidgetExt;

pub mod account_settings;
pub mod global;
pub mod login;
pub mod room_history;
pub mod room_settings;

pub use self::account_settings as AccountSettings;
pub use self::global as Global;
pub use self::global::AppState;
pub use self::login as Login;
pub use self::login::LoginState;
pub use self::room_history as RoomHistory;
pub use self::room_settings as RoomSettings;

#[derive(Debug, Clone, PartialEq)]
pub enum ButtonState {
    Sensitive,
    Insensitive,
}

impl<'a> From<&'a glib::Variant> for ButtonState {
    fn from(v: &glib::Variant) -> ButtonState {
        v.get::<bool>().expect("Invalid button state type").into()
    }
}

impl From<bool> for ButtonState {
    fn from(v: bool) -> ButtonState {
        if v {
            ButtonState::Sensitive
        } else {
            ButtonState::Insensitive
        }
    }
}

impl From<ButtonState> for bool {
    fn from(v: ButtonState) -> bool {
        (v == ButtonState::Sensitive)
    }
}

impl From<ButtonState> for glib::Variant {
    fn from(v: ButtonState) -> glib::Variant {
        (v == ButtonState::Sensitive).to_variant()
    }
}

pub trait StateExt {
    fn bind_button_state(&self, button: &gtk::Button);
}

// FIXME: workaround till we get GPropertyAction
impl StateExt for gio::Action {
    fn bind_button_state(&self, button: &gtk::Button) {
        let button = button.downgrade();
        if let Some(action) = self.downcast_ref::<SimpleAction>() {
            action.connect_change_state(move |_, data| {
                if let Some(data) = data {
                    let state: ButtonState = data.into();
                    let button = upgrade_weak!(button);
                    button.set_sensitive(state.into());
                }
            });
        }
    }
}
