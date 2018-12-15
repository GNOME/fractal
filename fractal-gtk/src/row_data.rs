//extern crate glib_sys as glib_ffi;
//extern crate gobject_sys as gobject_ffi;

extern crate gobject_subclass;

use gio::prelude::*;
use gtk::prelude::*;

/*
    let model = gio::ListStore::new(RowData::static_type());
    let listbox = gtk::ListBox::new();
    listbox.bind_model(&model, clone!(window_weak => move |item| {
        let box_ = gtk::ListBoxRow::new();
        let item = item.downcast_ref::<RowData>().unwrap();

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        let label = gtk::Label::new(None);
        item.bind_property("name", &label, "label")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .build();
        hbox.pack_start(&label, true, true, 0);

        box_
    }));
*/

// Our GObject subclass for carrying a name and count for the ListBox model
//
// Both name and count are stored in a RefCell to allow for interior mutability
// and are exposed via normal GObject properties. This allows us to use property
// bindings below to bind the values with what widgets display in the UI
pub mod row_data {
    use super::*;

    use row_data::gobject_subclass::object::*;

    use glib::translate::*;

    use std::mem;
    use std::ptr;

    // Implementation sub-module of the GObject
    mod imp {
        use super::*;
        use std::cell::RefCell;

        // The actual data structure that stores our values. This is not accessible
        // directly from the outside.
        pub struct RowData {
            room_id: RefCell<Option<String>>,
            name: RefCell<Option<String>>,
            avatar: RefCell<Option<String>>,
            notifications: RefCell<Option<String>>,
            direct: RefCell<bool>,
            bold: RefCell<bool>,
            highlight: RefCell<bool>,
        }

        // GObject property definitions for our two values
        static PROPERTIES: [Property; 7] = [
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
        ];

        impl RowData {
            // glib::Type registration of the RowData type. The very first time
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
                    let t = register_type(RowDataStatic);
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
        impl ObjectImpl<Object> for RowData {
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
                    _ => unimplemented!(),
                }
            }
        }

        // Static, per-type data that is used for actually registering the type
        // and providing the name of our type and how to initialize it to GObject
        //
        // It is used above in the get_type() function for passing that information
        // to GObject
        struct RowDataStatic;

        impl ImplTypeStatic<Object> for RowDataStatic {
            fn get_name(&self) -> &str {
                "RowData"
            }

            fn new(&self, obj: &Object) -> Box<ObjectImpl<Object>> {
                RowData::init(obj)
            }

            fn class_init(&self, klass: &mut ObjectClass) {
                RowData::class_init(klass);
            }
        }
    }

    // Public part of the RowData type. This behaves like a normal gtk-rs-style GObject
    // binding
    glib_wrapper! {
        pub struct RowData(Object<imp::RowData>):
            [Object => InstanceStruct<Object>];

        match fn {
            get_type => || imp::RowData::get_type().to_glib(),
        }
    }

    // Constructor for new instances. This simply calls glib::Object::new() with
    // initial values for our two properties and then returns the new instance
    impl RowData {
        pub fn new(
            room_id: &str,
            name: &str,
            avatar: &str,
            notifications: &str,
            direct: bool,
            bold: bool,
            highlight: bool,
        ) -> RowData {
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
                    ],
                )
                .unwrap()
                .downcast_unchecked()
            }
        }
    }
}
