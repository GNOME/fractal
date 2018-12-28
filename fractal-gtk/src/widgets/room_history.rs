use chrono::DateTime;
use chrono::Datelike;
use chrono::Local;
use chrono::Timelike;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use crate::appop::AppOp;
use crate::backend::BKCommand;
use crate::i18n::i18n;
use crate::uitypes::MessageContent;
use crate::uitypes::RowType;

use crate::globals;
use crate::widgets;
use gio::ActionMapExt;
use gio::SimpleActionGroup;
use glib::source;
use gtk;
use gtk::prelude::*;

struct List {
    list: VecDeque<Element>,
    listbox: gtk::ListBox,
    view: widgets::ScrollWidget,
}

impl List {
    pub fn new(view: widgets::ScrollWidget, listbox: gtk::ListBox) -> List {
        List {
            list: VecDeque::new(),
            listbox,
            view,
        }
    }

    pub fn add_top(&mut self, element: Element) -> Option<()> {
        self.view.set_balance_top();
        /* insert position is 1 because at position 0 is the spinner */
        match element {
            Element::Message(ref message) => {
                self.listbox
                    .insert(message.widget.as_ref()?.get_listbox_row()?, 1);
            }
            Element::NewDivider(ref divider) => {
                self.listbox.insert(divider.get_widget(), 1);
            }
            Element::DayDivider(ref divider) => {
                self.listbox.insert(divider, 1);
            }
        }
        self.list.push_back(element);
        /* TODO: update the previous message:
         * we need to update the previous row because it could be that we have to remove the header */
        None
    }

    pub fn add_bottom(&mut self, element: Element) -> Option<()> {
        match element {
            Element::Message(ref message) => {
                self.listbox
                    .insert(message.widget.as_ref()?.get_listbox_row()?, -1);
            }
            Element::NewDivider(ref divider) => {
                self.listbox.insert(divider.get_widget(), -1);
            }
            Element::DayDivider(ref divider) => {
                self.listbox.insert(divider, -1);
            }
        }
        self.list.push_front(element);
        None
    }
}

/* These Enum contains all differnet types of rows the room history can have, e.g room message, new
 * message divider, day divider */
#[derive(Clone)]
enum Element {
    Message(MessageContent),
    NewDivider(widgets::NewMessageDivider),
    DayDivider(gtk::ListBoxRow),
}

pub struct RoomHistory {
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Rc<RefCell<List>>,
    backend: Sender<BKCommand>,
    source_id: Rc<RefCell<Option<source::SourceId>>>,
    queue: Rc<RefCell<VecDeque<MessageContent>>>,
}

impl RoomHistory {
    pub fn new(actions: SimpleActionGroup, room_id: String, op: &AppOp) -> RoomHistory {
        let history_container = op
            .ui
            .builder
            .get_object::<gtk::Box>("history_container")
            .expect("Can't find history_container in ui file.");
        let action = actions.lookup_action("request_older_messages");
        let scroll = widgets::ScrollWidget::new(action, room_id);
        /* remove previous room history widget */
        for ch in history_container.get_children().iter() {
            history_container.remove(ch);
        }
        /* add room history widget */
        history_container.add(&scroll.get_container());
        let listbox = scroll.get_listbox();

        /* Add the action groupe to the room_history */
        listbox.insert_action_group("room_history", Some(&actions));

        RoomHistory {
            rows: Rc::new(RefCell::new(List::new(scroll, listbox))),
            backend: op.backend.clone(),
            source_id: Rc::new(RefCell::new(None)),
            queue: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    pub fn create(&mut self, mut messages: Vec<MessageContent>) -> Option<()> {
        let mut position = messages.len();
        /* Find position of last viewed message */
        for (i, item) in messages.iter().enumerate() {
            if item.last_viewed {
                position = i + 1;
            }
        }
        let bottom = messages.split_off(position);
        messages.reverse();
        self.add_old_messages_in_batch(messages);
        /* Add the rest of the messages after the new message divider */
        self.add_new_messages_in_batch(bottom);

        None
    }

    fn run_queue(&mut self) -> Option<()> {
        let backend = self.backend.clone();
        let queue = self.queue.clone();
        let rows = self.rows.clone();

        /* TO-DO: we could set the listbox height the 52 * length of messages, to descrease jumps of the
         * scrollbar. 52 is the normal height of a message with one line
         * self.listbox.set_size_request(-1, 52 * messages.len() as i32); */

        if self.source_id.borrow().is_some() {
            /* We don't need a new loop, just keeping the old one */
        } else {
            /* Lacy load initial messages */
            let source_id = self.source_id.clone();
            *self.source_id.borrow_mut() = Some(gtk::idle_add(move || {
                let mut data = queue.borrow_mut();
                if let Some(mut item) = data.pop_front() {
                    let last = data.front();
                    let mut day_divider = None;
                    let has_header = {
                        if let Some(last) = last {
                            if item.date.day() != last.date.day() {
                                day_divider =
                                    Some(Element::DayDivider(create_day_divider(item.date)));
                            }
                            last.mtype == RowType::Emote || !should_group_message(&item, &last)
                        } else {
                            true
                        }
                    };

                    if item.last_viewed && !rows.borrow().list.is_empty() {
                        let divider = Element::NewDivider(create_new_message_divider());
                        rows.borrow_mut().add_top(divider);
                    }
                    item.widget = create_row(item.clone(), has_header, backend.clone());
                    rows.borrow_mut().add_top(Element::Message(item));
                    if let Some(day_divider) = day_divider {
                        rows.borrow_mut().add_top(day_divider);
                    }
                } else {
                    /* Remove the source id, since the closure is destoryed */
                    source_id.borrow_mut().take();
                    return gtk::Continue(false);
                }
                return gtk::Continue(true);
            }));
        }
        None
    }

    pub fn destroy(self) {
        if let Some(id) = self.source_id.borrow_mut().take() {
            source::source_remove(id);
        }
    }

    /* This is a temporary function to make the listbox accesibile from outside the history, it is
     * currently needed for temp messages (which should also be moved to the room history) */
    pub fn get_listbox(&self) -> gtk::ListBox {
        let listbox = self.rows.borrow().listbox.clone();
        listbox
    }

    /* This adds new incomming messages at then end of the list */
    pub fn add_new_message(&mut self, mut item: MessageContent) -> Option<()> {
        let mut rows = self.rows.borrow_mut();
        let mut day_divider = None;
        let has_header = {
            let last = rows.list.front();
            if let Some(last) = last {
                match last {
                    Element::Message(ref message) => {
                        if item.date.day() != message.date.day() {
                            day_divider = Some(Element::DayDivider(create_day_divider(item.date)));
                        }
                        message.mtype == RowType::Emote || !should_group_message(&item, &message)
                    }
                    _ => false,
                }
            } else {
                true
            }
        };

        if item.last_viewed {
            let divider = Element::NewDivider(create_new_message_divider());
            rows.add_bottom(divider);
        }
        if let Some(day_divider) = day_divider {
            rows.add_bottom(day_divider);
        }

        let b = create_row(item.clone(), has_header, self.backend.clone());
        item.widget = b;
        rows.add_bottom(Element::Message(item));
        None
    }

    pub fn add_new_messages_in_batch(&mut self, messages: Vec<MessageContent>) -> Option<()> {
        /* TODO: use lazy loading */
        for item in messages {
            self.add_new_message(item);
        }

        None
    }

    pub fn add_old_messages_in_batch(&mut self, messages: Vec<MessageContent>) -> Option<()> {
        self.rows.borrow().view.reset_request_sent();
        /* TODO: Try if extend would be faster then append */
        self.queue
            .borrow_mut()
            .append(&mut VecDeque::from(messages));
        self.run_queue();

        None
    }
}

/* This function creates the content for a Row based on the conntent of msg */
fn create_row(
    row: MessageContent,
    has_header: bool,
    backend: Sender<BKCommand>,
) -> Option<widgets::MessageBox> {
    /* we need to create a message with the username, so that we don't have to pass
     * all information to the widget creating each row */
    let mut mb = widgets::MessageBox::new(backend);
    mb.create(&row, has_header && row.mtype != RowType::Emote);

    Some(mb)
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

/* Create the day divider */
fn create_day_divider(date: DateTime<Local>) -> gtk::ListBoxRow {
    let gdate = glib::DateTime::new_local(
        date.year(),
        date.month() as i32,
        date.day() as i32,
        date.hour() as i32,
        date.minute() as i32,
        date.second() as f64,
    );
    /* We show the year only when the message wasn't send in the current year */
    let format = if date.year() == Local::now().year() {
        // Translators: This is a date format in the day divider without the year
        i18n("%B %e")
    } else {
        // Translators: This is a date format in the day divider with the year
        i18n("%B %e, %Y")
    };
    let stamp = if let Some(string) = gdate.format(&format) {
        string
    } else {
        // Fallback to a non glib time string
        date.format(&format).to_string()
    };
    let row = gtk::ListBoxRow::new();
    if let Some(style) = row.get_style_context() {
        style.add_class("divider");
    }
    row.set_margin_top(24);
    row.set_selectable(false);
    row.set_activatable(false);
    let label = gtk::Label::new(stamp.as_str());
    label.set_selectable(false);
    row.add(&label);

    row.show_all();
    row
}

fn create_new_message_divider() -> widgets::NewMessageDivider {
    widgets::NewMessageDivider::new(i18n("New Messages").as_str())
}
