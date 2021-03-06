use crate::actions::AppState;
use crate::model::{member::Member, message::Message};
use crate::util::i18n::i18n;
use crate::widgets::{self, SVEntry};
use gtk::prelude::*;

pub mod about;
pub mod account;
pub mod attach;
pub mod connect;
pub mod directory;
pub mod invite;
pub mod media_viewer;
pub mod member;
pub mod notify;
pub mod room_settings;
pub mod start_chat;
pub mod state;
pub mod user;

pub struct UI {
    pub builder: gtk::Builder,
    pub gtk_app: gtk::Application,
    pub main_window: libhandy::ApplicationWindow,
    pub sventry: SVEntry,
    pub sventry_box: Box<gtk::Stack>,
    pub subview_stack: gtk::Stack,
    pub room_settings: Option<room_settings::RoomSettings>,
    pub history: Option<widgets::RoomHistory>,
    pub roomlist: widgets::RoomList,
    pub media_viewer: Option<widgets::MediaViewer>,
    pub room_back_history: Vec<AppState>,
    pub invite_list: Vec<(Member, gtk::TextChildAnchor)>,
    pub leaflet: libhandy::Leaflet,
    pub deck: libhandy::Deck,
    pub account_settings: account::AccountSettings,
    pub direct_chat_dialog: start_chat::DirectChatDialog,
}

impl UI {
    pub fn new(gtk_app: gtk::Application) -> UI {
        // The order here is important because some ui file depends on others

        let builder = gtk::Builder::new();

        builder
            .add_from_resource("/org/gnome/Fractal/ui/autocomplete.ui")
            .expect("Can't load ui file: autocomplete.ui");

        // needed from main_window
        // These are popup menus showed from main_window interface
        builder
            .add_from_resource("/org/gnome/Fractal/ui/main_menu.ui")
            .expect("Can't load ui file: main_menu.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/add_room_menu.ui")
            .expect("Can't load ui file: add_room_menu.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/room_menu.ui")
            .expect("Can't load ui file: room_menu.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/markdown_popover.ui")
            .expect("Can't load ui file: markdown_popover.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/server_chooser_menu.ui")
            .expect("Can't load ui file: server_chooser_menu.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/main_window.ui")
            .expect("Can't load ui file: main_window.ui");

        // Order which sventry is created matters
        let sventry_stack = gtk::Stack::new();

        let sventry = SVEntry::default();
        sventry_stack.add_named(&sventry.clamp, "Text Entry");
        let sventry_disabled = gtk::Label::new(Some(&i18n(
            "You don’t have permission to post to this room",
        )));
        sventry_disabled.set_hexpand(false);
        sventry_disabled.get_style_context().add_class("dim-label");
        sventry_disabled.set_line_wrap(true);
        sventry_disabled.set_line_wrap_mode(pango::WrapMode::WordChar);
        sventry_stack.add_named(&sventry_disabled, "Disabled Entry");

        let sventry_box = Box::new(sventry_stack.clone());
        let parent: gtk::Box = builder.get_object("room_parent").unwrap();
        parent.add(&sventry_stack);

        let subview_stack = builder
            .get_object("subview_stack")
            .expect("Can't find subview_stack in ui file.");

        // Depends on main_window
        // These are all dialogs transient for main_window
        builder
            .add_from_resource("/org/gnome/Fractal/ui/invite.ui")
            .expect("Can't load ui file: invite.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/invite_user.ui")
            .expect("Can't load ui file: invite_user.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/join_room.ui")
            .expect("Can't load ui file: join_room.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/leave_room.ui")
            .expect("Can't load ui file: leave_room.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/new_room.ui")
            .expect("Can't load ui file: new_room.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/password_dialog.ui")
            .expect("Can't load ui file: password_dialog.ui");
        let account_settings = account::AccountSettings::new();

        let main_window: libhandy::ApplicationWindow = builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        main_window.set_application(Some(&gtk_app));
        main_window.set_title("Fractal");

        let leaflet = builder
            .get_object::<libhandy::Leaflet>("chat_page")
            .expect("Couldn't find chat_page in ui file");
        let deck = builder
            .get_object::<libhandy::Deck>("main_deck")
            .expect("Couldn't find main_deck in ui file");

        let direct_chat_dialog = start_chat::DirectChatDialog::new(&main_window);

        UI {
            builder,
            gtk_app,
            main_window,
            sventry,
            sventry_box,
            subview_stack,
            room_settings: None,
            history: None,
            roomlist: widgets::RoomList::new(None, None),
            media_viewer: None,
            room_back_history: vec![],
            invite_list: vec![],
            leaflet,
            deck,
            account_settings,
            direct_chat_dialog,
        }
    }

    pub fn create_error_dialog(&self, msg: &str) -> gtk::MessageDialog {
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&self.main_window),
            flags,
            gtk::MessageType::Error,
            gtk::ButtonsType::None,
            msg,
        );
        dialog.add_button(&i18n("OK"), gtk::ResponseType::Ok);

        dialog
    }
}

/* MessageContent contains all data needed to display one row
 * therefore it should contain only one Message body with one format
 * To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Clone)]
pub struct MessageContent {
    pub msg: Message,
    pub sender_name: Option<String>,
    pub mtype: RowType,
    pub highlights: Vec<String>,
    pub redactable: bool,
    pub last_viewed: bool,
    pub widget: Option<widgets::MessageBox>,
}

/* To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RowType {
    Mention,
    Emote,
    Message,
    Sticker,
    Image,
    Audio,
    Video,
    File,
    Emoji,
}
