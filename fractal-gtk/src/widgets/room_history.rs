use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::VecDeque;
use chrono::DateTime;
use chrono::Local;
use chrono::Datelike;

use appop::AppOp;
use backend::BKCommand;
use i18n::i18n;
use types::Room;
use uibuilder::UI;
use uitypes::MessageContent;
use uitypes::RowType;
use App;

use gtk;
use gtk::prelude::*;
use glib;
use glib::source;
use globals;
use widgets;

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
                self.listbox.insert(message.widget.as_ref()?.get_listbox_row()?, 1);
            },
            Element::NewDivider(ref divider) => {
                self.listbox.insert(divider, 1);
            },
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
                self.listbox.insert(message.widget.as_ref()?.get_listbox_row()?, -1);
            },
            Element::NewDivider(ref divider) => {
                self.listbox.insert(divider, 1);
            },
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
    NewDivider(gtk::ListBoxRow),
    DayDivider(gtk::ListBoxRow),
}

pub struct RoomHistory {
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Rc<RefCell<List>>,
    ui: UI,
    backend: Sender<BKCommand>,
    room: Room,
    listbox: gtk::ListBox,
    loading_spinner: gtk::Spinner,
    source_id: Rc<RefCell<Option<source::SourceId>>>,
    queue: Rc<RefCell<VecDeque<MessageContent>>>,
    /* This is the id of the last viewed message */
}

impl RoomHistory {
    pub fn new(room: Room, op: &AppOp) -> RoomHistory {
        let history_container = op.ui.builder
            .get_object::<gtk::Box>("history_container")
            .expect("Can't find history_container in ui file.");
        let mut scroll = widgets::ScrollWidget::new();
        scroll.create();
        /* remove previous room history widget */
        for ch in history_container.get_children().iter() {
            history_container.remove(ch);
        }
        /* add room history widget */
        history_container.add(&scroll.get_container());
        let listbox = scroll.get_listbox();
        let loading_spinner = scroll.get_loading_spinner();

        RoomHistory {
            rows: Rc::new(RefCell::new(List::new(scroll, listbox.clone()))),
            ui: op.ui.clone(),
            listbox: listbox,
            loading_spinner,
            backend: op.backend.clone(),
            room: room,
            source_id: Rc::new(RefCell::new(None)),
            queue: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    pub fn create(&mut self, mut messages: Vec<MessageContent>) -> Option<()> {
        messages.reverse();
        self.queue.borrow_mut().append(&mut VecDeque::from(messages));
        self.run_queue();

        None
    }

    fn run_queue(&mut self) -> Option<()> {
        let backend = self.backend.clone();
        let ui = self.ui.clone();
        let queue = self.queue.clone();
        let rows = self.rows.clone();
        let room = self.room.clone();

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
                            day_divider = Some(Element::DayDivider(create_day_divider(item.date)));
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
                let b = create_row(item.clone(), &room, has_header, backend.clone(), ui.clone());
                item.widget = b;
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
    pub fn get_listbox(&self) -> &gtk::ListBox {
        &self.listbox
    }

    /* This is a temporary function to make the loadin spinner accesibile from outside the history,
     * it is currently needed for loading more messages
     * (which should also be moved to the room history) */
    pub fn get_loading_spinner(&self) -> &gtk::Spinner {
        &self.loading_spinner
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
                    _ => false
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

        let b = create_row(item.clone(),
        &self.room.clone(),
        has_header,
        self.backend.clone(),
            self.ui.clone());
        item.widget = b;
        rows.add_bottom(Element::Message(item));
        None
    }

    /* This adds messages to the top of the list */
    pub fn add_old_message(&mut self, item: MessageContent) -> Option<()> {
        self.queue.borrow_mut().push_back(item);
        self.run_queue();

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
    ) -> Option<widgets::MessageBox> {
    /* we need to create a message with the username, so that we don't have to pass
     * all information to the widget creating each row */
    let mut mb = widgets::MessageBox::new(backend, ui);
    mb.create(&row, has_header && row.mtype != RowType::Emote);

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
    /* We show the year only when the message wasn't send in the current year */
    let stamp = if date.year() == Local::now().year() {
        date.format(i18n("%e %B").as_str()).to_string()
    } else {
        date.format(i18n("%e %B %Y").as_str()).to_string()
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

fn create_new_message_divider() -> gtk::ListBoxRow {
    widgets::divider::new(i18n("New Messages").as_str())
}
