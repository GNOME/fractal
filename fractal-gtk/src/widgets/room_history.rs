use chrono::DateTime;
use chrono::Datelike;
use chrono::Local;
use chrono::Timelike;
use fragile::Fragile;
use log::warn;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::appop::AppOp;
use crate::backend::BKResponse;
use crate::i18n::i18n;
use crate::uitypes::MessageContent;
use crate::uitypes::RowType;

use crate::globals;
use crate::widgets;
use crate::widgets::{PlayerExt, VideoPlayerWidget};
use fractal_api::backend::ThreadPool;
use fractal_api::cache::CacheMap;
use fractal_api::identifiers::{RoomId, UserId};
use fractal_api::url::Url;
use gio::ActionMapExt;
use gio::SimpleActionGroup;
use glib::source;
use glib::source::Continue;
use glib::SignalHandlerId;
use glib::Source;
use gtk;
use gtk::prelude::*;

struct List {
    /* With the exception of temporary widgets, only modify the fields list and listbox
    through the methods add_top(), add_bottom(), and remove_item() to maintain the
    1-1 correspondence between them. */
    list: VecDeque<Element>,
    new_divider_index: Option<usize>,
    playing_videos: Vec<(Rc<VideoPlayerWidget>, SignalHandlerId)>,
    listbox: gtk::ListBox,
    video_scroll_debounce: Option<source::SourceId>,
    view: widgets::ScrollWidget,
}

impl List {
    pub fn new(view: widgets::ScrollWidget, listbox: gtk::ListBox) -> List {
        List {
            list: VecDeque::new(),
            new_divider_index: None,
            playing_videos: Vec::new(),
            listbox,
            video_scroll_debounce: None,
            view,
        }
    }

    pub fn add_top(&mut self, element: Element) {
        self.view.set_balance_top();
        /* insert position is 1 because at position 0 is the spinner */
        self.listbox.insert(element.get_listbox_row(), 1);
        self.list.push_back(element);
        self.view.set_kinetic_scrolling(true);
        /* TODO: update the previous message:
         * we need to update the previous row because it could be that we have to remove the header */
    }

    pub fn add_bottom(&mut self, element: Element) {
        self.listbox.insert(element.get_listbox_row(), -1);
        if let Some(index) = self.new_divider_index {
            self.new_divider_index = Some(index + 1);
        }
        self.list.push_front(element);
    }

    fn remove_item(&mut self, index: usize, row: &gtk::ListBoxRow) {
        self.list.remove(index);
        self.listbox.remove(row);
    }

    fn create_new_message_divider(rows: Rc<RefCell<Self>>) -> widgets::NewMessageDivider {
        let rows_weak = Rc::downgrade(&rows);
        let remove_divider = move || {
            rows_weak.upgrade().map(|rows| {
                let new_divider_index = rows
                    .borrow_mut()
                    .new_divider_index
                    .take()
                    .expect("The new divider index must exist, since there is a new divider");
                rows.borrow_mut().list.remove(new_divider_index);
            });
        };
        widgets::NewMessageDivider::new(i18n("New Messages").as_str(), remove_divider)
    }
    fn update_videos(&mut self) {
        let visible = self.find_visible_videos();
        let mut new_looped: Vec<(Rc<VideoPlayerWidget>, SignalHandlerId)> =
            Vec::with_capacity(visible.len());

        /* Once drain_filter is not nightly-only anymore, we can use drain_filter. */
        for (player, handler_id) in self.playing_videos.drain(..) {
            if visible.contains(&player) {
                new_looped.push((player, handler_id));
            } else {
                player.stop_loop(handler_id);
            }
        }
        for player in visible {
            if !new_looped.iter().any(|(widget, _)| widget == &player) {
                let handler_id = player.play_in_loop();
                new_looped.push((player, handler_id));
            }
        }
        self.playing_videos = new_looped;
    }

    fn find_visible_videos(&self) -> Vec<Rc<VideoPlayerWidget>> {
        self.find_all_visible_indices()
            .iter()
            .filter_map(|&index| match self.list.get(index)? {
                Element::Message(content) => match content.mtype {
                    RowType::Video => {
                        Some(content
                            .widget
                            .as_ref()?
                            .get_video_widget()
                            .expect("The widget of every MessageContent, whose mtype is RowType::Video, must have a video_player."))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    fn find_all_visible_indices(&self) -> Vec<usize> {
        let len = self.list.len();
        let mut indices = Vec::new();
        if len == 0 {
            return indices;
        }

        let sw = self.view.get_scrolled_window();
        let visible_index = match get_rel_position(&sw, &self.list[0]) {
            RelativePosition::InSight => Some(0),
            _ => self.find_visible_index((0, len - 1)),
        };
        if let Some(visible) = visible_index {
            indices.push(visible);
            let upper = self.list.iter().enumerate().skip(visible + 1);
            self.add_while_visible(&mut indices, upper);
            let lower = self
                .list
                .iter()
                .enumerate()
                .rev()
                .skip(self.list.len() - visible);
            self.add_while_visible(&mut indices, lower);
        }
        indices
    }

    fn find_visible_index(&self, range: (usize, usize)) -> Option<usize> {
        /* Looks for a message widget in sight among all elements in rows.list.list of RoomHistory
        whose corresponding index lies in the closed interval [range.0, range.1]. */
        if range.0 > range.1 {
            return None;
        }
        let middle_index = (range.0 + range.1) / 2;
        let element = &self.list[middle_index];
        let scrolled_window = self.view.get_scrolled_window();
        let index = match get_rel_position(&scrolled_window, element) {
            RelativePosition::AboveSight => {
                if range.0 == range.1 {
                    return None;
                } else {
                    self.find_visible_index((range.0, middle_index))
                }
            }
            RelativePosition::InSight => Some(middle_index),
            RelativePosition::BelowSight => {
                if range.0 == range.1 {
                    return None;
                } else {
                    self.find_visible_index((middle_index + 1, range.1))
                }
            }
        };
        index
    }

    fn add_while_visible<'a, T>(&self, indices: &mut Vec<usize>, iterator: T)
    where
        T: Iterator<Item = (usize, &'a Element)>,
    {
        let scrolled_window = self.view.get_scrolled_window();
        for (index, element) in iterator {
            match get_rel_position(&scrolled_window, element) {
                RelativePosition::InSight => {
                    indices.push(index);
                }
                _ => {
                    break;
                }
            }
        }
    }
}

fn get_rel_position(scrolled_window: &gtk::ScrolledWindow, element: &Element) -> RelativePosition {
    let widget = element.get_listbox_row();
    let height_visible_area = gtk::WidgetExt::get_allocated_height(scrolled_window);
    let height_widget = gtk::WidgetExt::get_allocated_height(widget);
    let rel_y = gtk::WidgetExt::translate_coordinates(widget, scrolled_window, 0, 0)
        .expect("Both scrolled_window and widget should be realized and share a common toplevel.")
        .1;
    if rel_y <= -height_widget {
        RelativePosition::AboveSight
    } else if rel_y < height_visible_area {
        RelativePosition::InSight
    } else {
        RelativePosition::BelowSight
    }
}

#[derive(Clone, Debug)]
enum RelativePosition {
    InSight,
    AboveSight,
    BelowSight,
}

/* These Enum contains all differnet types of rows the room history can have, e.g room message, new
 * message divider, day divider */
#[derive(Clone)]
enum Element {
    Message(MessageContent),
    NewDivider(widgets::NewMessageDivider),
    DayDivider(gtk::ListBoxRow),
}

impl Element {
    fn get_listbox_row(&self) -> &gtk::ListBoxRow {
        match self {
            Element::Message(content) => content
                .widget
                .as_ref()
                .expect("The content of every message element must have widget.")
                .get_listbox_row(),
            Element::NewDivider(widgets) => widgets.get_widget(),
            Element::DayDivider(widget) => widget,
        }
    }
}

pub struct RoomHistory {
    /* Contains a list of msg ids to keep track of the displayed messages */
    rows: Rc<RefCell<List>>,
    backend: Sender<BKResponse>,
    server_url: Url,
    source_id: Rc<RefCell<Option<source::SourceId>>>,
    queue: Rc<RefCell<VecDeque<MessageContent>>>,
}

impl RoomHistory {
    pub fn new(actions: SimpleActionGroup, room_id: RoomId, op: &AppOp) -> Option<RoomHistory> {
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
        listbox.insert_action_group("message", Some(&actions));
        let mut rh = RoomHistory {
            rows: Rc::new(RefCell::new(List::new(scroll, listbox))),
            backend: op.backend.clone(),
            server_url: op.login_data.clone()?.server_url.clone(),
            source_id: Rc::new(RefCell::new(None)),
            queue: Rc::new(RefCell::new(VecDeque::new())),
        };

        rh.connect_video_auto_play();
        rh.connect_video_focus();

        Some(rh)
    }

    pub fn create(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        mut messages: Vec<MessageContent>,
    ) -> Option<()> {
        let mut position = messages.len();
        /* Find position of last viewed message */
        for (i, item) in messages.iter().enumerate() {
            if item.last_viewed {
                position = i + 1;
            }
        }
        let bottom = messages.split_off(position);
        messages.reverse();
        self.add_old_messages_in_batch(thread_pool.clone(), user_info_cache.clone(), messages);
        /* Add the rest of the messages after the new message divider */
        self.add_new_messages_in_batch(thread_pool, user_info_cache, bottom);

        let weak_rows = Rc::downgrade(&self.rows);
        let id = timeout_add(250, move || {
            weak_rows.upgrade().map(|rows| {
                rows.borrow_mut().update_videos();
            });
            Continue(false)
        });
        self.rows.borrow_mut().video_scroll_debounce = Some(id);

        None
    }

    fn connect_video_auto_play(&self) {
        let scrollbar = self
            .rows
            .borrow()
            .view
            .get_scrolled_window()
            .get_vscrollbar()
            .expect("The scrolled window must have a vertical scrollbar.")
            .downcast::<gtk::Scrollbar>()
            .unwrap();
        let weak_rows = Rc::downgrade(&self.rows);
        scrollbar.connect_value_changed(move |sb| {
            if !sb.get_state_flags().contains(gtk::StateFlags::BACKDROP) {
                /* Fractal is focused */
                weak_rows.upgrade().map(|rows| {
                    let weak_rows_inner = weak_rows.clone();
                    let new_id = timeout_add(250, move || {
                        weak_rows_inner.upgrade().map(|rows| {
                            rows.borrow_mut().update_videos();
                            rows.borrow_mut().video_scroll_debounce = None;
                        });
                        Continue(false)
                    });
                    if let Some(old_id) = rows.borrow_mut().video_scroll_debounce.replace(new_id) {
                        let _ = Source::remove(old_id);
                    }
                });
            }
        });
    }

    fn connect_video_focus(&mut self) {
        let scrolled_window = self.rows.borrow().view.get_scrolled_window();
        let weak_rows = Rc::downgrade(&self.rows);
        scrolled_window.connect_map(move |_| {
            /* The user has navigated back into the room history */
            weak_rows.upgrade().map(|rows| {
                let len = rows.borrow().playing_videos.len();
                if len != 0 {
                    warn!(
                        "{:?} videos were playing while the room history was not displayed.",
                        len
                    );
                    for (player, hander_id) in rows.borrow_mut().playing_videos.drain(..) {
                        player.stop_loop(hander_id);
                    }
                }
                let visible_videos = rows.borrow().find_visible_videos();
                let mut videos = Vec::with_capacity(visible_videos.len());
                for player in visible_videos {
                    let handler_id = player.play_in_loop();
                    videos.push((player, handler_id));
                }
                rows.borrow_mut().playing_videos = videos;
            });
        });

        let weak_rows = Rc::downgrade(&self.rows);
        scrolled_window.connect_unmap(move |_| {
            /* The user has navigated out of the room history */
            weak_rows.upgrade().map(|rows| {
                if let Some(id) = rows.borrow_mut().video_scroll_debounce.take() {
                    let _ = Source::remove(id);
                }
                for (player, handler_id) in rows.borrow_mut().playing_videos.drain(..) {
                    player.stop_loop(handler_id);
                }
            });
        });

        let weak_rows = Rc::downgrade(&self.rows);
        scrolled_window.connect_state_flags_changed(move |window, flag| {
            if window.get_mapped() {
                /* The room history is being displayed */
                weak_rows.upgrade().map(|rows| {
                    let focused = gtk::StateFlags::BACKDROP;
                    if flag.contains(focused) {
                        /* Fractal has been focused */
                        let len = rows.borrow().playing_videos.len();
                        if len != 0 {
                            warn!(
                                "{:?} videos were playing while Fractal was focused out.",
                                len
                            );
                            for (player, handler_id) in rows.borrow_mut().playing_videos.drain(..) {
                                player.stop_loop(handler_id);
                            }
                        }
                        let visible_videos = rows.borrow().find_visible_videos();
                        let mut videos = Vec::with_capacity(visible_videos.len());
                        for player in visible_videos {
                            let handler_id = player.play_in_loop();
                            videos.push((player, handler_id));
                        }
                        rows.borrow_mut().playing_videos = videos;
                    } else {
                        /* Fractal has been unfocused */
                        if let Some(id) = rows.borrow_mut().video_scroll_debounce.take() {
                            let _ = Source::remove(id);
                        }
                        for (player, handler_id) in rows.borrow_mut().playing_videos.drain(..) {
                            player.stop_loop(handler_id);
                        }
                    }
                });
            }
        });
    }

    fn run_queue(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
    ) -> Option<()> {
        let backend = self.backend.clone();
        let queue = self.queue.clone();
        let rows = self.rows.clone();

        /* TO-DO: we could set the listbox height the 52 * length of messages, to decrease jumps of the
         * scrollbar. 52 is the normal height of a message with one line
         * self.listbox.set_size_request(-1, 52 * messages.len() as i32); */

        if self.source_id.borrow().is_some() {
            /* We don't need a new loop, just keeping the old one */
        } else {
            /* Lacy load initial messages */
            let source_id = self.source_id.clone();
            let server_url = self.server_url.clone();
            *self.source_id.borrow_mut() = Some(gtk::idle_add(move || {
                let mut data = queue.borrow_mut();
                if let Some(mut item) = data.pop_front() {
                    let last = data.front();
                    let mut prev_day_divider = None;
                    let mut day_divider = None;

                    if let Some(first) = rows.borrow().list.back() {
                        match first {
                            Element::Message(ref message) => {
                                if item.date.day() != message.date.day() {
                                    prev_day_divider =
                                        Some(Element::DayDivider(create_day_divider(message.date)));
                                }
                            }
                            _ => (),
                        }
                    };
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

                    if let Some(prev_day_divider) = prev_day_divider {
                        rows.borrow_mut().add_top(prev_day_divider);
                    }
                    if item.last_viewed && !rows.borrow().list.is_empty() {
                        let divider =
                            Element::NewDivider(List::create_new_message_divider(rows.clone()));
                        rows.borrow_mut().add_top(divider);
                        let new_divider_index = rows.borrow().list.len() - 1;
                        rows.borrow_mut().new_divider_index = Some(new_divider_index);
                    }
                    item.widget = Some(create_row(
                        thread_pool.clone(),
                        user_info_cache.clone(),
                        item.clone(),
                        has_header,
                        backend.clone(),
                        server_url.clone(),
                        &rows,
                    ));
                    rows.borrow_mut().add_top(Element::Message(item));
                    if let Some(day_divider) = day_divider {
                        rows.borrow_mut().add_top(day_divider);
                    }
                } else {
                    /* Remove the source id, since the closure is destroyed */
                    source_id.borrow_mut().take();
                    return Continue(false);
                }
                return Continue(true);
            }));
        }
        None
    }

    pub fn destroy(self) {
        if let Some(id) = self.source_id.borrow_mut().take() {
            source::source_remove(id);
        }
    }

    /* This is a temporary function to make the listbox accessible from outside the history, it is
     * currently needed for temp messages (which should also be moved to the room history) */
    pub fn get_listbox(&self) -> gtk::ListBox {
        let listbox = self.rows.borrow().listbox.clone();
        listbox
    }

    /* This adds new incomming messages at then end of the list */
    pub fn add_new_message(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        mut item: MessageContent,
    ) -> Option<()> {
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
            let divider = Element::NewDivider(List::create_new_message_divider(self.rows.clone()));
            rows.add_bottom(divider);
            let new_divider_index = rows.list.len() - 1;
            rows.new_divider_index = Some(new_divider_index);
        }
        if let Some(day_divider) = day_divider {
            rows.add_bottom(day_divider);
        }

        let b = create_row(
            thread_pool,
            user_info_cache,
            item.clone(),
            has_header,
            self.backend.clone(),
            self.server_url.clone(),
            &self.rows,
        );
        item.widget = Some(b);
        rows.add_bottom(Element::Message(item));
        None
    }

    pub fn remove_message(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        item: MessageContent,
    ) -> Option<()> {
        let mut rows = self.rows.borrow_mut();
        let (i, ref mut msg) = rows
            .list
            .iter_mut()
            .enumerate()
            .find_map(|(i, e)| match e {
                Element::Message(ref mut itermessage) if itermessage.id == item.id => {
                    Some((i, itermessage))
                }
                _ => None,
            })?;

        let msg_widget = msg.widget.clone()?;
        let msg_sender = msg.sender.clone();
        msg.msg.redacted = true;
        rows.remove_item(i, msg_widget.get_listbox_row());

        // If the redacted message was a header message let's set
        // the header on the next non-redacted message instead.
        if msg_widget.header {
            let rows_list_len = rows.list.len();
            if let Some((msg_next_cloned, msg_widget)) = rows
                .list
                .iter_mut()
                .rev()
                .skip(rows_list_len - i)
                .filter_map(|message_next| match message_next {
                    Element::Message(ref mut msg_next) => {
                        let msg_next_cloned = msg_next.clone();
                        msg_next
                            .widget
                            .as_mut()
                            .filter(|_| !msg_next_cloned.msg.redacted)
                            .map(|msg_widet| (msg_next_cloned, msg_widet))
                    }
                    _ => None,
                })
                .next()
                .filter(|(msg_next_cloned, _)| {
                    msg_next_cloned.redactable && msg_next_cloned.sender == msg_sender
                })
            {
                msg_widget.update_header(thread_pool, user_info_cache, msg_next_cloned, true);
            }
        }
        None
    }

    pub fn add_new_messages_in_batch(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        messages: Vec<MessageContent>,
    ) -> Option<()> {
        /* TODO: use lazy loading */
        for item in messages {
            self.add_new_message(thread_pool.clone(), user_info_cache.clone(), item);
        }
        None
    }

    pub fn add_old_messages_in_batch(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        messages: Vec<MessageContent>,
    ) -> Option<()> {
        self.rows.borrow().view.reset_request_sent();
        /* TODO: Try if extend would be faster then append */
        self.queue
            .borrow_mut()
            .append(&mut VecDeque::from(messages));
        self.run_queue(thread_pool, user_info_cache);

        None
    }

    pub fn typing_notification(&mut self, typing_str: &str) {
        self.rows.borrow().view.typing_notification(typing_str);
    }

    pub fn page_up(&mut self) {
        let scrolled_window = self.rows.borrow().view.get_scrolled_window();
        widgets::page_up(scrolled_window);
    }

    pub fn page_down(&self) {
        let scrolled_window = self.rows.borrow().view.get_scrolled_window();
        widgets::page_down(scrolled_window);
    }
}

/* This function creates the content for a Row based on the content of msg */
fn create_row(
    thread_pool: ThreadPool,
    user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
    row: MessageContent,
    has_header: bool,
    backend: Sender<BKResponse>,
    server_url: Url,
    rows: &Rc<RefCell<List>>,
) -> widgets::MessageBox {
    /* we need to create a message with the username, so that we don't have to pass
     * all information to the widget creating each row */
    let mut mb = widgets::MessageBox::new(backend, server_url);
    mb.create(
        thread_pool,
        user_info_cache,
        &row,
        has_header && row.mtype != RowType::Emote,
        false,
    );

    match row.mtype {
        RowType::Video => {
            /* The followign callback requires `Send` but is handled by the gtk main loop */
            let fragile_rows = Fragile::new(Rc::downgrade(rows));
            PlayerExt::get_player(&mb.get_video_widget()
                .expect("The widget of every MessageContent, whose mtype is RowType::Video, must have a video_player."))
                .connect_uri_loaded(move |player, _| {
                    fragile_rows.get().upgrade().map(|rows| {
                        if rows.borrow().playing_videos.iter().any(|(player_widget, _)| {
                            &PlayerExt::get_player(&player_widget) == player
                        }) {
                            player.play();
                        }
                    });
                });
        }
        _ => {}
    }
    mb
}

/* returns if two messages should have only a single header or not */
fn should_group_message(msg: &MessageContent, prev: &MessageContent) -> bool {
    if msg.sender == prev.sender && !prev.msg.redacted {
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
    let stamp = if let Some(gstr) = gdate.format(&format) {
        gstr.to_string()
    } else {
        // Fallback to a non glib time string
        date.format(&format).to_string()
    };
    let row = gtk::ListBoxRow::new();
    row.get_style_context().add_class("divider");
    row.set_margin_top(24);
    row.set_selectable(false);
    row.set_activatable(false);
    let label = gtk::Label::new(Some(stamp.as_str()));
    label.set_selectable(false);
    row.add(&label);

    row.show_all();
    row
}
