extern crate gtk;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};

use chrono::prelude::DateTime;
use chrono::prelude::Local;

use i18n::i18n;
use i18n::ni18n_f;
use backend::BKCommand;

use self::gtk::prelude::*;
use widgets;

#[derive(Debug, Clone)]
pub struct RoomHistory {
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Rc<RefCell<Vec<MessageContent>>>,
    backend: Sender<BKCommand>,
    listbox: gtk::ListBox
}

/* MessageContent contains all data needed to display one row
 * therefore it should contain only one Message body with one format
 * To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Clone, PartialEq)]
pub struct MessageContent {
    pub id: String,
    pub sender: String,
    pub sender_name: Option<String>,
    pub mtype: RowType,
    pub body: String,
    pub date: DateTime<Local>,
    pub thumb: Option<String>,
    pub url: Option<String>,
    pub formatted_body: Option<String>,
    pub format: Option<String>,
    pub highlights: Vec<String>,
}

/* To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Clone, PartialEq)]
pub enum RowType {
    Divider,
    WithHeader,
    Mention,
    Emote,
    Message,
    Sticker,
    Image,
    Audio,
    Video,
    File,
}

impl RoomHistory {
    pub fn new(listbox: gtk::ListBox, backend: Sender<BKCommand>) -> RoomHistory {
        /* remove all old messages from the listbox */
        for ch in listbox.get_children().iter().skip(1) {
            listbox.remove(ch);
        }

        println!("Create new room history");

        RoomHistory {
            rows: Rc::new(RefCell::new(vec![])),
            listbox: listbox,
            backend: backend
        }
    }

    pub fn create(&mut self, messages: Vec<MessageContent>) -> Option<()> {
        let mut last = String::from("");
        //self.listbox.set_size_request(-1, 52 * messages.len() as i32);
        let data: Rc<RefCell<Vec<MessageContent>>> = Rc::new(RefCell::new(messages));
        let backend = self.backend.clone();
        let data = data.clone();
        let listbox = self.listbox.clone();
        let rows = self.rows.clone();
        /* TO-DO: we could set the listbox height the 52 * length of messages, to descrease jumps of the
         * scrollbar. 52 is the normal height of a message with one line */
        /* Lacy load initial messages */
        gtk::idle_add(move || {
            let mut data = data.borrow_mut();
            if let Some(item) = data.pop() {
                let last = data.last();
                let has_header = item.mtype != RowType::Emote && !(last.is_some() && last.unwrap().sender == item.sender);
                if let Some(row) = create_row(&item, has_header, backend.clone()) {
                    rows.borrow_mut().push(item);
                    listbox.insert(&row, 1);
                }
            } else {
                return gtk::Continue(false);
            }
            return gtk::Continue(true);
        });
        None
    }

    /* updates the current message list, it adds new message and update exciting once */
    pub fn update(&mut self, mut messages: Vec<MessageContent>) -> Option<()> {
        /* Steps update the listbox 
         * 1. Find last old message in the new messages list (length N) if not present skip steps
         *    to 4.
         * 2. Add new messages, after last old message to the end of the listbox
         * 3. Check if the last N messages in the resulting list are the same as in the new
         *    messages list
         * 4. Drop everything if the check in step 3. fails (if it fails we will still have a flash)
         * 5. Create a complete new list
         */

        let mut new_msgs : Vec<MessageContent> = vec![];
        let mut rows = self.rows.borrow_mut();
        let mut found = false;
        messages.reverse();
        {
            let last = rows.last();
            if let Some(last) = last {
                for (i, m) in messages.iter().enumerate() {
                    if found {
                        println!("Append new message");
                        new_msgs.push(m.clone());

                    } else {
                        if last.id == m.id {
                            println!("Found last message {}", last.body);
                            found = true;
                        }
                    }
                }
            }
            else {
                println!("No last message");
            }
        }
        if !found {
            println!("not found clean everything");
            /* remove old list and start over */
            messages.reverse();
            new_msgs = messages;
            /* remove all old messages from the listbox */
            for ch in self.listbox.get_children().iter().skip(1) {
                self.listbox.remove(ch);
            }
            /* clean the vector */
            rows.drain(..);
        }
        let mut last = String::from("");
        //self.listbox.set_size_request(-1, 52 * messages.len() as i32);
        let data: Rc<RefCell<Vec<MessageContent>>> = Rc::new(RefCell::new(new_msgs));
        let backend = self.backend.clone();
        let data = data.clone();
        let listbox = self.listbox.clone();
        let rows = self.rows.clone();
        /* TO-DO: we could set the listbox height the 52 * length of messages, to descrease jumps of the
         * scrollbar. 52 is the normal height of a message with one line */
        /* Lacy load initial messages */
        gtk::idle_add(move || {
            let mut data = data.borrow_mut();
            if let Some(item) = data.pop() {
                let has_header = {
                    let rows = rows.borrow();
                    let last = rows.last();
                    item.mtype != RowType::Emote && !(last.is_some() && last.unwrap().sender == item.sender)
                };
                if let Some(row) = create_row(&item, has_header, backend.clone()) {
                    rows.borrow_mut().push(item);
                    listbox.insert(&row, 1);
                }
            } else {
                return gtk::Continue(false);
            }
            return gtk::Continue(true);
        });

        None
    }

    /* this adds new incomming messages at then end of the list */
    pub fn add_new_message(&mut self, item: MessageContent) -> Option<()> {
        let mut rows = self.rows.borrow_mut();
        let has_header = {
            let last = rows.last();
            item.mtype != RowType::Emote && !(last.is_some() && last.unwrap().sender == item.sender)
        };

        println!("{} | {} | new message bottom", item.id, item.date.to_string());
        if let Some(row) = create_row(&item, has_header, self.backend.clone()) {
            rows.push(item);
            self.listbox.insert(&row, -1);
        }
        None
    }

    /* this adds messages to the top of the list */
    pub fn add_old_message(&mut self, item: MessageContent) -> Option<()> {
        /* We need to update the message before the new message because it could be possibile that
         * we need to remove the header */
        /*
           let rows = self.rows.borrow();
           let last = rows.last();
           let has_header = !(last.is_some() && last.unwrap().sender == item.sender);
           */
        let has_header = true;

        println!("{} | {} old message top", item.id, item.date.to_string());
        if let Some(row) = create_row(&item, has_header, self.backend.clone()) {
            self.rows.borrow_mut().insert(0, item);
            self.listbox.insert(&row, 1);
        }
        None
    }
}
/* This function creates the content for a Row based on the conntent of msg
 * msg tells us what the type of the Message is: divider, message with header or message
 * without header */
fn create_row(row: &MessageContent, has_header: bool, backend: Sender<BKCommand>) -> Option<gtk::ListBoxRow> {
    let widget = match row.mtype {
        RowType::Divider => Some(widgets::divider::new(i18n("New Messages").as_str())),
        _ => {
            /* we need to create a message with the username, so that we don't have to pass
             * all information to the widget creating each row */
            let mb = widgets::MessageBox::new(&row, &backend);
            if has_header {
                Some(mb.widget())
            } else {
                Some(mb.small_widget())
            }
        }
    };
    return widget;
}
