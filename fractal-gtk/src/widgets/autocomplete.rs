extern crate gtk;
extern crate pango;
extern crate gdk;
extern crate unicode_segmentation;

use self::unicode_segmentation::UnicodeSegmentation;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use glib;
use self::gtk::prelude::*;
use self::pango::LayoutExt;

use types::Member;
//use types::Room;
//use types::RoomList;

use widgets;
use app::AppOp;

pub struct Autocomplete {
    entry: gtk::Entry,
    listbox: gtk::ListBox,
    popover: gtk::Popover,
    window: gtk::Window,
    highlighted_entry: Vec<String>,
    popover_position: Option<i32>,
    popover_search: Option<String>,
    popover_closing: bool,
    op: Arc<Mutex<AppOp>>,
}

impl Autocomplete {
    pub fn new(op: Arc<Mutex<AppOp>>, window: gtk::Window, msg_entry: gtk::Entry, popover: gtk::Popover, listbox: gtk::ListBox) -> Autocomplete {
        Autocomplete {
            entry: msg_entry,
            listbox: listbox,
            popover: popover,
            window: window,
            highlighted_entry: vec![],
            popover_position: None,
            popover_search: None,
            popover_closing: false,
            op: op,
        }
    }

    pub fn connect(self) {
        let this: Rc<RefCell<Autocomplete>> = Rc::new(RefCell::new(self));

        let own = this.clone();
        this.borrow().window.connect_button_press_event(move |_, _| {
            if own.borrow().popover_position.is_some() {
                own.borrow_mut().autocomplete_enter();
                return Inhibit(true)
            }
            else {
                return Inhibit(false);
            }
        });

        let own = this.clone();
        this.borrow().entry.connect_property_cursor_position_notify(move |w| {
            if let Ok(item) = own.try_borrow() {
                let input = w.get_text().unwrap();
                let attr = item.add_highlight(input);
                w.set_attributes(&attr);
            }
        });

        let own = this.clone();
        this.borrow().entry.connect_property_selection_bound_notify(move |w| {
            if let Ok(item) = own.try_borrow() {
                let input = w.get_text().unwrap();
                let attr = item.add_highlight(input);
                w.set_attributes(&attr);
            }
        });

        let own = this.clone();
        this.borrow().entry.connect_changed(move |w| {
            if let Ok(item) = own.try_borrow() {
                let input = w.get_text().unwrap();
                let attr = item.add_highlight(input);
                w.set_attributes(&attr);
            }
        });

        let own = this.clone();
        this.borrow().entry.connect_delete_text(move |_, start, end| {
            if let Ok(mut item) = own.try_borrow_mut() {
                if let Some(pos) = item.popover_position {
                    if end <= pos + 1 || (start <= pos && end > pos){
                        item.autocomplete_enter();
                    }
                }
            }
        });

        let own = this.clone();
        this.borrow().entry.connect_key_release_event(move |_, k| {
            match k.get_keyval() {
                gdk::enums::key::Escape => {
                    if own.borrow().popover_position.is_some() {
                        own.borrow_mut().autocomplete_enter();
                        return Inhibit(true)
                    }
                }
                _ => {}
            }
            Inhibit(false)
        });

        let own = this.clone();
        this.borrow().entry.connect_key_press_event(move |w, ev| {
            match ev.get_keyval() {
                gdk::enums::key::BackSpace => {
                    if w.get_text().is_none()  || w.get_text().unwrap() == "" {
                        own.borrow_mut().autocomplete_enter();
                    }
                    return glib::signal::Inhibit(false);
                },
                /* Tab and Enter key */
                gdk::enums::key::Tab | gdk::enums::key::Return => {
                    if own.borrow().popover_position.is_some() {
                        let widget = {
                            own.borrow_mut().popover_closing = true;
                            own.borrow_mut().autocomplete_arrow(0)
                        };
                        if let Some(w) = widget {
                            let ev: &gdk::Event = ev;
                            let _ = w.emit("button-press-event", &[ev]);
                        }
                    }
                    else {
                        if ev.get_keyval() != gdk::enums::key::Tab {
                            return glib::signal::Inhibit(false);
                        }
                    }
                },
                /* Arrow key */
                gdk::enums::key::Up => {
                    let widget = {
                        own.borrow_mut().autocomplete_arrow(-1)
                    };
                    if let Some(w) = widget {
                        let ev: &gdk::Event = ev;
                        let _ = w.emit("button-press-event", &[ev]);
                    }
                },
                /* Arrow key */
                gdk::enums::key::Down => {
                    let widget = {
                        own.borrow_mut().autocomplete_arrow(1)
                    };

                    if let Some(w) = widget {
                        let ev: &gdk::Event = ev;
                        let _ = w.emit("button-press-event", &[ev]);
                    }
                }
                _ => return glib::signal::Inhibit(false),
            }
            return glib::signal::Inhibit(true);
        });

        let own = this.clone();
        this.borrow().entry.connect_key_release_event(move |e, ev| {
            let is_tab = ev.get_keyval() == gdk::enums::key::Tab;
            let text = e.get_text();
            /* when closing popover with tab */
            {
                if own.borrow().popover_closing {
                    own.borrow_mut().popover_closing = false;
                    return Inhibit(false);
                }
            }
            /* allow popover opening with tab 
             * don't update popover when the input didn't change */
            if !is_tab {
                if let Some(ref text) = text {
                    if let Some(ref old) = own.borrow().popover_search {
                        if text == old {
                            return Inhibit(false);
                        }
                    }
                }
            }
            /* update the popover when closed and tab is released
             * don't update the popover the arrow keys are pressed */
            if (is_tab && own.borrow().popover_position.is_none()) ||
                (ev.get_keyval() != gdk::enums::key::Up && ev.get_keyval() != gdk::enums::key::Down) {
                    own.borrow_mut().popover_search = text.clone();
                    let pos = e.get_position();
                    if let Some(text) = text.clone() {
                        let graphs = UnicodeSegmentation::graphemes(text.as_str(), true).collect::<Vec<&str>>();
                        let (p1, _) = graphs.split_at(pos as usize);
                        let first = p1.join("");
                        if own.borrow().popover_position.is_none() {
                            if !is_tab {
                                if let Some(at_pos) = first.rfind("@") {
                                    own.borrow_mut().popover_position = Some(at_pos as i32);
                                }
                            }
                            else {
                                if let Some(space_pos) = first.rfind(" ") {
                                    own.borrow_mut().popover_position = Some(space_pos as i32 + 1);
                                }
                                else {
                                    own.borrow_mut().popover_position = Some(0);
                                }
                            }
                        }
                    }

                    if own.borrow().popover_position.is_some() {
                        let list = {
                            own.borrow().autocomplete(text, e.get_position())
                        };
                        let widget_list = {
                            own.borrow_mut().autocomplete_show_popover(list)
                        };
                        for (alias, widget) in widget_list.iter() {
                            widget.connect_button_press_event(clone!(own, alias => move |_, ev| {
                                own.borrow_mut().autocomplete_insert(alias.clone());
                                if ev.is::<gdk::EventKey>() {
                                    let ev = {
                                        let ev: &gdk::Event = ev;
                                        ev.clone().downcast::<gdk::EventKey>().unwrap()
                                    };
                                    /* Submit on enter */
                                    if ev.get_keyval() == gdk::enums::key::Return || ev.get_keyval() == gdk::enums::key::Tab  {
                                        own.borrow_mut().autocomplete_enter();
                                    }
                                }
                                else if ev.is::<gdk::EventButton>() {
                                    own.borrow_mut().autocomplete_enter();
                                }
                                Inhibit(true)
                            }));
                        }
                        /*for element in op.lock().unwrap().highlighted_entry.iter() {
                          println!("Saved aliases {}", element);
                          }
                          */
                    }
                }
            Inhibit(false)
        });
    }

    pub fn autocomplete_insert(&mut self, alias: String) {
        if let Some(start_pos) = self.popover_position {
            let mut start_pos = start_pos as i32;
            let end_pos = self.entry.get_position();
            self.entry.delete_text(start_pos, end_pos);
            self.entry.insert_text(&alias, &mut start_pos);
            self.entry.set_position(start_pos);

            /* highlight member inside the entry */
            /* we need to set the highlight here the first time
             * because the ui changes from others are blocked as long we hold the look */
            if let Some(input) = self.entry.get_text() {
                self.highlighted_entry.push(alias);
                let attr = self.add_highlight(input);
                self.entry.set_attributes(&attr);
            }
        }
    }

    pub fn autocomplete_enter(&mut self) -> bool {
        if let Some(input) = self.entry.get_text() {
            let attr = self.add_highlight(input);
            self.entry.set_attributes(&attr);
        }
        self.popover_position = None;
        self.popover_search = None;
        let visible = self.popover.is_visible();
        self.popover.popdown();
        return visible;
    }

    pub fn add_highlight(&self, input: String) -> pango::AttrList {
        fn contains((start, end): (i32, i32), item: i32) -> bool {
            if start <= end {
                return start <= item && end > item;
            } else {
                return start <= item || end > item;
            }
        }
        let input = input.to_lowercase();
        let bounds = self.entry.get_selection_bounds();
        let context = gtk::Widget::get_style_context (&self.entry.clone().upcast::<gtk::Widget>()).unwrap();
        let fg  = gtk::StyleContext::lookup_color (&context, "theme_selected_bg_color").unwrap();
        let red = fg.red * 65535. + 0.5;
        let green = fg.green * 65535. + 0.5;
        let blue = fg.blue * 65535. + 0.5;
        let color = pango::Attribute::new_foreground(red as u16, green as u16, blue as u16).unwrap();

        let attr = pango::AttrList::new();
        for (_, alias) in self.highlighted_entry.iter().enumerate() {
            let mut input = input.clone();
            let alias = &alias.to_lowercase();
            let mut removed_char = 0;
            let mut found = false;
            while input.contains(alias) {
                let pos = {
                    let start = input.find(alias).unwrap() as i32;
                    (start, start + alias.len() as i32)
                };
                let mut color = color.clone();
                let mark_start = removed_char as i32 + pos.0;
                let mark_end = removed_char as i32 + pos.1;
                let mut final_pos = Some((mark_start, mark_end));
                /* exclude selected text */
                if let Some((bounds_start, bounds_end)) = bounds {
                    /* If the selection is within the alias */
                    if contains((mark_start, mark_end), bounds_start) &&
                        contains((mark_start, mark_end), bounds_end) {
                            final_pos = Some((mark_start, bounds_start));
                            /* Add blue color after a selection */
                            let mut color = color.clone();
                            color.set_start_index(bounds_end as u32);
                            color.set_end_index(mark_end as u32);
                            attr.insert(color);
                        } else {
                            /* The alias starts inside a selection */
                            if contains(bounds.unwrap(), mark_start) {
                                final_pos = Some((bounds_end, final_pos.unwrap().1));
                            }
                            /* The alias ends inside a selection */
                            if contains(bounds.unwrap(), mark_end - 1) {
                                final_pos = Some((final_pos.unwrap().0, bounds_start));
                            }
                        }
                }

                if let Some((start, end)) = final_pos {
                    color.set_start_index(start as u32);
                    color.set_end_index(end as u32);
                    attr.insert(color);
                }
                {
                    let end = pos.1 as usize;
                    input.drain(0..end);
                }
                removed_char = removed_char + pos.1 as u32;
                found = true;
            }
            if !found {
                //guard.highlighted_entry.remove(i);
                //println!("Should remove {} form store", alias);
            }
        }

        return attr;
    }

    pub fn autocomplete_arrow(&mut self, direction: i32) -> Option<gtk::Widget> {
        let mut result = None;
        if let Some(row) = self.listbox.get_selected_row() {
            let index = row.get_index() + direction;
            if index >= 0 {
                let row = self.listbox.get_row_at_index(row.get_index() + direction);
                match row {
                    None => {
                        if let Some(row) = self.listbox.get_row_at_index(0) {
                            self.listbox.select_row(&row);
                            result = Some(row.get_children().first().unwrap().clone());
                        }
                    }
                    Some(row) => {
                        self.listbox.select_row(&row);
                        result = Some(row.get_children().first().unwrap().clone());
                    }
                };
            }
            else {
                if let Some(row) = self.listbox.get_children().last() {
                    if let Ok(row) = row.clone().downcast::<gtk::ListBoxRow>() {
                        self.listbox.select_row(&row);
                        result = Some(row.get_children().first().unwrap().clone());
                    }
                }
            }
        }
        else {
            if let Some(row) = self.listbox.get_row_at_index(0) {
                self.listbox.select_row(&row);
                result = Some(row.get_children().first().unwrap().clone());
            }
        }
        return result;
    }

    pub fn autocomplete_show_popover(&mut self, list: Vec<Member>) -> HashMap<String, gtk::EventBox> {
        for ch in self.listbox.get_children().iter() {
            self.listbox.remove(ch);
        }

        let mut widget_list : HashMap<String, gtk::EventBox> = HashMap::new();

        if list.len() > 0 {
            let guard = self.op.lock().unwrap();
            for m in list.iter() {
                let alias = &m.alias.clone().unwrap_or_default().trim_right_matches(" (IRC)").to_owned();
                let widget;
                {
                    let mb = widgets::MemberBox::new(&m, &guard);
                    widget = mb.widget(true);
                }

                let w = widget.clone();
                let a = alias.clone();
                widget_list.insert(a, w);
                self.listbox.add(&widget);
            }

            self.popover.set_relative_to(Some(&self.entry));
            self.popover.set_modal(false);
            /* calculate position for popover */

            if let Some(text_index) = self.popover_position {
                let offset = self.entry.get_layout_offsets().0;
                let layout = self.entry.get_layout().unwrap();
                let layout_index = self.entry.text_index_to_layout_index(text_index);
                let (_, index) = layout.get_cursor_pos(layout_index);

                pango::extents_to_pixels(Some(&index), None);
                self.popover.set_pointing_to(&gdk::Rectangle{x: index.x + offset + 10, y: 0, width: 0, height: 0});
            }

            if let Some(row) = self.listbox.get_row_at_index(0) {
                self.listbox.select_row(&row);
            }

            self.popover.popup();
        }
        else {
            self.autocomplete_enter();
        }
        return widget_list;
    }

    pub fn autocomplete(&self, text: Option<String>, pos : i32) -> Vec<Member> {
        let mut list: Vec<Member> = vec![];
        let guard = self.op.lock().unwrap();
        let rooms = &guard.rooms;
        match text {
            None => {},
            Some(txt) => {
                if let Some(at_pos) = self.popover_position {
                    let last = {
                        let start = at_pos as usize;
                        let end = pos as usize;
                        txt.get(start..end)
                    };
                    if let Some(last) = last {
                        println!("Matching string '{}'", last);
                        /*remove @ from string*/
                        let w = if last.starts_with("@") {
                            last[1..].to_lowercase()
                        }
                        else {
                            last.to_lowercase()
                        };

                        /* Search for the 5 most recent active users */
                        if let Some(aroom) = guard.active_room.clone() {
                            if let Some(r) = rooms.get(&aroom) {
                                let mut count = 0;
                                for (_, m) in r.members.iter() {
                                    let alias = &m.alias.clone().unwrap_or_default().to_lowercase();
                                    let uid = &m.uid.clone().to_lowercase()[1..];
                                    if alias.starts_with(&w) || uid.starts_with(&w) {
                                        list.push(m.clone());
                                        count = count + 1;
                                        /* Search only for 5 matching users */
                                        if count > 4 {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
        return list;
    }
}
