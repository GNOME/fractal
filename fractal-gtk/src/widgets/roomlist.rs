use crate::i18n::i18n;
use fractal_api::clone;
use fractal_api::identifiers::RoomId;

use fractal_api::url::Url;
use gtk::prelude::*;
use log::info;
use std::collections::HashMap;

use crate::globals;
use crate::types::{Room, RoomTag};
use crate::widgets::roomrow::RoomRow;
use std::convert::TryFrom;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::prelude::*;

pub struct RoomUpdated {
    pub room: Room,
    pub updated: DateTime<Local>,
}

impl RoomUpdated {
    pub fn new(room: Room) -> RoomUpdated {
        let updated = match room.messages.last() {
            Some(l) => l.date,
            None => Local.ymd(1970, 1, 1).and_hms(0, 0, 0),
        };

        RoomUpdated { room, updated }
    }

    pub fn up(&mut self) {
        self.updated = Local::now();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoomListType {
    Invites,
    Rooms,
    Favorites,
}

pub struct RoomListGroup {
    pub rooms: HashMap<RoomId, RoomRow>,
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

        let empty = gtk::Label::new(Some(empty_text));
        empty.set_line_wrap_mode(pango::WrapMode::WordChar);
        empty.set_line_wrap(true);
        empty.set_justify(gtk::Justification::Center);
        empty.get_style_context().add_class("room-empty-text");

        let rev = gtk::Revealer::new();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
        b.add(&empty);
        b.add(&list);

        rev.add(&b);
        rev.set_reveal_child(true);

        let title = gtk::Label::new(Some(name));
        title.set_halign(gtk::Align::Start);
        title.set_valign(gtk::Align::Start);
        let arrow =
            gtk::Image::new_from_icon_name(Some("pan-down-symbolic"), gtk::IconSize::SmallToolbar);
        let expanded = Arc::new(Mutex::new(true));
        let title_eb = gtk::EventBox::new();

        title_eb.connect_button_press_event(clone!(list, arrow, rev, expanded => move |_, _| {
            if *expanded.lock().unwrap() {
                arrow.set_from_icon_name(Some("pan-end-symbolic"), gtk::IconSize::SmallToolbar);
                rev.set_reveal_child(false);
                list.get_style_context().add_class("collapsed");
            } else {
                arrow.set_from_icon_name(Some("pan-down-symbolic"), gtk::IconSize::SmallToolbar);
                rev.set_reveal_child(true);
                list.get_style_context().remove_class("collapsed");
            }
            let exp = !(*expanded.lock().unwrap());
            *expanded.lock().unwrap() = exp;
            glib::signal::Inhibit(true)
        }));

        let widget = gtk::EventBox::new();
        let wbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add(&wbox);

        let filter = None;

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
        self.roomvec
            .lock()
            .unwrap()
            .push(RoomUpdated::new(r.clone()));

        let row = RoomRow::new(r);
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

        let row = RoomRow::new(r.room);
        self.list.insert(&row.widget(), pos as i32);

        self.rooms.insert(rid, row);
        self.show();
    }

    pub fn set_bold(&mut self, room_id: RoomId, bold: bool) {
        if let Some(ref mut r) = self.rooms.get_mut(&room_id) {
            r.set_bold(bold);
        }
    }

    pub fn rooms_with_notifications(&self) -> usize {
        self.rooms
            .iter()
            .filter(|(_, r)| r.room.notifications > 0 || r.room.highlight > 0)
            .count()
    }

    pub fn set_room_notifications(&mut self, room_id: RoomId, n: i32, h: i32) {
        if let Some(ref mut r) = self.rooms.get_mut(&room_id) {
            r.set_notifications(n, h);
        }

        self.edit_room(&room_id, move |rv| {
            rv.room.notifications = n;
            rv.room.highlight = h;
        });
    }

    pub fn remove_room(&mut self, room_id: RoomId) -> Option<RoomUpdated> {
        self.rooms.remove(&room_id);
        let mut rv = self.roomvec.lock().unwrap();
        if let Some(idx) = rv.iter().position(|x| x.room.id == room_id) {
            if let Some(row) = self.list.get_row_at_index(idx as i32) {
                self.list.remove(&row);
            }
            self.show();
            return Some(rv.remove(idx));
        }

        None
    }

    pub fn rename_room(&mut self, room_id: RoomId, newname: Option<String>) {
        if let (Some(r), Some(n)) = (self.rooms.get_mut(&room_id), newname.clone()) {
            r.set_name(n);
        }

        self.edit_room(&room_id, move |rv| {
            rv.room.name = newname.clone();
        });
    }

    pub fn set_room_avatar(&mut self, room_id: RoomId, av: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&room_id) {
            r.set_avatar(av.clone());
        }

        self.edit_room(&room_id, move |rv| {
            rv.room.avatar = av.clone();
        });
    }

    pub fn widget(&self) -> &gtk::EventBox {
        let b = self.wbox.clone();
        let b_ctx = b.get_style_context();
        b_ctx.add_class("room-list");
        b_ctx.add_class("sidebar");

        // building the heading
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        hbox.get_style_context().add_class("room-title");
        hbox.pack_start(&self.title, true, true, 0);
        hbox.pack_start(&self.arrow, false, false, 0);

        for ch in self.title_eb.get_children() {
            self.title_eb.remove(&ch);
        }
        self.title_eb.add(&hbox);

        self.arrow
            .set_from_icon_name(Some("pan-down-symbolic"), gtk::IconSize::SmallToolbar);
        *self.expanded.lock().unwrap() = true;
        self.rev.set_reveal_child(true);
        self.list.get_style_context().remove_class("collapsed");

        b.pack_start(&self.title_eb, false, false, 0);
        b.pack_start(&self.rev, true, true, 0);

        self.show();

        &self.widget
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

    pub fn get_selected(&self) -> Option<RoomId> {
        let rv = self.roomvec.lock().unwrap();
        self.list
            .get_selected_row()
            .map(|row| rv[row.get_index() as usize].room.id.clone())
    }

    pub fn set_selected(&self, room_id: Option<RoomId>) {
        self.list.unselect_all();

        if let Some(room_id) = room_id {
            let rv = self.roomvec.lock().unwrap();
            if let Some(idx) = rv.iter().position(|x| x.room.id == room_id) {
                if let Some(ref row) = self.list.get_row_at_index(idx as i32) {
                    self.list.select_row(Some(row));
                }
            }
        }
    }

    /// Find the ID of a room after or before the current one in the list
    ///
    /// # Parameters
    ///
    /// - `unread_only`: true to only look for rooms with unread messages
    /// - `direction`: `-1` for the previous room, `+1` for the next
    ///
    /// # Return value
    ///
    /// `(Room id if found, go to previous group, go to next group)`
    fn sibling_id(&self, unread_only: bool, direction: i32) -> (Option<RoomId>, bool, bool) {
        match self.list.get_selected_row() {
            Some(row) => {
                let rv = self.roomvec.lock().unwrap();
                let mut idx = row.get_index() + direction;
                while unread_only
                    && idx >= 0
                    && (idx as usize) < rv.len()
                    && rv[idx as usize].room.notifications == 0
                {
                    idx += direction;
                }

                if idx >= 0 && (idx as usize) < rv.len() {
                    (Some(rv[idx as usize].room.id.clone()), false, false)
                } else {
                    (None, idx < 0, idx >= 0)
                }
            }
            None => (None, false, false),
        }
    }

    fn first_id(&self, unread_only: bool) -> Option<RoomId> {
        self.roomvec
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                if unread_only {
                    r.room.notifications > 0
                } else {
                    true
                }
            })
            .next()
            .map(|r| r.room.id.clone())
    }

    fn last_id(&self, unread_only: bool) -> Option<RoomId> {
        self.roomvec
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                if unread_only {
                    r.room.notifications > 0
                } else {
                    true
                }
            })
            .last()
            .map(|r| r.room.id.clone())
    }

    pub fn add_rooms(&mut self, mut array: Vec<Room>) {
        array.sort_by_key(|ref x| match x.messages.last() {
            Some(l) => l.date,
            None => Local.ymd(1970, 1, 1).and_hms(0, 0, 0),
        });

        for r in array.iter().rev() {
            self.add_room(r.clone());
        }
    }

    pub fn moveup(&mut self, room_id: RoomId) {
        let s = self.get_selected();

        self.edit_room(&room_id, move |rv| {
            rv.up();
        });
        if let Some(r) = self.remove_room(room_id) {
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

    fn edit_room<F: Fn(&mut RoomUpdated) + 'static>(&mut self, room_id: &RoomId, cb: F) {
        let mut rv = self.roomvec.lock().unwrap();
        if let Some(idx) = rv.iter().position(|x| x.room.id == *room_id) {
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
                        let rname = r.room.name.clone().unwrap_or_default().to_lowercase();
                        if rname.contains(&t.to_lowercase()) {
                            row.show();
                        } else {
                            row.hide();
                        }
                    }
                    _ => {
                        row.show();
                    }
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
        RGroup {
            g: Arc::new(Mutex::new(r)),
        }
    }

    pub fn get(&self) -> MutexGuard<'_, RoomListGroup> {
        self.g.lock().unwrap()
    }
}

pub struct RoomList {
    pub baseu: Url,
    widget: gtk::Box,
    adj: Option<gtk::Adjustment>,

    inv: RGroup,
    fav: RGroup,
    rooms: RGroup,
}

macro_rules! run_in_group {
    ($self: expr, $room_id: expr, $fn: ident, $($arg: expr),*) => {{
        if $self.inv.get().rooms.contains_key($room_id) {
            $self.inv.get().$fn($($arg),*)
        } else if $self.fav.get().rooms.contains_key($room_id) {
            $self.fav.get().$fn($($arg),*)
        } else {
            $self.rooms.get().$fn($($arg),*)
        }
    }}
}

impl RoomList {
    pub fn new(adj: Option<gtk::Adjustment>, url: Option<Url>) -> RoomList {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let baseu = url.unwrap_or(globals::DEFAULT_HOMESERVER.clone());

        let inv = RGroup::new(
            &baseu,
            i18n("Invites").as_str(),
            i18n("You don’t have any invitations").as_str(),
        );
        let fav = RGroup::new(
            &baseu,
            i18n("Favorites").as_str(),
            i18n("Drag and drop rooms here to add them to your favorites").as_str(),
        );
        let rooms = RGroup::new(
            &baseu,
            i18n("Rooms").as_str(),
            i18n("You don’t have any rooms yet").as_str(),
        );

        let rl = RoomList {
            baseu,
            widget,
            adj,
            inv,
            fav,
            rooms,
        };

        rl
    }

    pub fn select(&self, room_id: &RoomId) {
        run_in_group!(self, room_id, set_selected, Some(room_id.clone()));
    }

    fn sibling_id_inv(&self, unread_only: bool, direction: i32) -> Option<RoomId> {
        let (room_id, _, next) = self.inv.get().sibling_id(unread_only, direction);

        if let Some(room_id) = room_id {
            Some(room_id)
        } else if next {
            self.fav.get().first_id(unread_only)
        } else {
            self.sibling_id_fav(unread_only, direction)
        }
    }

    fn sibling_id_fav(&self, unread_only: bool, direction: i32) -> Option<RoomId> {
        let (room_id, prev, next) = self.fav.get().sibling_id(unread_only, direction);

        if let Some(room_id) = room_id {
            Some(room_id)
        } else if prev {
            self.inv.get().last_id(unread_only)
        } else if next {
            self.rooms.get().first_id(unread_only)
        } else {
            self.sibling_id_rooms(unread_only, direction)
        }
    }

    fn sibling_id_rooms(&self, unread_only: bool, direction: i32) -> Option<RoomId> {
        let (room_id, prev, _) = self.rooms.get().sibling_id(unread_only, direction);

        if let Some(room_id) = room_id {
            Some(room_id)
        } else if prev {
            self.fav.get().last_id(unread_only)
        } else {
            None
        }
    }

    fn sibling_id(&self, unread_only: bool, direction: i32) -> Option<RoomId> {
        self.sibling_id_inv(unread_only, direction)
    }

    pub fn next_id(&self) -> Option<RoomId> {
        self.sibling_id(false, 1)
    }

    pub fn prev_id(&self) -> Option<RoomId> {
        self.sibling_id(false, -1)
    }

    pub fn next_unread_id(&self) -> Option<RoomId> {
        self.sibling_id(true, 1)
    }

    pub fn prev_unread_id(&self) -> Option<RoomId> {
        self.sibling_id(true, -1)
    }

    pub fn first_id(&self) -> Option<RoomId> {
        self.inv
            .get()
            .first_id(false)
            .or_else(|| self.fav.get().first_id(false))
            .or_else(|| self.rooms.get().first_id(false))
    }

    pub fn last_id(&self) -> Option<RoomId> {
        self.rooms
            .get()
            .last_id(false)
            .or_else(|| self.fav.get().last_id(false))
            .or_else(|| self.inv.get().last_id(false))
    }

    pub fn unselect(&self) {
        self.inv.get().set_selected(None);
        self.fav.get().set_selected(None);
        self.rooms.get().set_selected(None);
    }

    pub fn add_rooms(&mut self, array: Vec<Room>) {
        self.inv.get().add_rooms(
            array
                .iter()
                .filter(|r| r.membership.is_invited())
                .cloned()
                .collect::<Vec<Room>>(),
        );
        self.fav.get().add_rooms(
            array
                .iter()
                .filter(|r| r.membership.match_joined_tag(RoomTag::Favourite))
                .cloned()
                .collect::<Vec<Room>>(),
        );
        self.rooms.get().add_rooms(
            array
                .iter()
                .filter(|r| !r.membership.match_joined_tag(RoomTag::Favourite))
                .cloned()
                .collect::<Vec<Room>>(),
        );
        self.show_and_hide();
    }

    pub fn connect_fav<F: Fn(Room, bool) + 'static>(&self, cb: F) {
        let acb = Arc::new(cb);

        let favw = self.fav.get().widget.clone();
        let r = self.rooms.clone();
        let f = self.fav.clone();
        let cb = acb.clone();
        self.connect_drop(favw, move |room_id| {
            if let Some(room) = r.get().remove_room(room_id) {
                cb(room.room.clone(), true);
                f.get().add_room_up(room);
            }
        });

        let rw = self.rooms.get().widget.clone();
        let r = self.rooms.clone();
        let f = self.fav.clone();
        let cb = acb;
        self.connect_drop(rw, move |roomid| {
            if let Some(room) = f.get().remove_room(roomid) {
                cb(room.room.clone(), false);
                r.get().add_room_up(room);
            }
        });
    }

    pub fn set_room_avatar(&mut self, room_id: RoomId, av: Option<String>) {
        run_in_group!(self, &room_id, set_room_avatar, room_id, av);
    }

    pub fn rooms_with_notifications(&self) -> usize {
        self.inv.get().rooms_with_notifications()
            + self.fav.get().rooms_with_notifications()
            + self.rooms.get().rooms_with_notifications()
    }

    pub fn set_room_notifications(&mut self, room_id: RoomId, n: i32, h: i32) {
        run_in_group!(self, &room_id, set_room_notifications, room_id, n, h);
    }

    pub fn remove_room(&mut self, room_id: RoomId) -> Option<RoomUpdated> {
        let ret = run_in_group!(self, &room_id, remove_room, room_id);
        self.show_and_hide();
        ret
    }

    pub fn set_bold(&mut self, room_id: RoomId, bold: bool) {
        run_in_group!(self, &room_id, set_bold, room_id, bold)
    }

    pub fn add_room(&mut self, r: Room) {
        if r.membership.is_invited() {
            self.inv.get().add_room(r);
        } else if r.membership.match_joined_tag(RoomTag::Favourite) {
            info!("We have fav rooms");
            self.fav.get().add_room(r);
        } else {
            info!("We have non fav rooms");
            self.rooms.get().add_room(r);
        }
        self.show_and_hide();
    }

    pub fn rename_room(&mut self, room_id: RoomId, newname: Option<String>) {
        run_in_group!(self, &room_id, rename_room, room_id, newname);
    }

    pub fn moveup(&mut self, room_id: RoomId) {
        run_in_group!(self, &room_id, moveup, room_id);
    }

    // Roomlist widget
    pub fn widget(&self) -> &gtk::Box {
        for ch in self.widget.get_children() {
            self.widget.remove(&ch);
        }
        self.widget.add(self.inv.get().widget());
        self.widget.add(self.fav.get().widget());
        self.widget.add(self.rooms.get().widget());
        self.connect_select();
        self.connect_keynav();

        self.show_and_hide();

        &self.widget
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

    // Connect handlers for unselecting rooms from other categories when a room is selected
    pub fn connect_select(&self) {
        let fav = self.fav.get().list.downgrade();
        let rooms = self.rooms.get().list.downgrade();
        self.inv.get().list.connect_row_selected(move |_, row| {
            if row.is_some() {
                upgrade_weak!(fav).unselect_all();
                upgrade_weak!(rooms).unselect_all();
            }
        });

        let inv = self.inv.get().list.downgrade();
        let rooms = self.rooms.get().list.downgrade();
        self.fav.get().list.connect_row_selected(move |_, row| {
            if row.is_some() {
                upgrade_weak!(inv).unselect_all();
                upgrade_weak!(rooms).unselect_all();
            }
        });

        let inv = self.inv.get().list.downgrade();
        let fav = self.fav.get().list.downgrade();
        self.rooms.get().list.connect_row_selected(move |_, row| {
            if row.is_some() {
                upgrade_weak!(inv).unselect_all();
                upgrade_weak!(fav).unselect_all();
            }
        });
    }

    pub fn connect_drop<F: Fn(RoomId) + 'static>(&self, widget: gtk::EventBox, cb: F) {
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
            if let Some(room_id) = data
                .get_text()
                .and_then(|rid| RoomId::try_from(rid.as_str()).ok())
            {
                cb(room_id);
            }
        });
    }

    pub fn connect_keynav(&self) {
        let weak_inv_lb = self.inv.get().list.downgrade();
        let weak_fav_lb = self.fav.get().list.downgrade();
        let weak_room_lb = self.rooms.get().list.downgrade();
        let adj = self.adj.clone();
        let type_ = RoomListType::Invites;
        self.inv.get().list.connect_keynav_failed(move |_, d| {
            let inv_lb = upgrade_weak!(weak_inv_lb, gtk::Inhibit(false));
            let fav_lb = upgrade_weak!(weak_fav_lb, gtk::Inhibit(false));
            let room_lb = upgrade_weak!(weak_room_lb, gtk::Inhibit(false));

            keynav_cb(d, &inv_lb, &fav_lb, &room_lb, adj.clone(), type_)
        });

        let weak_fav_lb = self.fav.get().list.downgrade();
        let weak_inv_lb = self.inv.get().list.downgrade();
        let weak_room_lb = self.rooms.get().list.downgrade();
        let adj = self.adj.clone();
        let type_ = RoomListType::Favorites;
        self.fav.get().list.connect_keynav_failed(move |_, d| {
            let fav_lb = upgrade_weak!(weak_fav_lb, gtk::Inhibit(false));
            let inv_lb = upgrade_weak!(weak_inv_lb, gtk::Inhibit(false));
            let room_lb = upgrade_weak!(weak_room_lb, gtk::Inhibit(false));

            keynav_cb(d, &inv_lb, &fav_lb, &room_lb, adj.clone(), type_)
        });

        let weak_rooms_lb = self.rooms.get().list.downgrade();
        let weak_inv_lb = self.inv.get().list.downgrade();
        let weak_fav_lb = self.fav.get().list.downgrade();
        let adj = self.adj.clone();
        let type_ = RoomListType::Rooms;
        self.rooms.get().list.connect_keynav_failed(move |_, d| {
            let rooms_lb = upgrade_weak!(weak_rooms_lb, gtk::Inhibit(false));
            let inv_lb = upgrade_weak!(weak_inv_lb, gtk::Inhibit(false));
            let fav_lb = upgrade_weak!(weak_fav_lb, gtk::Inhibit(false));

            keynav_cb(d, &inv_lb, &fav_lb, &rooms_lb, adj.clone(), type_)
        });
    }

    pub fn filter_rooms(&self, term: Option<String>) {
        self.inv.get().filter_rooms(&term);
        self.fav.get().filter_rooms(&term);
        self.rooms.get().filter_rooms(&term);
    }
}

/// Navigates between the different room
/// lists seamlessly with widget focus,
/// while keeping the `gtk::ScrolledWindow` in
/// the proper position.
///
/// Translated from https://gitlab.gnome.org/GNOME/gtk/blob/d3ad6425/gtk/inspector/general.c#L655
fn keynav_cb(
    direction: gtk::DirectionType,
    inv_lb: &gtk::ListBox,
    fav_lb: &gtk::ListBox,
    room_lb: &gtk::ListBox,
    adj: Option<gtk::Adjustment>,
    type_: RoomListType,
) -> gtk::Inhibit {
    let next: Option<&gtk::ListBox>;
    next = match (direction, type_) {
        (gtk::DirectionType::Down, RoomListType::Invites) => Some(fav_lb),
        (gtk::DirectionType::Down, RoomListType::Favorites) => Some(room_lb),
        (gtk::DirectionType::Up, RoomListType::Rooms) => Some(fav_lb),
        (gtk::DirectionType::Up, RoomListType::Favorites) => Some(inv_lb),
        _ => None,
    };

    if let Some(widget) = next {
        widget.child_focus(direction);
        gtk::Inhibit(true)
    } else if let Some(adjustment) = adj {
        let value = adjustment.get_value();
        let lower = adjustment.get_lower();
        let upper = adjustment.get_upper();
        let page = adjustment.get_page_size();

        match direction {
            gtk::DirectionType::Up if value > lower => {
                adjustment.set_value(lower);
                gtk::Inhibit(true)
            }
            gtk::DirectionType::Down if value < upper - page => {
                adjustment.set_value(upper - page);
                gtk::Inhibit(true)
            }
            _ => gtk::Inhibit(false),
        }
    } else {
        gtk::Inhibit(false)
    }
}
