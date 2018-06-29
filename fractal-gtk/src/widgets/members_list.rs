extern crate gtk;

use std::cell::RefCell;
use std::rc::Rc;

use self::gtk::prelude::*;
use glib::signal;

use types::Member;
use widgets;
use widgets::avatar::AvatarExt;

#[derive(Debug, Clone)]
pub struct MembersList {
    container: gtk::ListBox,
    search_entry: gtk::SearchEntry,
    error: gtk::Label,
    members: Vec<Member>,
}

impl MembersList {
    pub fn new(m: Vec<Member>, entry: gtk::SearchEntry) -> MembersList {
        MembersList {
            container: gtk::ListBox::new(),
            error: gtk::Label::new(None),
            members: m,
            search_entry: entry,
        }
    }

    /* creates a empty list with members.len() rows, the content will be loaded when the row is
     * drawn */
    pub fn create(&self) -> Option<gtk::Box> {
        let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
        b.set_hexpand(true);
        b.pack_start(&self.container, true, true, 0);
        add_rows(self.container.clone(), self.members.clone());
        self.error
            .get_style_context()?
            .add_class("no_member_search");
        self.error.set_text("Nothing found");
        b.pack_start(&self.error, true, true, 0);
        self.connect();
        b.show_all();
        self.error.hide();
        Some(b)
    }

    /* removes the content of the row with index i */
    pub fn update(&self, i: usize) -> Option<()> {
        let widget = self.container.get_row_at_index(i as i32)?;
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

fn create_row(member: Member) -> Option<gtk::ListBoxRow> {
    let row = gtk::ListBoxRow::new();
    row.connect_draw(clone!(member => move |w, _| {
        if w.get_child().is_none() {
            w.add(&load_row_content(member.clone()));
        }
        gtk::Inhibit(false)
    }));
    row.set_selectable(false);
    row.set_size_request(-1, 56);
    row.show();
    Some(row)
}

/* creating the row is quite slow, therefore we have a small delay when scrolling the members list */
fn load_row_content(member: Member) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let avatar = widgets::Avatar::avatar_new(Some(40));
    avatar.circle(member.uid.clone(), member.alias.clone(), 40);
    let user_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let username = gtk::Label::new(Some(member.get_alias().as_str()));
    b.set_margin_start(12);
    b.set_margin_end(12);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    user_box.pack_start(&username, true, true, 0);
    /* we don't have this state yet
     * let state = gtk::Label::new();
     * user_box.pack_end(&state, true, true, 0); */
    b.pack_start(&avatar, false, true, 0);
    b.pack_start(&user_box, false, true, 0);
    b.show_all();
    b
}

fn add_rows(container: gtk::ListBox, members: Vec<Member>) -> Option<usize> {
    /* Load just enough members to fill atleast the visible list */
    for member in members.iter() {
        container.insert(&create_row(member.clone())?, -1);
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
    let search = search?;
    let search = search.as_str();
    let mut empty = true;
    for (index, member) in members.iter().enumerate() {
        if !member.get_alias().contains(search) {
            container.get_row_at_index(index as i32)?.hide();
        } else {
            container.get_row_at_index(index as i32)?.show();
            empty = false;
        }
    }
    label.set_visible(empty);
    None
}
