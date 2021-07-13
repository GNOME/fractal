use crate::components::RoomTitle;
use crate::session::{content::ItemRow, content::MarkdownPopover, room::Room, room::RoomType};
use adw::subclass::prelude::*;
use gtk::{
    gdk, glib, glib::clone, glib::signal::Inhibit, prelude::*, subclass::prelude::*,
    CompositeTemplate,
};
use sourceview::prelude::*;

mod imp {
    use super::*;
    use crate::Application;
    use glib::{signal::SignalHandlerId, subclass::InitializingObject};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content-room-history.ui")]
    pub struct RoomHistory {
        pub compact: Cell<bool>,
        pub room: RefCell<Option<Room>>,
        pub category_handler: RefCell<Option<SignalHandlerId>>,
        pub empty_timeline_handler: RefCell<Option<SignalHandlerId>>,
        pub md_enabled: Cell<bool>,
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub room_title: TemplateChild<RoomTitle>,
        #[template_child]
        pub room_menu: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub listview: TemplateChild<gtk::ListView>,
        #[template_child]
        pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub message_entry: TemplateChild<sourceview::View>,
        #[template_child]
        pub markdown_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub loading: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RoomHistory {
        const NAME: &'static str = "ContentRoomHistory";
        type Type = super::RoomHistory;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            ItemRow::static_type();
            MarkdownPopover::static_type();
            Self::bind_template(klass);
            klass.set_accessible_role(gtk::AccessibleRole::Group);
            klass.install_action(
                "room-history.send-text-message",
                None,
                move |widget, _, _| {
                    widget.send_text_message();
                },
            );
            klass.install_action("room-history.leave", None, move |widget, _, _| {
                widget.leave();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RoomHistory {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_boolean(
                        "compact",
                        "Compact",
                        "Whether a compact view is used",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "room",
                        "Room",
                        "The room currently shown",
                        Room::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_boolean(
                        "empty",
                        "Empty",
                        "Wheter there is currently a room shown",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpec::new_boolean(
                        "markdown-enabled",
                        "Markdown enabled",
                        "Whether outgoing messages should be interpreted as markdown",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "compact" => {
                    let compact = value.get().unwrap();
                    self.compact.set(compact);
                }
                "room" => {
                    let room = value.get().unwrap();
                    obj.set_room(room);
                }
                "markdown-enabled" => {
                    let md_enabled = value.get().unwrap();
                    self.md_enabled.set(md_enabled);
                    self.markdown_button.set_icon_name(if md_enabled {
                        "format-indent-more-symbolic"
                    } else {
                        "format-justify-left-symbolic"
                    });
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "room" => obj.room().to_value(),
                "empty" => obj.room().is_none().to_value(),
                "markdown-enabled" => self.md_enabled.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            let adj = self.listview.vadjustment().unwrap();

            adj.connect_value_changed(clone!(@weak obj => move |adj| {
                obj.load_more_messages(adj);
            }));

            let key_events = gtk::EventControllerKey::new();
            self.message_entry.add_controller(&key_events);

            key_events
                .connect_key_pressed(clone!(@weak obj => @default-return Inhibit(false), move |_, key, _, modifier| {
                if !modifier.contains(gdk::ModifierType::SHIFT_MASK) && (key == gdk::keys::constants::Return || key == gdk::keys::constants::KP_Enter) {
                    obj.activate_action("room-history.send-text-message", None);
                    Inhibit(true)
                } else {
                    Inhibit(false)
                }
            }));

            let buffer = self
                .message_entry
                .buffer()
                .downcast::<sourceview::Buffer>()
                .unwrap();

            buffer.connect_text_notify(clone!(@weak obj => move |buffer| {
               let (start_iter, end_iter) = buffer.bounds();
               obj.action_set_enabled("room-history.send-text-message", start_iter != end_iter);
            }));

            let (start_iter, end_iter) = buffer.bounds();
            obj.action_set_enabled("room-history.send-text-message", start_iter != end_iter);

            let md_lang =
                sourceview::LanguageManager::default().and_then(|lm| lm.language("markdown"));
            buffer.set_language(md_lang.as_ref());
            obj.bind_property("markdown-enabled", &buffer, "highlight-syntax")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            let settings = Application::default().settings();
            settings
                .bind("markdown-enabled", obj, "markdown-enabled")
                .build();

            self.parent_constructed(obj);
        }
    }

    impl WidgetImpl for RoomHistory {}
    impl BinImpl for RoomHistory {}
}

glib::wrapper! {
    pub struct RoomHistory(ObjectSubclass<imp::RoomHistory>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl RoomHistory {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create RoomHistory")
    }

    pub fn set_room(&self, room: Option<Room>) {
        let priv_ = imp::RoomHistory::from_instance(self);

        if self.room() == room {
            return;
        }

        if let Some(category_handler) = priv_.category_handler.take() {
            if let Some(room) = self.room() {
                room.disconnect(category_handler);
            }
        }

        if let Some(empty_timeline_handler) = priv_.empty_timeline_handler.take() {
            if let Some(room) = self.room() {
                room.timeline().disconnect(empty_timeline_handler);
            }
        }

        if let Some(ref room) = room {
            let handler_id = room.connect_notify_local(
                Some("category"),
                clone!(@weak self as obj => move |_, _| {
                        obj.update_room_state();
                }),
            );

            priv_.category_handler.replace(Some(handler_id));

            let handler_id = room.timeline().connect_notify_local(
                Some("empty"),
                clone!(@weak self as obj => move |_, _| {
                        obj.set_empty_timeline();
                }),
            );

            priv_.empty_timeline_handler.replace(Some(handler_id));
        }

        // TODO: use gtk::MultiSelection to allow selection
        let model = room
            .as_ref()
            .and_then(|room| Some(gtk::NoSelection::new(Some(room.timeline()))));

        priv_.listview.set_model(model.as_ref());
        priv_.room.replace(room);
        let adj = priv_.listview.vadjustment().unwrap();
        self.load_more_messages(&adj);
        self.update_room_state();
        self.set_empty_timeline();
        self.notify("room");
        self.notify("empty");
    }

    pub fn room(&self) -> Option<Room> {
        let priv_ = imp::RoomHistory::from_instance(self);
        priv_.room.borrow().clone()
    }

    pub fn send_text_message(&self) {
        let priv_ = imp::RoomHistory::from_instance(self);
        let buffer = priv_.message_entry.buffer();
        let (start_iter, end_iter) = buffer.bounds();
        let body = buffer.text(&start_iter, &end_iter, true);

        if let Some(room) = &*priv_.room.borrow() {
            room.send_text_message(body.as_str(), priv_.md_enabled.get());
        }

        buffer.set_text("");
    }

    pub fn leave(&self) {
        let priv_ = imp::RoomHistory::from_instance(self);

        if let Some(room) = &*priv_.room.borrow() {
            room.set_category(RoomType::Left);
        }
    }

    fn update_room_state(&self) {
        let priv_ = imp::RoomHistory::from_instance(self);

        if let Some(room) = &*priv_.room.borrow() {
            if room.category() == RoomType::Left {
                self.action_set_enabled("room-history.leave", false);
                priv_.room_menu.hide();
            } else {
                self.action_set_enabled("room-history.leave", true);
                priv_.room_menu.show();
            }
        }
    }

    fn set_empty_timeline(&self) {
        let priv_ = imp::RoomHistory::from_instance(self);

        if let Some(room) = &*priv_.room.borrow() {
            if room.timeline().empty() {
                priv_.stack.set_visible_child(&*priv_.loading);
            } else {
                priv_.stack.set_visible_child(&*priv_.scrolled_window);
            }
        }
    }

    fn load_more_messages(&self, adj: &gtk::Adjustment) {
        // Load more messages when the user gets close to the end of the known room history
        // Use the page size twice to detect if the user gets close to the end
        if adj.value() < adj.page_size() * 2.0 || adj.upper() <= adj.page_size() * 2.0 {
            if let Some(room) = self.room() {
                room.load_previous_events();
            }
        }
    }
}
