extern crate glib_sys as glib_ffi;
extern crate gobject_sys as gobject_ffi;

// The GObject subclass for carrying all properties each row in the sidebar needs
use gobject_subclass::object::*;

use glib::translate::*;
use gtk::prelude::*;
use gio::ListStoreExt;

use std::mem;
use std::ptr;

// Implementation sub-module of the GObject
mod imp {
    use super::*;
    use std::cell::RefCell;

    // The actual data structure that stores our values. This is not accessible
    // directly from the outside.
    pub struct SidebarRow {
        room_id: RefCell<Option<String>>,
        name: RefCell<Option<String>>,
        avatar: RefCell<Option<String>>,
        notifications: RefCell<Option<String>>,
        direct: RefCell<bool>,
        bold: RefCell<bool>,
        highlight: RefCell<bool>,
        selected: RefCell<bool>,
        hidden: RefCell<bool>,
        // This is a timestamp used for sorting
        key: RefCell<u64>,
    }

    // GObject property definitions for our two values
    static PROPERTIES: [Property; 10] = [
        Property::String(
            "room_id",
            "RoomId",
            "RoomId",
            None, // Default value
            PropertyMutability::ReadWrite,
        ),
        Property::String(
            "name",
            "Name",
            "Name",
            None, // Default value
            PropertyMutability::ReadWrite,
        ),
        Property::String(
            "avatar",
            "Avatar",
            "Avatar",
            None, // Default value
            PropertyMutability::ReadWrite,
        ),
        Property::String(
            "notifications",
            "Notifications",
            "Notifications",
            None, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
        Property::Boolean(
            "direct",
            "Direct",
            "Direct",
            false, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
        Property::Boolean(
            "bold",
            "Bold",
            "Bold",
            false, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
        Property::Boolean(
            "highlight",
            "Highlight",
            "Highlight",
            false, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
        Property::Boolean(
            "selected",
            "Selected",
            "Selected",
            false, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
        Property::Boolean(
            "hidden",
            "Hidden",
            "Hidden",
            false, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
        Property::UInt64(
            "key",
            "Key",
            "Key",
            (u64::min_value(), u64::max_value()),
            0, // Allowed range and default value
            PropertyMutability::ReadWrite,
        ),
    ];

    impl SidebarRow {
        // glib::Type registration of the SidebarRow type. The very first time
        // this registers the type with GObject and afterwards only returns
        // the type id that was registered the first time
        pub fn get_type() -> glib::Type {
            use std::sync::{Once, ONCE_INIT};

            // unsafe code here because static mut variables are inherently
            // unsafe. Via std::sync::Once we guarantee here that the variable
            // is only ever set once, and from that point onwards is only ever
            // read, which makes its usage safe.
            static ONCE: Once = ONCE_INIT;
            static mut TYPE: glib::Type = glib::Type::Invalid;

            ONCE.call_once(|| {
                let t = register_type(SidebarRowStatic);
                unsafe {
                    TYPE = t;
                }
            });

            unsafe { TYPE }
        }

        // Called exactly once before the first instantiation of an instance. This
        // sets up any type-specific things, in this specific case it installs the
        // properties so that GObject knows about their existence and they can be
        // used on instances of our type
        fn class_init(klass: &mut ObjectClass) {
            klass.install_properties(&PROPERTIES);
        }

        // Called once at the very beginning of instantiation of each instance and
        // creates the data structure that contains all our state
        fn init(_obj: &Object) -> Box<ObjectImpl<Object>> {
            let imp = Self {
                room_id: RefCell::new(None),
                name: RefCell::new(None),
                avatar: RefCell::new(None),
                notifications: RefCell::new(None),
                direct: RefCell::new(false),
                bold: RefCell::new(false),
                highlight: RefCell::new(false),
                selected: RefCell::new(false),
                hidden: RefCell::new(false),
                key: RefCell::new(0),
            };
            Box::new(imp)
        }
    }

    // The ObjectImpl trait provides the setters/getters for GObject properties.
    // Here we need to provide the values that are internally stored back to the
    // caller, or store whatever new value the caller is providing.
    //
    // This maps between the GObject properties and our internal storage of the
    // corresponding values of the properties.
    impl ObjectImpl<Object> for SidebarRow {
        fn set_property(&self, _obj: &glib::Object, id: u32, value: &glib::Value) {
            let prop = &PROPERTIES[id as usize];

            match *prop {
                Property::String("room_id", ..) => {
                    let room_id = value.get();
                    self.room_id.replace(room_id.clone());
                }
                Property::String("name", ..) => {
                    let name = value.get();
                    self.name.replace(name.clone());
                }
                Property::String("avatar", ..) => {
                    let avatar = value.get();
                    self.avatar.replace(avatar.clone());
                }
                Property::String("notifications", ..) => {
                    let notifications = value.get();
                    self.notifications.replace(notifications.clone());
                }
                Property::Boolean("direct", ..) => {
                    let direct = value.get().unwrap();
                    self.direct.replace(direct);
                }
                Property::Boolean("bold", ..) => {
                    let bold = value.get().unwrap();
                    self.bold.replace(bold);
                }
                Property::Boolean("highlight", ..) => {
                    let highlight = value.get().unwrap();
                    self.highlight.replace(highlight);
                }
                Property::Boolean("selected", ..) => {
                    let selected = value.get().unwrap();
                    self.selected.replace(selected);
                }
                Property::Boolean("hidden", ..) => {
                    let hidden = value.get().unwrap();
                    self.hidden.replace(hidden);
                }
                Property::UInt64("key", ..) => {
                    let key = value.get().unwrap();
                    self.key.replace(key);
                }
                _ => unimplemented!(),
            }
        }

        fn get_property(&self, _obj: &glib::Object, id: u32) -> Result<glib::Value, ()> {
            let prop = &PROPERTIES[id as usize];

            match *prop {
                Property::String("room_id", ..) => Ok(self.room_id.borrow().clone().to_value()),
                Property::String("name", ..) => Ok(self.name.borrow().clone().to_value()),
                Property::String("avatar", ..) => Ok(self.avatar.borrow().clone().to_value()),
                Property::String("notifications", ..) => {
                    Ok(self.notifications.borrow().clone().to_value())
                }
                Property::Boolean("direct", ..) => Ok(self.direct.borrow().clone().to_value()),
                Property::Boolean("bold", ..) => Ok(self.bold.borrow().clone().to_value()),
                Property::Boolean("highlight", ..) => {
                    Ok(self.highlight.borrow().clone().to_value())
                }
                Property::Boolean("selected", ..) => Ok(self.selected.borrow().clone().to_value()),
                Property::Boolean("hidden", ..) => Ok(self.hidden.borrow().clone().to_value()),
                Property::UInt64("key", ..) => Ok(self.key.borrow().clone().to_value()),
                _ => unimplemented!(),
            }
        }
    }

    // Static, per-type data that is used for actually registering the type
    // and providing the name of our type and how to initialize it to GObject
    //
    // It is used above in the get_type() function for passing that information
    // to GObject
    struct SidebarRowStatic;

    impl ImplTypeStatic<Object> for SidebarRowStatic {
        fn get_name(&self) -> &str {
            "SidebarRow"
        }

        fn new(&self, obj: &Object) -> Box<ObjectImpl<Object>> {
            SidebarRow::init(obj)
        }

        fn class_init(&self, klass: &mut ObjectClass) {
            SidebarRow::class_init(klass);
        }
    }
}

// Public part of the SidebarRow type. This behaves like a normal gtk-rs-style GObject
// binding
glib_wrapper! {
    pub struct SidebarRow(Object<imp::SidebarRow>):
        [Object => InstanceStruct<Object>];

    match fn {
        get_type => || imp::SidebarRow::get_type().to_glib(),
    }
}

// Constructor for new instances. This simply calls glib::Object::new() with
// initial values for our two properties and then returns the new instance
impl SidebarRow {
    pub fn new(
        room_id: &str,
        name: &str,
        avatar: &str,
        notifications: &str,
        direct: bool,
        bold: bool,
        highlight: bool,
        key: u64,
    ) -> SidebarRow {
        use glib::object::Downcast;

        unsafe {
            glib::Object::new(
                Self::static_type(),
                &[
                    ("room_id", &room_id),
                    ("name", &name),
                    ("avatar", &avatar),
                    ("notifications", &notifications),
                    ("direct", &direct),
                    ("bold", &bold),
                    ("highlight", &highlight),
                    ("selected", &false),
                    ("hidden", &false),
                    ("key", &key),
                ],
            )
            .unwrap()
            .downcast_unchecked()
        }
    }
}

// Implment conversion from rust struct to SidebarRow
use fractal_api::types::Room;
impl<'a> From<&'a Room> for SidebarRow {
    fn from(room: &Room) -> SidebarRow {
        let notifications = if room.inv {
            ".".to_string()
        } else if room.notifications != 0 {
            room.notifications.to_string()
        } else {
            "".to_string()
        };
        // Todo: use key, bold and highlight value from room
        SidebarRow::new(&room.id,
                        room.name.as_ref().unwrap_or(&"...".to_string()),
                        room.avatar.as_ref().unwrap_or(&"".to_string()),
                        &notifications,
                        room.direct, false, false, 0)
    }
}

pub struct Sidebar {
    invites: gio::ListStore,
    favorites: gio::ListStore,
    rooms: gio::ListStore,
    low_priority: gio::ListStore,
}

impl Sidebar {
    pub fn new() -> Self {
        // create ListStores for each room category
        let invites = gio::ListStore::new(SidebarRow::static_type());
        let favorites = gio::ListStore::new(SidebarRow::static_type());
        let rooms = gio::ListStore::new(SidebarRow::static_type());
        let low_priority = gio::ListStore::new(SidebarRow::static_type());
        Sidebar {
            invites,
            favorites,
            rooms,
            low_priority,
        }
    }

    pub fn get_invites_store(&self) -> &gio::ListStore {
        &self.invites
    }

    pub fn get_favorites_store(&self) -> &gio::ListStore {
        &self.favorites
    }

    pub fn get_rooms_store(&self) -> &gio::ListStore {
        &self.rooms
    }

    pub fn get_low_priority_store(&self) -> &gio::ListStore {
        &self.low_priority
    }

    // Add the room to the correct liststore
    pub fn add_room(&self, room: &Room) {
        let row: SidebarRow = room.into();
        if room.fav {
            // favorites
            self.favorites.append(&row);
        } else if room.inv {
            // invites
            self.invites.append(&row);
        } else {
            // Normal rooms
            self.rooms.append(&row);
        }
        //TODO:: popolate low_priority
    }

    pub fn remove_all(&self) {
        self.invites.remove_all();
        self.favorites.remove_all();
        self.rooms.remove_all();
        self.low_priority.remove_all();
    }
}
