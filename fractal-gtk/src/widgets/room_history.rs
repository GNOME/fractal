extern crate gtk;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};

use i18n::i18n;
use i18n::ni18n_f;
use types::Message;
use backend::BKCommand;

use self::gtk::prelude::*;
use widgets;

#[derive(Debug, Clone)]
pub struct RoomHistory {
    messages: Vec<RowHistory>,
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Vec<Message>,
    backend: Sender<BKCommand>,
    listbox: gtk::ListBox
}

#[derive(Debug, Clone)]
pub struct RowHistory {
    pub message: Message,
    pub t: Option<RowType>,
}

#[derive(Debug, Clone)]
pub enum RowType {
    Divider,
    WithHeader,
    WithoutHeader
}

impl RoomHistory {
    pub fn new(messages: Vec<RowHistory>, listbox: gtk::ListBox, backend: Sender<BKCommand>) -> RoomHistory {
        RoomHistory {
            messages: messages,
            rows: vec![],
            listbox: listbox,
            backend: backend
        }
    }

    pub fn create(&mut self) -> Option<()> {
        let messages = self.messages.clone();
        let mut last = String::from("");
        for item in messages {
            let has_header = if last == item.message.sender {
                false
            } else {
                true
            };
            if let Some(row) = self.create_row(&item, has_header) {
                last = item.message.sender.clone();
                self.rows.push(item.message);
                self.listbox.insert(&row, -1);
            }
        }
        None
    }

    pub fn add_new_message(&mut self, item: RowHistory) -> Option<()> {
        let mut has_header = true;
        if let Some(last) = self.rows.last() {
            if last.sender == item.message.sender {
                has_header = false;
            }
        }
        if let Some(row) = self.create_row(&item, has_header) {
            self.rows.push(item.message);
            self.listbox.insert(&row, -1);
        }
        None
    }

    /* This function creates the content for a Row based on the conntent of msg
     * msg tells us what the type of the Message is: divider, message with header or message
     * without header */
    fn create_row(&self, row: &RowHistory, has_header: bool) -> Option<gtk::ListBoxRow> {
        if let Some(ref t) = row.t {
            let widget = match t {
                RowType::Divider => Some(widgets::divider::new(i18n("New Messages").as_str())),
                _ => { 
                    let mb = widgets::MessageBox::new(&row.message, &self.backend);
                    /*
                    match t {
                        RowType::WithHeader =>  Some(mb.widget()),
                        RowType::WithoutHeader => Some(mb.small_widget()),
                        _ => None
                    }
                    */
                    if has_header {
                        Some(mb.widget())
                    } else {
                        Some(mb.small_widget())
                    }
                }
            };
            return widget;
        }
        None
    }
}
