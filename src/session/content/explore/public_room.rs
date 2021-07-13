use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*};
use matrix_sdk::ruma::directory::PublicRoomsChunk;

use crate::session::{room::Room, Avatar, Session};

mod imp {
    use super::*;
    use glib::signal::SignalHandlerId;
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default)]
    pub struct PublicRoom {
        pub session: OnceCell<Session>,
        pub matrix_public_room: OnceCell<PublicRoomsChunk>,
        pub avatar: OnceCell<Avatar>,
        pub room: OnceCell<Room>,
        pub is_pending: Cell<bool>,
        pub room_handler: RefCell<Option<SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PublicRoom {
        const NAME: &'static str = "PublicRoom";
        type Type = super::PublicRoom;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for PublicRoom {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "session",
                        "Session",
                        "The session",
                        Session::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_object(
                        "room",
                        "Room",
                        "The room, this is only set if the user is already a member",
                        Room::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpec::new_boolean(
                        "pending",
                        "Pending",
                        "A room is pending when the user already clicked to join a room",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpec::new_object(
                        "avatar",
                        "Avatar",
                        "The Avatar of this room",
                        Avatar::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "session" => self.session.set(value.get().unwrap()).unwrap(),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "session" => obj.session().to_value(),
                "avatar" => obj.avatar().to_value(),
                "room" => obj.room().to_value(),
                "pending" => obj.is_pending().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.avatar.set(Avatar::new(obj.session(), None)).unwrap();

            obj.session()
                .room_list()
                .connect_pending_rooms_changed(clone!(@weak obj => move |_| {
                    if let Some(matrix_public_room) = obj.matrix_public_room() {
                        obj.set_pending(obj.session()
                        .room_list()
                        .is_pending_room(&matrix_public_room.room_id.clone().into()));
                    }
                }));
        }

        fn dispose(&self, obj: &Self::Type) {
            if let Some(handler_id) = self.room_handler.take() {
                obj.session().room_list().disconnect(handler_id);
            }
        }
    }
}

glib::wrapper! {
    pub struct PublicRoom(ObjectSubclass<imp::PublicRoom>);
}

impl PublicRoom {
    pub fn new(session: &Session) -> Self {
        glib::Object::new(&[("session", session)]).expect("Failed to create Room")
    }

    pub fn session(&self) -> &Session {
        let priv_ = imp::PublicRoom::from_instance(&self);
        priv_.session.get().unwrap()
    }

    pub fn avatar(&self) -> &Avatar {
        let priv_ = imp::PublicRoom::from_instance(self);
        priv_.avatar.get().unwrap()
    }

    /// The room if the user is already a member of this room.
    pub fn room(&self) -> Option<&Room> {
        let priv_ = imp::PublicRoom::from_instance(self);
        priv_.room.get()
    }

    fn set_room(&self, room: Room) {
        let priv_ = imp::PublicRoom::from_instance(self);
        priv_.room.set(room).unwrap();
        self.notify("room");
    }

    fn set_pending(&self, is_pending: bool) {
        let priv_ = imp::PublicRoom::from_instance(self);

        if self.is_pending() == is_pending {
            return;
        }

        priv_.is_pending.set(is_pending);
        self.notify("pending");
    }

    pub fn is_pending(&self) -> bool {
        let priv_ = imp::PublicRoom::from_instance(self);
        priv_.is_pending.get()
    }

    pub fn set_matrix_public_room(&self, room: PublicRoomsChunk) {
        let priv_ = imp::PublicRoom::from_instance(self);

        self.avatar().set_display_name(room.name.clone());
        self.avatar().set_url(room.avatar_url.clone());

        if let Some(room) = self.session().room_list().get(&room.room_id) {
            self.set_room(room);
        } else {
            let room_id = room.room_id.clone();
            let handler_id = self.session().room_list().connect_items_changed(
                clone!(@weak self as obj => move |room_list, _, _, _| {
                    if let Some(room) = room_list.get(&room_id) {
                        let priv_ = imp::PublicRoom::from_instance(&obj);
                        if let Some(handler_id) = priv_.room_handler.take() {
                            obj.set_room(room);
                            room_list.disconnect(handler_id);
                        }
                    }
                }),
            );

            priv_.room_handler.replace(Some(handler_id));
        }

        self.set_pending(
            self.session()
                .room_list()
                .is_pending_room(&room.room_id.clone().into()),
        );

        priv_.matrix_public_room.set(room).unwrap();
    }

    pub fn matrix_public_room(&self) -> Option<&PublicRoomsChunk> {
        let priv_ = imp::PublicRoom::from_instance(self);
        priv_.matrix_public_room.get()
    }

    pub fn join_or_view(&self) {
        let session = self.session();
        if let Some(room) = self.room() {
            session.set_selected_room(Some(room.clone()));
        } else {
            if let Some(matrix_public_room) = self.matrix_public_room() {
                session
                    .room_list()
                    .join_by_id_or_alias(matrix_public_room.room_id.clone().into());
            }
        }
    }
}
