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
    messages: Vec<MessageContent>,
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Rc<RefCell<Vec<MessageContent>>>,
    backend: Sender<BKCommand>,
    listbox: gtk::ListBox
}

/* MessageContent contains all data needed to display one row
 * therefore it should contain only one Message body with one format
 * To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Clone)]
pub struct MessageContent {
    pub sender: String,
    pub sender_name: Option<String>,
    pub mtype: RowType,
    pub body: String,
    pub date: DateTime<Local>,
    pub thumb: Option<String>,
    pub url: Option<String>,
    pub formatted_body: Option<String>,
    pub format: Option<String>,
}

/* To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Clone)]
pub enum RowType {
    Divider,
    WithHeader,
    Mention,
    Emote,
    Message,
}

impl RoomHistory {
    pub fn new(messages: Vec<MessageContent>, listbox: gtk::ListBox, backend: Sender<BKCommand>) -> RoomHistory {
        /* remove all old messages from the listbox */
        for ch in listbox.get_children().iter().skip(1) {
            listbox.remove(ch);
        }

        RoomHistory {
            messages: messages,
            rows: Rc::new(RefCell::new(vec![])),
            listbox: listbox,
            backend: backend
        }
    }

    pub fn create(&mut self) -> Option<()> {
        let messages = self.messages.clone();
        let mut last = String::from("");
        /* lazy load this */
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
            if let Some(item) = data.borrow_mut().pop() {
                let mut has_header = true;
                if let Some(last) = rows.borrow().last() {
                    if last.sender == item.sender {
                        has_header = false;
                    }
                }
                if let Some(row) = create_row(&item, has_header, backend.clone()) {
                    last = item.sender.clone();
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
        let mut has_header = true;
        if let Some(last) = self.rows.borrow().last() {
            if last.sender == item.sender {
                has_header = false;
            }
        }

        if let Some(row) = create_row(&item, has_header, self.backend.clone()) {
            self.rows.borrow_mut().push(item);
            self.listbox.insert(&row, -1);
        }
        None
    }

    /* this adds messages to to the top of the list */
    pub fn add_old_message(&mut self, item: MessageContent) -> Option<()> {
        let mut has_header = true;
        if let Some(last) = self.rows.borrow().last() {
            if last.sender == item.sender {
                has_header = false;
            }
        }
        if let Some(row) = create_row(&item, has_header, self.backend.clone()) {
            //self.rows.insert(0, item);
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
