use crate::{
    components::{Avatar, LabelWithWidgets, Pill, SpinnerButton},
    session::room::{Room, RoomType},
};
use adw::subclass::prelude::*;
use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*, CompositeTemplate};
use gtk_macros::spawn;

mod imp {
    use super::*;
    use glib::{signal::SignalHandlerId, subclass::InitializingObject};
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content-invite.ui")]
    pub struct Invite {
        pub compact: Cell<bool>,
        pub room: RefCell<Option<Room>>,
        pub accept_requests: RefCell<HashSet<Room>>,
        pub reject_requests: RefCell<HashSet<Room>>,
        pub category_handler: RefCell<Option<SignalHandlerId>>,
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub room_topic: TemplateChild<gtk::Label>,
        #[template_child]
        pub accept_button: TemplateChild<SpinnerButton>,
        #[template_child]
        pub reject_button: TemplateChild<SpinnerButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Invite {
        const NAME: &'static str = "ContentInvite";
        type Type = super::Invite;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Pill::static_type();
            SpinnerButton::static_type();
            LabelWithWidgets::static_type();
            Avatar::static_type();
            Self::bind_template(klass);
            klass.set_accessible_role(gtk::AccessibleRole::Group);

            klass.install_action("invite.reject", None, move |widget, _, _| {
                widget.reject();
            });
            klass.install_action("invite.accept", None, move |widget, _, _| {
                widget.accept();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Invite {
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

                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "room" => obj.room().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.room_topic
                .connect_notify_local(Some("label"), |room_topic, _| {
                    room_topic.set_visible(!room_topic.label().is_empty());
                });

            self.room_topic
                .set_visible(!self.room_topic.label().is_empty());
        }
    }

    impl WidgetImpl for Invite {}
    impl BinImpl for Invite {}
}

glib::wrapper! {
    pub struct Invite(ObjectSubclass<imp::Invite>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl Invite {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Invite")
    }

    pub fn set_room(&self, room: Option<Room>) {
        let priv_ = imp::Invite::from_instance(self);

        if self.room() == room {
            return;
        }

        match room {
            Some(ref room) if priv_.accept_requests.borrow().contains(room) => {
                self.action_set_enabled("invite.accept", false);
                self.action_set_enabled("invite.reject", false);
                priv_.accept_button.set_loading(true);
            }
            Some(ref room) if priv_.reject_requests.borrow().contains(room) => {
                self.action_set_enabled("invite.accept", false);
                self.action_set_enabled("invite.reject", false);
                priv_.reject_button.set_loading(true);
            }
            _ => self.reset(),
        }

        if let Some(category_handler) = priv_.category_handler.take() {
            if let Some(room) = self.room() {
                room.disconnect(category_handler);
            }
        }

        // FIXME: remove closure when room changes
        if let Some(ref room) = room {
            let handler_id = room.connect_notify_local(
                Some("category"),
                clone!(@weak self as obj => move |room, _| {
                        if room.category() != RoomType::Invited {
                                let priv_ = imp::Invite::from_instance(&obj);
                                priv_.reject_requests.borrow_mut().remove(&room);
                                priv_.accept_requests.borrow_mut().remove(&room);
                                obj.reset();
                                if let Some(category_handler) = priv_.category_handler.take() {
                                    room.disconnect(category_handler);
                                }
                        }
                }),
            );
            priv_.category_handler.replace(Some(handler_id));
        }

        priv_.room.replace(room);

        self.notify("room");
    }

    pub fn room(&self) -> Option<Room> {
        let priv_ = imp::Invite::from_instance(self);
        priv_.room.borrow().clone()
    }

    fn reset(&self) {
        let priv_ = imp::Invite::from_instance(self);
        priv_.accept_button.set_loading(false);
        priv_.reject_button.set_loading(false);
        self.action_set_enabled("invite.accept", true);
        self.action_set_enabled("invite.reject", true);
    }

    fn accept(&self) -> Option<()> {
        let priv_ = imp::Invite::from_instance(self);
        let room = self.room()?;

        self.action_set_enabled("invite.accept", false);
        self.action_set_enabled("invite.reject", false);
        priv_.accept_button.set_loading(true);
        priv_.accept_requests.borrow_mut().insert(room.clone());

        spawn!(
            clone!(@weak self as obj, @strong room => move || async move {
                    let priv_ = imp::Invite::from_instance(&obj);
                    let result = room.accept_invite().await;
                    if result.is_err() {
                        priv_.accept_requests.borrow_mut().remove(&room);
                        obj.reset();
                    }
            })()
        );

        Some(())
    }

    fn reject(&self) -> Option<()> {
        let priv_ = imp::Invite::from_instance(self);
        let room = self.room()?;

        self.action_set_enabled("invite.accept", false);
        self.action_set_enabled("invite.reject", false);
        priv_.reject_button.set_loading(true);
        priv_.reject_requests.borrow_mut().insert(room.clone());

        spawn!(
            clone!(@weak self as obj, @strong room => move || async move {
                    let priv_ = imp::Invite::from_instance(&obj);
                    let result = room.reject_invite().await;
                    if result.is_err() {
                        priv_.reject_requests.borrow_mut().remove(&room);
                        obj.reset();
                    }
            })()
        );

        Some(())
    }
}
