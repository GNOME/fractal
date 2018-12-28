extern crate glib_sys as glib_ffi;
extern crate gobject_sys as gobject_ffi;

// The GObject subclass for carrying all properties each row in the sidebar needs
use gobject_subclass::object::*;

use gio::ListStoreExt;
use glib::translate::*;
use gtk::prelude::*;

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
        name: Option<&str>,
        avatar: Option<&str>,
        notifications: Option<&str>,
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

    pub fn change_name(&mut self, value: Option<&str>) {
        let name = "name";
        if let Some(value) = value {
            if let Some(current_value) =
                self.get_property(name).ok().and_then(|x| x.get::<String>())
            {
                if value == current_value {
                    return;
                }
            }
        }
        let _ = self.set_property(name, &value);
    }

    pub fn change_avatar(&mut self, value: Option<&str>) {
        let name = "avatar";
        if let Some(value) = value {
            if let Some(current_value) =
                self.get_property(name).ok().and_then(|x| x.get::<String>())
            {
                if value == current_value {
                    return;
                }
            }
        }
        let _ = self.set_property(name, &value);
    }
    pub fn change_notifications(&mut self, value: Option<&str>) {
        let name = "notifications";
        if let Some(value) = value {
            if let Some(current_value) =
                self.get_property(name).ok().and_then(|x| x.get::<String>())
            {
                if value == current_value {
                    return;
                }
            }
        }
        let _ = self.set_property(name, &value);
    }
    pub fn change_direct(&mut self, value: bool) {
        let name = "direct";
        if let Some(current_value) = self.get_property(name).ok().and_then(|x| x.get::<bool>()) {
            if value == current_value {
                return;
            }
        }
        let _ = self.set_property(name, &value);
    }
    pub fn change_bold(&mut self, value: bool) {
        let name = "bold";
        if let Some(current_value) = self.get_property(name).ok().and_then(|x| x.get::<bool>()) {
            if value == current_value {
                return;
            }
        }
        let _ = self.set_property(name, &value);
    }
    pub fn change_hightlight(&mut self, value: bool) {
        let name = "highlight";
        if let Some(current_value) = self.get_property(name).ok().and_then(|x| x.get::<bool>()) {
            if value == current_value {
                return;
            }
        }
        let _ = self.set_property(name, &value);
    }
    pub fn change_key(&mut self, value: u64) {
        let name = "key";
        if let Some(current_value) = self.get_property(name).ok().and_then(|x| x.get::<u64>()) {
            if value == current_value {
                return;
            }
        }
        let _ = self.set_property(name, &value);
    }
}

// Implment conversion from rust struct to SidebarRow
use fractal_api::types::Room;
impl<'a> From<&'a Room> for SidebarRow {
    fn from(room: &Room) -> SidebarRow {
        let n = room.notifications.to_string();
        let notifications = if room.inv {
            Some("●")
        } else if room.notifications != 0 {
            Some(n.as_str())
        } else {
            None
        };
        SidebarRow::new(
            &room.id,
            room.name.as_ref().map(|x| x.as_str()),
            room.avatar.as_ref().map(|x| x.as_str()),
            notifications,
            room.direct,
            false, // Default value for bold
            room.highlight > 0 || room.inv,
            room.messages
                .last()
                .map_or(0, |m| m.date.timestamp() as u64),
        )
    }
}

use gio::ListStoreExtManual;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
pub struct Sidebar {
    invites: gio::ListStore,
    favorites: gio::ListStore,
    rooms: gio::ListStore,
    low_priority: gio::ListStore,
    // FIXME: store a weak reference to SidebarRow directly not as gObject
    store: HashMap<String, glib::WeakRef<glib::Object>>,
    // We need an Arc<Mutex> because connect_notify() is sync (thread-safe)
    // Rc<Cell> would be better because it has a smaller overhead
    selected: Arc<Mutex<Option<SelectedData>>>,
}

struct SelectedData {
    listbox: glib::object::SendWeakRef<gtk::ListBox>,
    row: glib::object::SendWeakRef<gtk::ListBoxRow>,
    room_id: String,
}

impl SelectedData {
    pub fn new(listbox: &gtk::ListBox, row: &gtk::ListBoxRow) -> Option<Self> {
        let room_id = row
            .get_action_target_value()
            .and_then(|x| x.get::<String>())?;
        Some(SelectedData {
            listbox: listbox.downgrade().into(),
            row: row.downgrade().into(),
            room_id,
        })
    }
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
            store: HashMap::new(),
            selected: Arc::new(Mutex::new(None)),
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
    pub fn add_room(&mut self, room: &Room) {
        let sort_methode = |a: &glib::Object, b: &glib::Object| {
            if let Some(key_a) = a.get_property("key").ok().map(|x| x.get::<u64>()) {
                if let Some(key_b) = b.get_property("key").ok().map(|x| x.get::<u64>()) {
                    return key_b.cmp(&key_a);
                }
            }
            Ordering::Less
        };

        let row: SidebarRow = room.into();
        if room.fav {
            // favorites
            self.favorites.append(&row);
        } else if room.inv {
            // invites
            self.invites.append(&row);
        } else {
            // Normal rooms
            self.rooms.insert_sorted(&row, sort_methode);
            // Connect to key notify, so we can reorder the liststore when the value changes
            let store_weak: glib::object::SendWeakRef<gio::ListStore> =
                self.rooms.downgrade().into();
            let selected = Arc::downgrade(&self.selected);
            row.connect_notify("key", move |_, _| {
                let store = upgrade_weak!(store_weak);
                // We need to reset the selection after sort
                let selected = upgrade_weak!(selected);
                let selected_unwraped = { selected.lock().unwrap().take() };
                store.sort(sort_methode);
                if let Some(old) = selected_unwraped {
                    let old_listbox = upgrade_weak!(old.listbox);
                    if let Some(position) = get_position_by_id(&store, &old.room_id) {
                        let row = old_listbox.get_row_at_index(position as i32);
                        old_listbox.select_row(&row);
                    }
                }
            });
        }
        //TODO:: popolate low_priority
        let obj = row.upcast::<glib::Object>();
        self.store.insert(room.id.clone(), obj.downgrade());
    }

    // Remove room to the correct liststore
    pub fn remove_room(&mut self, id: &str) {
        // favorites
        if let Some(position) = get_position_by_id(&self.favorites, &id) {
            self.favorites.remove(position);
        }
        // invites
        if let Some(position) = get_position_by_id(&self.invites, &id) {
            self.invites.remove(position);
        }
        // Normal rooms
        if let Some(position) = get_position_by_id(&self.rooms, &id) {
            self.rooms.remove(position);
        }
        self.store.remove(id);
    }

    pub fn remove_all(&mut self) {
        self.invites.remove_all();
        self.favorites.remove_all();
        self.rooms.remove_all();
        self.low_priority.remove_all();
        self.store.clear();
    }

    // Update all properties for a row in the sidebar
    pub fn update_room(&self, room: &Room) {
        let obj = self.store.get(&room.id).and_then(|obj| obj.upgrade());
        if let Some(mut row) = obj.and_then(|obj| obj.downcast::<SidebarRow>().ok()) {
            let n = room.notifications.to_string();
            let notifications = if room.inv {
                Some("●")
            } else if room.notifications != 0 {
                Some(n.as_str())
            } else {
                None
            };
            row.change_name(room.name.as_ref().map(|x| x.as_str()));
            row.change_avatar(room.avatar.as_ref().map(|x| x.as_str()));
            row.change_notifications(notifications);
            row.change_direct(room.direct);
            row.change_bold(false);
            row.change_hightlight(room.highlight > 0 || room.inv);
            row.change_key(
                room.messages
                    .last()
                    .map_or(0, |m| m.date.timestamp() as u64),
            );
        }
    }

    // Allow only one row to be selected
    pub fn connect_selection(&self, listbox: &gtk::ListBox) {
        let selected = Arc::downgrade(&self.selected);
        listbox.connect_row_selected(move |listbox, row| {
            let selected = upgrade_weak!(selected);
            let selected_unwraped = { selected.lock().unwrap().take() };
            if let Some(old) = selected_unwraped {
                let old_listbox = upgrade_weak!(old.listbox);
                let old_row = upgrade_weak!(old.row);
                old_listbox.unselect_row(&old_row);
            }
            if let Some(row) = row {
                *selected.lock().unwrap() = SelectedData::new(&listbox, &row);
            }
        });
    }

    pub fn filter(&self, filter: &str) {
        let filter = filter.to_lowercase();
        for (_, obj) in self.store.iter() {
            if let Some(obj) = obj.upgrade() {
                if let Some(value) = obj
                    .get_property("name")
                    .ok()
                    .and_then(|x| x.get::<String>())
                {
                    let filter = !value.to_lowercase().contains(&filter);
                    let _ = obj.set_property("hidden", &filter);
                }
            }
        }
    }
}

use gio::ListModelExt;
fn get_position_by_id(store: &gio::ListStore, id: &str) -> Option<u32> {
    let mut i = 0;
    while i < store.get_n_items() {
        let obj = store.get_object(i)?;
        if obj.get_property("room_id").ok()?.get::<String>()? == id {
            return Some(i);
        }
        i += 1;
    }
    None
}
