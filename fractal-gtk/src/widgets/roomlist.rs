extern crate chrono;
extern crate url;
extern crate gtk;
extern crate pango;
extern crate gdk;
extern crate gettextrs;

use glib;
use self::gdk::DragContextExtManual;

use self::url::Url;
use std::collections::HashMap;
use self::gtk::prelude::*;
use self::gettextrs::gettext;

use widgets::roomrow::RoomRow;
use types::Room;
use types::Message;
use std::sync::{Arc, Mutex, MutexGuard};

use self::chrono::prelude::*;


fn get_url(url: Option<String>) -> Url {
    let defurl = Url::parse("https://matrix.org").unwrap();

    match url {
        Some(u) => {
            match Url::parse(&u) {
                Ok(url) => url,
                Err(_) => defurl,
            }
        }
        None => defurl,
    }
}


pub struct RoomUpdated {
    pub room: Room,
    pub updated: DateTime<Local>,
}

impl RoomUpdated {
    pub fn new(room: Room) -> RoomUpdated {
        let updated = match room.messages.last() {
            Some(l) => l.date,
            None => Message::default().date,
        };

        RoomUpdated {
            room,
            updated,
        }
    }

    pub fn up(&mut self) {
        self.updated = Local::now();
    }
}

pub struct RoomListGroup {
    pub rooms: HashMap<String, RoomRow>,
    pub baseu: Url,
    pub list: gtk::ListBox,
    rev: gtk::Revealer,
    arrow: gtk::Image,
    expanded: Arc<Mutex<bool>>,
    title: gtk::Label,
    empty: gtk::Label,
    title_eb: gtk::EventBox,

    wbox: gtk::Box,
    pub widget: gtk::EventBox,

    roomvec: Arc<Mutex<Vec<RoomUpdated>>>,
    filter: Option<String>,
}

impl RoomListGroup {
    pub fn new(url: &Url, name: &str, empty_text: &str) -> RoomListGroup {
        let list = gtk::ListBox::new();
        let baseu = url.clone();
        let rooms = HashMap::new();
        let roomvec = Arc::new(Mutex::new(vec![]));

        let empty = gtk::Label::new(empty_text);
        empty.set_line_wrap_mode(pango::WrapMode::WordChar);
        empty.set_line_wrap(true);
        empty.set_justify(gtk::Justification::Center);
        if let Some(style) = empty.get_style_context() {
            style.add_class("room-empty-text");
        }

        let rev = gtk::Revealer::new();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
        b.add(&empty);
        b.add(&list);

        rev.add(&b);
        rev.set_reveal_child(true);

        let title = gtk::Label::new(name);
        title.set_halign(gtk::Align::Start);
        title.set_valign(gtk::Align::Start);
        let arrow = gtk::Image::new_from_icon_name("pan-down-symbolic", 2);
        let expanded = Arc::new(Mutex::new(true));
        let title_eb = gtk::EventBox::new();

        title_eb.connect_button_press_event(clone!(list, arrow, rev, expanded => move |_, _| {
            match *expanded.lock().unwrap() {
                true => {
                    arrow.set_from_icon_name("pan-end-symbolic", 2);
                    rev.set_reveal_child(false);
                    if let Some(style) = list.get_style_context() {
                        style.add_class("collapsed");
                    }
                }
                false => {
                    arrow.set_from_icon_name("pan-down-symbolic", 2);
                    rev.set_reveal_child(true);
                    if let Some(style) = list.get_style_context() {
                        style.remove_class("collapsed");
                    }
                }
            };
            let exp = !(*expanded.lock().unwrap());
            *expanded.lock().unwrap() = exp;
            glib::signal::Inhibit(true)
        }));

        let widget = gtk::EventBox::new();
        let wbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add(&wbox);

        let filter= None;

        RoomListGroup {
            list,
            baseu,
            rooms,
            roomvec,
            rev,
            title,
            arrow,
            title_eb,
            widget,
            empty,
            wbox,
            expanded,
            filter,
        }
    }

    pub fn add_room(&mut self, r: Room) {
        if self.rooms.contains_key(&r.id) {
            // room added, we'll pass
            return;
        }

        let rid = r.id.clone();
        self.roomvec.lock().unwrap().push(RoomUpdated::new(r.clone()));

        let row = RoomRow::new(r, &self.baseu);
        self.list.add(&row.widget());

        self.rooms.insert(rid, row);
        self.show();
    }

    pub fn add_room_up(&mut self, r: RoomUpdated) {
        if self.rooms.contains_key(&r.room.id) {
            // room added, we'll pass
            return;
        }

        let rid = r.room.id.clone();
        let mut rv = self.roomvec.lock().unwrap();
        let mut pos = rv.len();
        for (i, ru) in rv.iter().enumerate() {
            if ru.updated < r.updated {
                pos = i;
                break;
            }
        }

        rv.insert(pos, RoomUpdated::new(r.room.clone()));

        let row = RoomRow::new(r.room, &self.baseu);
        self.list.insert(&row.widget(), pos as i32);

        self.rooms.insert(rid, row);
        self.show();
    }

    pub fn set_bold(&mut self, room: String, bold: bool) {
        if let Some(ref mut r) = self.rooms.get_mut(&room) {
            r.set_bold(bold);
        }
    }

    pub fn set_room_notifications(&mut self, room: String, n: i32, h: i32) {
        if let Some(ref mut r) = self.rooms.get_mut(&room) {
            r.set_notifications(n, h);
        }

        self.edit_room(&room, move |rv| {
            rv.room.notifications = n;
            rv.room.highlight = h;
        });
    }

    pub fn remove_room(&mut self, room: String) -> Option<RoomUpdated> {
        self.rooms.remove(&room);
        let mut rv = self.roomvec.lock().unwrap();
        if let Some(idx) = rv.iter().position(|x| { x.room.id == room}) {
            if let Some(row) = self.list.get_row_at_index(idx as i32) {
                self.list.remove(&row);
            }
            self.show();
            return Some(rv.remove(idx));
        }

        None
    }

    pub fn rename_room(&mut self, room: String, newname: Option<String>) {
        if let (Some(r), Some(n)) = (self.rooms.get_mut(&room), newname.clone()) {
            r.set_name(n);
        }

        self.edit_room(&room, move |rv| { rv.room.name = newname.clone(); });
    }

    pub fn set_room_avatar(&mut self, room: String, av: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&room) {
            r.set_avatar(av.clone());
        }

        self.edit_room(&room, move |rv| { rv.room.avatar = av.clone(); });
    }

    pub fn widget(&self) -> gtk::EventBox {
        let b = self.wbox.clone();
        if let Some(style) = b.get_style_context() {
            style.add_class("room-list");
            style.add_class("sidebar");
        }

        // building the heading
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        if let Some(style) = hbox.get_style_context() {
            style.add_class("room-title");
        }
        hbox.pack_start(&self.title, true, true, 0);
        hbox.pack_start(&self.arrow, false, false, 0);

        for ch in self.title_eb.get_children() {
            self.title_eb.remove(&ch);
        }
        self.title_eb.add(&hbox);

        self.arrow.set_from_icon_name("pan-down-symbolic", 2);
        *self.expanded.lock().unwrap() = true;
        self.rev.set_reveal_child(true);
        if let Some(style) = self.list.get_style_context() {
            style.remove_class("collapsed");
        }

        b.pack_start(&self.title_eb, false, false, 0);
        b.pack_start(&self.rev, true, true, 0);

        self.show();

        self.widget.clone()
    }

    pub fn show(&self) {
        self.widget.show_all();
        if self.rooms.is_empty() {
            self.empty.show();
            self.list.hide();
        } else {
            self.list.show();
            self.empty.hide();
        }
        self.render_notifies();
    }

    pub fn hide(&self) {
        self.widget.hide();
    }

    pub fn connect<F: Fn(Room) + 'static>(&self, cb: F) {
        let rs = self.roomvec.clone();
        self.list.connect_row_activated(move |_, row| {
            let idx = row.get_index();
            cb(rs.lock().unwrap()[idx as usize].room.clone());
        });
    }

    pub fn get_selected(&self) -> Option<String> {
        let rv = self.roomvec.lock().unwrap();
        match self.list.get_selected_row() {
            Some(row) => Some(rv[row.get_index() as usize].room.id.clone()),
            None => None,
        }
    }

    pub fn set_selected(&self, room: Option<String>) {
        self.list.unselect_all();

        if room.is_none() {
            return;
        }

        let room = room.unwrap();

        let rv = self.roomvec.lock().unwrap();
        if let Some(idx) = rv.iter().position(|x| { x.room.id == room}) {
            if let Some(ref row) = self.list.get_row_at_index(idx as i32) {
                self.list.select_row(row);
            }
        }
    }

    pub fn add_rooms(&mut self, mut array: Vec<Room>) {
        array.sort_by_key(|ref x| {
            match x.messages.last() {
                Some(l) => l.date,
                None => Message::default().date,
            }
        });

        for r in array.iter().rev() {
            self.add_room(r.clone());
        }
    }

    pub fn moveup(&mut self, room: String) {
        let s = self.get_selected();

        self.edit_room(&room, move |rv| { rv.up(); });
        if let Some(r) = self.remove_room(room) {
            self.add_room_up(r);
        }

        self.set_selected(s);
        let term = self.filter.clone();
        self.filter_rooms(&term);
    }

    fn render_notifies(&self) {
        for (_k, r) in self.rooms.iter() {
            r.render_notifies();
        }
    }

    fn edit_room<F: Fn(&mut RoomUpdated) + 'static>(&mut self, room: &str, cb: F) {
        let mut rv = self.roomvec.lock().unwrap();
        if let Some(idx) = rv.iter().position(|x| { x.room.id == room}) {
            if let Some(ref mut m) = rv.get_mut(idx) {
                cb(m);
            }
        }
    }

    pub fn filter_rooms(&mut self, term: &Option<String>) {
        self.filter = term.clone();

        for (i, r) in self.roomvec.lock().unwrap().iter().enumerate() {
            if let Some(row) = self.list.get_row_at_index(i as i32) {
                match term {
                    &Some(ref t) if !t.is_empty() => {
                        let rname = r.room.name.clone()
                                     .unwrap_or("".to_string())
                                     .to_lowercase();
                        if rname.contains(&t.to_lowercase()) {
                            row.show();
                        } else {
                            row.hide();
                        }
                    }
                    _ => { row.show(); }
                };
            }
        }
    }
}

#[derive(Clone)]
struct RGroup {
    g: Arc<Mutex<RoomListGroup>>,
}

impl RGroup {
    pub fn new(url: &Url, name: &str, empty_text: &str) -> RGroup {
        let r = RoomListGroup::new(url, name, empty_text);
        RGroup{ g: Arc::new(Mutex::new(r)) }
    }

    pub fn get(&self) -> MutexGuard<RoomListGroup> {
        self.g.lock().unwrap()
    }
}

pub struct RoomList {
    pub baseu: Url,
    widget: gtk::Box,

    inv: RGroup,
    fav: RGroup,
    rooms: RGroup,
}

macro_rules! run_in_group {
    ($self: expr, $roomid: expr, $fn: ident, $($arg: expr),*) => {{
        if $self.inv.get().rooms.contains_key($roomid) {
            $self.inv.get().$fn($($arg),*)
        } else if $self.fav.get().rooms.contains_key($roomid) {
            $self.fav.get().$fn($($arg),*)
        } else {
            $self.rooms.get().$fn($($arg),*)
        }
    }}
}

impl RoomList {
    pub fn new(url: Option<String>) -> RoomList {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let baseu = get_url(url);

        let inv = RGroup::new(&baseu, gettext("Invites").as_str(),
                              gettext("You don't have any invitations").as_str());
        let fav = RGroup::new(&baseu, gettext("Favorites").as_str(),
                              gettext("Drag and drop rooms here to \
                                       add them to your favorites").as_str());
        let rooms = RGroup::new(&baseu, gettext("Rooms").as_str(),
                                gettext("You don't have any rooms yet").as_str());

        let rl = RoomList {
            baseu,
            widget,
            inv,
            fav,
            rooms,
        };

        rl
    }

    pub fn set_selected(&self, room: Option<String>) {
        self.inv.get().set_selected(None);
        self.fav.get().set_selected(None);
        self.rooms.get().set_selected(None);

        if let Some(r) = room {
            run_in_group!(self, &r, set_selected, Some(r.clone()));
        }
    }

    pub fn get_selected(&self) -> Option<String> {
        for i in [&self.inv, &self.fav, &self.rooms].iter() {
            if let Some(s) = i.get().get_selected() {
                return Some(s.clone());
            }
        }
        None
    }

    pub fn add_rooms(&mut self, array: Vec<Room>) {
        self.inv.get().add_rooms(array.iter().filter(|r| r.inv).cloned().collect::<Vec<Room>>());
        self.fav.get().add_rooms(array.iter().filter(|r| r.fav).cloned().collect::<Vec<Room>>());
        self.rooms.get().add_rooms(array.iter().filter(|r| !r.fav && !r.inv).cloned().collect::<Vec<Room>>());
        self.show_and_hide();
    }

    pub fn connect<F: Fn(Room) + 'static>(&self, cb: F) {
        let acb = Arc::new(cb);

        let cb = acb.clone();
        self.inv.get().connect(move |room| cb(room));
        let cb = acb.clone();
        self.fav.get().connect(move |room| cb(room));
        let cb = acb.clone();
        self.rooms.get().connect(move |room| cb(room));
    }

    pub fn connect_fav<F: Fn(Room, bool) + 'static>(&self, cb: F) {
        let acb = Arc::new(cb);

        let favw = self.fav.get().widget.clone();
        let r = self.rooms.clone();
        let f = self.fav.clone();
        let cb = acb.clone();
        self.connect_drop(favw, move |roomid| {
            if let Some(room) = r.get().remove_room(roomid) {
                cb(room.room.clone(), true);
                f.get().add_room_up(room);
            }
        });

        let rw = self.rooms.get().widget.clone();
        let r = self.rooms.clone();
        let f = self.fav.clone();
        let cb = acb.clone();
        self.connect_drop(rw, move |roomid| {
            if let Some(room) = f.get().remove_room(roomid) {
                cb(room.room.clone(), false);
                r.get().add_room_up(room);
            }
        });
    }

    pub fn set_room_avatar(&mut self, room: String, av: Option<String>) {
        run_in_group!(self, &room, set_room_avatar, room, av);
    }

    pub fn set_room_notifications(&mut self, room: String, n: i32, h: i32) {
        run_in_group!(self, &room, set_room_notifications, room, n, h);
    }

    pub fn remove_room(&mut self, room: String) -> Option<RoomUpdated> {
        let ret = run_in_group!(self, &room, remove_room, room);
        self.show_and_hide();
        ret
    }

    pub fn set_bold(&mut self, room: String, bold: bool) {
        run_in_group!(self, &room, set_bold, room, bold)
    }

    pub fn add_room(&mut self, r: Room) {
        if r.inv {
            self.inv.get().add_room(r);
        } else if r.fav {
            self.fav.get().add_room(r);
        } else {
            self.rooms.get().add_room(r);
        }
        self.show_and_hide();
    }

    pub fn rename_room(&mut self, room: String, newname: Option<String>) {
        run_in_group!(self, &room, rename_room, room, newname);
    }

    pub fn moveup(&mut self, room: String) {
        run_in_group!(self, &room, moveup, room);
    }

    pub fn widget(&self) -> gtk::Box {
        self.connect_select();

        for ch in self.widget.get_children() {
            self.widget.remove(&ch);
        }
        self.widget.add(&self.inv.get().widget());
        self.widget.add(&self.fav.get().widget());
        self.widget.add(&self.rooms.get().widget());

        self.show_and_hide();

        self.widget.clone()
    }

    pub fn show_and_hide(&self) {
        self.widget.show_all();

        if self.inv.get().rooms.is_empty() {
            self.inv.get().hide();
        } else {
            self.inv.get().show();
        }

        self.fav.get().show();
        self.rooms.get().show();
    }

    pub fn connect_select(&self) {
        let inv = self.inv.clone();
        let rooms = self.rooms.clone();
        self.fav.get().list.connect_row_activated(move |_, _| {
            inv.get().set_selected(None);
            rooms.get().set_selected(None);
        });

        let inv = self.inv.clone();
        let fav = self.fav.clone();
        self.rooms.get().list.connect_row_activated(move |_, _| {
            inv.get().set_selected(None);
            fav.get().set_selected(None);
        });
    }

    pub fn connect_drop<F: Fn(String) + 'static>(&self, widget: gtk::EventBox, cb: F) {
        let flags = gtk::DestDefaults::empty();
        let action = gdk::DragAction::all();
        widget.drag_dest_set(flags, &[], action);
        widget.drag_dest_add_text_targets();
        widget.connect_drag_motion(move |_w, ctx, _x, _y, time| {
            ctx.drag_status(gdk::DragAction::MOVE, time);
            glib::signal::Inhibit(true)
        });
        widget.connect_drag_drop(move |w, ctx, _x, _y, time| {
            if let Some(target) = w.drag_dest_find_target(ctx, None) {
                w.drag_get_data(ctx, &target, time);
            }
            glib::signal::Inhibit(true)
        });
        widget.connect_drag_data_received(move |_w, _ctx, _x, _y, data, _info, _time| {
            if let Some(roomid) = data.get_text() {
                cb(roomid);
            }
        });
    }

    pub fn filter_rooms(&self, term: Option<String>) {
        self.inv.get().filter_rooms(&term);
        self.fav.get().filter_rooms(&term);
        self.rooms.get().filter_rooms(&term);
    }
}
