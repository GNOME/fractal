use gtk::{glib, glib::DateTime, prelude::*, subclass::prelude::*};
use matrix_sdk::ruma::{
    events::AnyRoomEvent,
    identifiers::{EventId, UserId},
};

use crate::session::room::Event;

/// This enum contains all possible types the room history can hold.
#[derive(Debug, Clone)]
pub enum ItemType {
    Event(Event),
    // TODO: Add item type for grouped events
    DayDivider(DateTime),
    NewMessageDivider,
}

#[derive(Clone, Debug, glib::GBoxed)]
#[gboxed(type_name = "BoxedItemType")]
pub struct BoxedItemType(ItemType);

impl From<ItemType> for BoxedItemType {
    fn from(type_: ItemType) -> Self {
        BoxedItemType(type_)
    }
}

mod imp {
    use super::*;
    use once_cell::sync::{Lazy, OnceCell};

    #[derive(Debug, Default)]
    pub struct Item {
        pub type_: OnceCell<ItemType>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Item {
        const NAME: &'static str = "RoomItem";
        type Type = super::Item;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Item {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_boxed(
                        "type",
                        "Type",
                        "The type of this item",
                        BoxedItemType::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_boolean(
                        "selectable",
                        "Selectable",
                        "Whether this item is selectable.",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpec::new_boolean(
                        "show-header",
                        "Show Header",
                        "Whether this item should show a header. This does do nothing if this event doesn’t have a header. ",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_boolean(
                        "can-hide-header",
                        "Can hide header",
                        "Whether this item is allowed to hide its header.",
                        false,
                        glib::ParamFlags::READABLE,
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
                "type" => {
                    let type_ = value.get::<BoxedItemType>().unwrap();
                    self.type_.set(type_.0).unwrap();
                }
                "show-header" => {
                    let show_header = value.get().unwrap();
                    let _ = obj.set_show_header(show_header);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "selectable" => obj.selectable().to_value(),
                "show-header" => obj.show_header().to_value(),
                "can-hide-header" => obj.can_hide_header().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Item(ObjectSubclass<imp::Item>);
}

/// This represents any row inside the room history.
/// This can be AnyRoomEvent, a day divider or new message divider.
impl Item {
    pub fn for_event(event: Event) -> Self {
        let type_ = BoxedItemType(ItemType::Event(event));
        glib::Object::new(&[("type", &type_)]).expect("Failed to create Item")
    }

    pub fn for_day_divider(day: DateTime) -> Self {
        let type_ = BoxedItemType(ItemType::DayDivider(day));
        glib::Object::new(&[("type", &type_)]).expect("Failed to create Item")
    }

    pub fn for_new_message_divider() -> Self {
        let type_ = BoxedItemType(ItemType::NewMessageDivider);
        glib::Object::new(&[("type", &type_)]).expect("Failed to create Item")
    }

    pub fn selectable(&self) -> bool {
        let priv_ = imp::Item::from_instance(&self);
        if let ItemType::Event(_event) = priv_.type_.get().unwrap() {
            true
        } else {
            false
        }
    }

    pub fn matrix_event(&self) -> Option<AnyRoomEvent> {
        let priv_ = imp::Item::from_instance(&self);
        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            Some(event.matrix_event())
        } else {
            None
        }
    }

    pub fn event(&self) -> Option<&Event> {
        let priv_ = imp::Item::from_instance(&self);
        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            Some(event)
        } else {
            None
        }
    }

    pub fn matrix_sender(&self) -> Option<UserId> {
        let priv_ = imp::Item::from_instance(&self);
        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            Some(event.matrix_sender())
        } else {
            None
        }
    }

    pub fn matrix_event_id(&self) -> Option<EventId> {
        let priv_ = imp::Item::from_instance(&self);

        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            Some(event.matrix_event_id())
        } else {
            None
        }
    }

    pub fn event_timestamp(&self) -> Option<DateTime> {
        let priv_ = imp::Item::from_instance(&self);

        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            Some(event.timestamp())
        } else {
            None
        }
    }

    pub fn set_show_header(&self, visible: bool) {
        let priv_ = imp::Item::from_instance(&self);
        if self.show_header() == visible {
            return;
        }

        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            event.set_show_header(visible);
        }

        self.notify("show-header");
    }

    pub fn show_header(&self) -> bool {
        let priv_ = imp::Item::from_instance(&self);

        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            event.show_header()
        } else {
            false
        }
    }

    pub fn can_hide_header(&self) -> bool {
        let priv_ = imp::Item::from_instance(&self);

        if let ItemType::Event(event) = priv_.type_.get().unwrap() {
            event.can_hide_header()
        } else {
            false
        }
    }

    pub fn type_(&self) -> &ItemType {
        let priv_ = imp::Item::from_instance(&self);
        priv_.type_.get().unwrap()
    }

    pub fn connect_show_header_notify<F: Fn(&Self, &glib::ParamSpec) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_notify_local(Some("show-header"), f)
    }
}
