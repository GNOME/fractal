use fractal_api::clone;
use std::cell::RefCell;
use std::collections::hash_map::HashMap;
use std::rc::Rc;

use glib::signal;
use gtk;
use gtk::prelude::*;

use crate::i18n::i18n;
use crate::types::Member;
use crate::widgets;
use crate::widgets::avatar::{AvatarExt, BadgeColor};

#[derive(Debug, Clone)]
pub struct MembersList {
    container: gtk::ListBox,
    search_entry: gtk::SearchEntry,
    error: gtk::Label,
    members: Vec<Member>,
    power_levels: HashMap<String, i32>,
}

impl MembersList {
    pub fn new(
        m: Vec<Member>,
        power_levels: HashMap<String, i32>,
        entry: gtk::SearchEntry,
    ) -> MembersList {
        MembersList {
            container: gtk::ListBox::new(),
            error: gtk::Label::new(None),
            members: m,
            search_entry: entry,
            power_levels: power_levels,
        }
    }

    /* creates a empty list with members.len() rows, the content will be loaded when the row is
     * drawn */
    pub fn create(&self) -> Option<gtk::Box> {
        let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
        b.set_hexpand(true);
        b.pack_start(&self.container, true, true, 0);
        add_rows(
            self.container.clone(),
            self.members.clone(),
            self.power_levels.clone(),
        );
        self.error
            .get_style_context()?
            .add_class("no_member_search");
        self.error.set_text(&i18n("No matching members found"));
        b.pack_start(&self.error, true, true, 0);
        self.connect();
        b.show_all();
        self.error.hide();
        Some(b)
    }

    /* removes the content of the row with index i */
    #[allow(dead_code)]
    pub fn update(&self, uid: String) -> Option<()> {
        let mut index = None;
        for (i, member) in self.members.iter().enumerate() {
            if member.uid == uid {
                index = Some(i);
                break;
            }
        }
        let widget = self.container.get_row_at_index(index? as i32)?;
        let child = widget.get_child()?;
        widget.remove(&child);
        /* We don't need to create a new widget because the draw signal
         * will handle the creation */

        None
    }

    pub fn connect(&self) {
        let container = self.container.clone();
        let members = self.members.clone();
        let error = self.error.clone();
        let id = self.search_entry.connect_search_changed(move |w| {
            filter_rows(
                container.clone(),
                members.clone(),
                error.clone(),
                w.get_text(),
            );
        });
        /* we need to remove the handler when the member list is destroyed */
        let id: Rc<RefCell<Option<signal::SignalHandlerId>>> = Rc::new(RefCell::new(Some(id)));
        let search_entry = self.search_entry.clone();
        self.container.connect_destroy(move |_| {
            let id = id.borrow_mut().take();
            if let Some(id) = id {
                signal::signal_handler_disconnect(&search_entry, id);
            }
        });
        /* we could slowly load members when the main thread is idle */
        /*
        let container = self.container.clone();
        let members = self.members.clone();
        for (index, member) in members.iter().enumerate() {
        gtk::idle_add(clone!(index, member, container => move || {
        if let Some(w) = container.get_row_at_index(index as i32) {
        if w.get_child().is_none() {
        w.add(&load_row_content(member.clone()));
        }
        }
        gtk::Continue(false)
        }));
        }
        */
    }
}

fn create_row(member: Member, power_level: Option<i32>) -> Option<gtk::ListBoxRow> {
    let row = gtk::ListBoxRow::new();
    row.connect_draw(clone!(member => move |w, _| {
        if w.get_child().is_none() {
            w.add(&load_row_content(member.clone(), power_level));
        }
        gtk::Inhibit(false)
    }));
    row.set_selectable(false);
    row.set_size_request(-1, 56);
    row.show();
    Some(row)
}

/* creating the row is quite slow, therefore we have a small delay when scrolling the members list */
fn load_row_content(member: Member, power_level: Option<i32>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);

    // Power level badge colour
    let pl = power_level.unwrap_or_default();
    let badge_color = match pl {
        100 => Some(BadgeColor::Gold),
        50...99 => Some(BadgeColor::Silver),
        1...49 => Some(BadgeColor::Gray),
        _ => None,
    };

    // Avatar
    let avatar = widgets::Avatar::avatar_new(Some(40));
    avatar.circle(
        member.uid.clone(),
        member.alias.clone(),
        40,
        badge_color,
        None,
    );

    let user_box = gtk::Box::new(gtk::Orientation::Vertical, 0); // Name & badge + Matrix ID
    let username_box = gtk::Box::new(gtk::Orientation::Horizontal, 0); // Name + badge

    let username = gtk::Label::new(Some(member.get_alias().as_str()));
    username.set_xalign(0.);
    username.set_margin_end(5);
    username.set_ellipsize(pango::EllipsizeMode::End);
    username_box.pack_start(&username, false, false, 0);

    // Power level badge colour
    let pl = power_level.unwrap_or_default();
    if pl > 0 && pl <= 100 {
        let badge_data = match pl {
            100 => (i18n("Admin"), "badge-gold"),
            50...99 => (i18n("Moderator"), "badge-silver"),
            1...49 => (i18n("Privileged"), "badge-grey"),
            _ => panic!(),
        };

        let badge_wid = gtk::Label::new(Some(format!("{} ({})", badge_data.0, pl).as_str()));
        if let Some(style) = badge_wid.get_style_context() {
            style.add_class("badge");
            style.add_class(badge_data.1);
        }
        username_box.pack_start(&badge_wid, false, false, 0);
    }

    // matrix ID + power level
    let uid = gtk::Label::new(Some(member.uid.as_str()));
    uid.set_xalign(0.);
    uid.set_line_wrap(true);
    uid.set_line_wrap_mode(pango::WrapMode::Char);
    if let Some(style) = uid.get_style_context() {
        style.add_class("small-font");
        style.add_class("dim-label");
    }

    b.set_margin_start(12);
    b.set_margin_end(12);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    user_box.pack_start(&username_box, true, true, 0);
    user_box.pack_start(&uid, true, true, 0);
    /* we don't have this state yet
     * let state = gtk::Label::new();
     * user_box.pack_end(&state, true, true, 0); */
    b.pack_start(&avatar, false, true, 0);
    b.pack_start(&user_box, true, true, 0);
    b.show_all();
    b
}

fn add_rows(
    container: gtk::ListBox,
    members: Vec<Member>,
    power_levels: HashMap<String, i32>,
) -> Option<usize> {
    /* Load just enough members to fill atleast the visible list */
    for member in members.iter() {
        let power_level = match power_levels.get(&member.uid) {
            Some(pl) => Some(*pl),
            None => None,
        };
        container.insert(&create_row(member.clone(), power_level)?, -1);
    }
    None
}

fn filter_rows(
    container: gtk::ListBox,
    members: Vec<Member>,
    label: gtk::Label,
    search: Option<String>,
) -> Option<usize> {
    /* Load just enough members to fill atleast the visible list */
    // Convert to Lowercase for case-insensitive searching
    let search = search?.to_lowercase();
    let search = search.as_str();
    let mut empty = true;
    for (index, member) in members.iter().enumerate() {
        let alias_lower = member.get_alias().to_lowercase();
        if !alias_lower.contains(search) {
            container.get_row_at_index(index as i32)?.hide();
        } else {
            container.get_row_at_index(index as i32)?.show();
            empty = false;
        }
    }
    label.set_visible(empty);
    None
}
