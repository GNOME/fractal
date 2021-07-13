use crate::session::{
    content::{ContentType, Explore, Invite, RoomHistory},
    room::{Room, RoomType},
    Session,
};
use adw::subclass::prelude::*;
use gtk::{gio, glib, glib::clone, prelude::*, subclass::prelude::*, CompositeTemplate};

mod imp {
    use super::*;
    use glib::{signal::SignalHandlerId, subclass::InitializingObject};
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content.ui")]
    pub struct Content {
        pub compact: Cell<bool>,
        pub session: RefCell<Option<Session>>,
        pub room: RefCell<Option<Room>>,
        pub content_type: Cell<ContentType>,
        pub error_list: RefCell<Option<gio::ListStore>>,
        pub category_handler: RefCell<Option<SignalHandlerId>>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub room_history: TemplateChild<RoomHistory>,
        #[template_child]
        pub invite: TemplateChild<Invite>,
        #[template_child]
        pub explore: TemplateChild<Explore>,
        #[template_child]
        pub empty_page: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Content {
        const NAME: &'static str = "Content";
        type Type = super::Content;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            RoomHistory::static_type();
            Invite::static_type();
            Explore::static_type();
            Self::bind_template(klass);
            klass.set_accessible_role(gtk::AccessibleRole::Group);

            klass.install_action("content.go-back", None, move |widget, _, _| {
                widget.set_content_type(ContentType::None);
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Content {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "session",
                        "Session",
                        "The session",
                        Session::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
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
                    glib::ParamSpec::new_object(
                        "error-list",
                        "Error List",
                        "A list of errors shown as in-app-notification",
                        gio::ListStore::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_enum(
                        "content-type",
                        "Content Type",
                        "The type of content currently displayed",
                        ContentType::static_type(),
                        ContentType::default() as i32,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "session" => {
                    let _ = self.session.replace(value.get().unwrap());
                }
                "room" => {
                    let room = value.get().unwrap();
                    obj.set_room(room);
                }
                "error-list" => {
                    self.error_list.replace(value.get().unwrap());
                }
                "content-type" => obj.set_content_type(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "session" => obj.session().to_value(),
                "room" => obj.room().to_value(),
                "error-list" => self.error_list.borrow().to_value(),
                "content-type" => obj.content_type().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Content {}
    impl BinImpl for Content {}
}

glib::wrapper! {
    pub struct Content(ObjectSubclass<imp::Content>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl Content {
    pub fn new(session: &Session) -> Self {
        glib::Object::new(&[("session", session)]).expect("Failed to create Content")
    }

    pub fn session(&self) -> Option<Session> {
        let priv_ = imp::Content::from_instance(self);
        priv_.session.borrow().to_owned()
    }

    pub fn content_type(&self) -> ContentType {
        let priv_ = imp::Content::from_instance(self);
        priv_.content_type.get()
    }

    pub fn set_content_type(&self, content_type: ContentType) {
        let priv_ = imp::Content::from_instance(self);

        if self.content_type() == content_type {
            return;
        }

        priv_.content_type.set(content_type);
        self.set_visible_child();

        self.notify("content-type");
    }

    pub fn set_room(&self, room: Option<Room>) {
        let priv_ = imp::Content::from_instance(self);

        if self.room() == room {
            return;
        }

        if let Some(category_handler) = priv_.category_handler.take() {
            if let Some(room) = self.room() {
                room.disconnect(category_handler);
            }
        }

        if let Some(ref room) = room {
            let handler_id = room.connect_notify_local(
                Some("category"),
                clone!(@weak self as obj => move |_, _| {
                        obj.set_visible_child();
                }),
            );

            priv_.category_handler.replace(Some(handler_id));
        }

        priv_.room.replace(room);
        self.set_visible_child();
        self.notify("room");
    }

    pub fn room(&self) -> Option<Room> {
        let priv_ = imp::Content::from_instance(self);
        priv_.room.borrow().clone()
    }

    fn set_visible_child(&self) {
        let priv_ = imp::Content::from_instance(self);

        match self.content_type() {
            ContentType::None => {
                priv_.stack.set_visible_child(&*priv_.empty_page);
            }
            ContentType::Room => {
                if let Some(room) = &*priv_.room.borrow() {
                    if room.category() == RoomType::Invited {
                        priv_.stack.set_visible_child(&*priv_.invite);
                    } else {
                        priv_.stack.set_visible_child(&*priv_.room_history);
                    }
                }
            }
            ContentType::Explore => {
                priv_.explore.init();
                priv_.stack.set_visible_child(&*priv_.explore);
            }
        }
    }
}
