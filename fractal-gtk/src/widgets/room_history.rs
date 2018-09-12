extern crate glib;
extern crate gtk;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use appop::AppOp;
use backend::BKCommand;
use i18n::i18n;
use types::Room;
use uibuilder::UI;
use uitypes::MessageContent;
use uitypes::RowType;
use App;

use self::gtk::prelude::*;
use globals;
use widgets;

#[derive(Clone)]
pub struct RoomHistory {
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Rc<RefCell<Vec<MessageContent>>>,
    /* Op should be removed, but the MessageBox still needs it */
    ui: UI,
    backend: Sender<BKCommand>,
    room: Room,
    listbox: gtk::ListBox,
    divider: Option<gtk::ListBoxRow>,
}

impl RoomHistory {
    /*FIXME: return Option, to handler errors */
    pub fn new(listbox: gtk::ListBox, op: &AppOp) -> RoomHistory {
        /* remove all old messages from the listbox */
        for ch in listbox.get_children().iter().skip(1) {
            listbox.remove(ch);
        }

        let ui = op.ui.clone();
        RoomHistory {
            rows: Rc::new(RefCell::new(vec![])),
            ui: ui,
            listbox: listbox,
            backend: op.backend.clone(),
            /* FIXME: don't use unwarp, because it could fail:
             * We only create the room history when we have an active room */
            room: op
                .rooms
                .get(&op.active_room.clone().unwrap_or_default())
                .unwrap()
                .clone(),
            divider: None,
        }
    }

    pub fn create(&mut self, messages: Vec<MessageContent>) -> Option<()> {
        let data: Rc<RefCell<Vec<MessageContent>>> = Rc::new(RefCell::new(messages));
        let backend = self.backend.clone();
        let ui = self.ui.clone();
        let data = data.clone();
        let listbox = self.listbox.clone();
        let rows = self.rows.clone();
        let room = self.room.clone();

        /* TO-DO: we could set the listbox height the 52 * length of messages, to descrease jumps of the
         * scrollbar. 52 is the normal height of a message with one line
         * self.listbox.set_size_request(-1, 52 * messages.len() as i32); */

        /* Lacy load initial messages */
        gtk::idle_add(move || {
            let mut data = data.borrow_mut();
            if let Some(item) = data.pop() {
                let last = data.last();
                let has_header = {
                    if let Some(last) = last {
                        last.mtype == RowType::Emote || !should_group_message(&item, &last)
                    } else {
                        true
                    }
                };

                if let Some(row) = create_row(item.clone(), &room, has_header, backend.clone(), ui.clone())
                {
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

    /* This adds new incomming messages at then end of the list */
    pub fn add_new_message(&mut self, mut item: MessageContent) -> Option<()> {
        let mut rows = self.rows.borrow_mut();
        let has_header = {
            let last = rows.last();
            if let Some(last) = last {
                last.mtype == RowType::Emote || !should_group_message(&item, &last)
            } else {
                true
            }
        };

        if let Some(row) = create_row(
            item.clone(),
            &self.room.clone(),
            has_header,
            self.backend.clone(),
            self.ui.clone(),
        ) {
            self.listbox.insert(&row, -1);
            item.widget = Some(row);
            rows.push(item);
        }
        None
    }

    /* This adds messages to the top of the list */
    pub fn add_old_message(&mut self, mut item: MessageContent) -> Option<()> {
        let row = create_row(
            item.clone(),
            &self.room.clone(),
            true,
            self.backend.clone(),
            self.ui.clone(),
        )?;
        self.listbox.insert(&row, 1);
        item.widget = Some(row);
        let old = item.clone();
        self.rows.borrow_mut().insert(0, item);

        /* update the previous message:
         * we need to update the previous row because it could be that we have to remove the header */
        let mut rows = self.rows.borrow_mut();
        if let Some(previous) = rows.get_mut(1) {
            /* we need the header if the previous message was a emote */
            let has_header = !should_group_message(previous, &old) || old.mtype == RowType::Emote;
            if !has_header {
                let row = create_row(
                    previous.clone(),
                    &self.room.clone(),
                    has_header,
                    self.backend.clone(),
                    self.ui.clone(),
                )?;
                previous.widget = if let Some(widget) = previous.widget.take() {
                    let index = widget.get_index();
                    widget.destroy();
                    self.listbox.insert(&row, index);
                    //widget = row;
                    Some(row)
                } else {
                    None
                };
            }
        }

        None
    }

    pub fn add_divider(&mut self) -> Option<()> {
        let divider = widgets::divider::new(i18n("New Messages").as_str());
        self.listbox.insert(&divider, -1);
        self.divider = Some(divider);
        None
    }
}
/* This function creates the content for a Row based on the conntent of msg */
fn create_row(
    row: MessageContent,
    room: &Room,
    has_header: bool,
    backend: Sender<BKCommand>,
    ui: UI,
) -> Option<gtk::ListBoxRow> {
    let widget = {
        /* we need to create a message with the username, so that we don't have to pass
         * all information to the widget creating each row */
        let mut mb = widgets::MessageBox::new(row.clone(), backend, ui);
        let w = Some(mb.create(has_header && row.mtype != RowType::Emote));

        if let Some(ref image) = mb.image {
            let msg = row.msg.clone();
            let room = room.clone();
            image.connect_button_press_event(move |_, btn| {
                if btn.get_button() != 3 {
                    let msg = msg.clone();
                    let room = room.clone();
                    APPOP!(create_media_viewer, (msg, room));

                    Inhibit(true)
                } else {
                    Inhibit(false)
                }
            });
        }
        w
    };
    return widget;
}

/* returns if two messages should have only a single header or not */
fn should_group_message(msg: &MessageContent, prev: &MessageContent) -> bool {
    if msg.sender == prev.sender {
        let diff = msg.date.signed_duration_since(prev.date);
        let minutes = diff.num_minutes();
        minutes < globals::MINUTES_TO_SPLIT_MSGS
    } else {
        false
    }
}
