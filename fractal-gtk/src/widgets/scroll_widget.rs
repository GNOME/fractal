use std::cell::Cell;
use std::rc::Rc;

use gdk::FrameClockExt;
use gio::Action;
use gio::ActionExt;
use gtk;
use gtk::prelude::*;
use pango;

use libhandy;
use libhandy::ColumnExt;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
enum Position {
    Top,
    Bottom,
}

#[allow(dead_code)]
pub struct ScrollWidget {
    upper: Rc<Cell<f64>>,
    value: Rc<Cell<f64>>,
    balance: Rc<Cell<Option<Position>>>,
    autoscroll: Rc<Cell<bool>>,
    /* whether a request for more messages has been send or not */
    request_sent: Rc<Cell<bool>>,
    widgets: Widgets,
}

pub struct Widgets {
    container: gtk::Widget,
    view: gtk::ScrolledWindow,
    button: gtk::Button,
    btn_revealer: gtk::Revealer,
    listbox: gtk::ListBox,
    spinner: gtk::Spinner,
    typing_label: gtk::Label,
}

impl Widgets {
    pub fn new(builder: gtk::Builder) -> Widgets {
        let view = builder
            .get_object::<gtk::ScrolledWindow>("messages_scroll")
            .expect("Can't find message_scroll in ui file.");
        let button = builder
            .get_object::<gtk::Button>("scroll_btn")
            .expect("Can't find scroll_btn in ui file.");
        let btn_revealer = builder
            .get_object::<gtk::Revealer>("scroll_btn_revealer")
            .expect("Can't find scroll_btn_revealer in ui file.");
        let main_container = builder
            .get_object::<gtk::Widget>("history")
            .expect("Can't find history in ui file.");
        /* create a width constrained column and add message_box to the UI */
        let container = builder
            .get_object::<gtk::Box>("message_column")
            .expect("Can't find message_column in ui file.");
        // Create the listbox insteate of the following line
        //let messages = self.op.lock().unwrap().message_box.clone();
        let messages = gtk::ListBox::new();
        let column = libhandy::Column::new();
        column.set_maximum_width(800);
        column.set_linear_growth_width(600);
        /* For some reason the Column is not seen as a gtk::container
         * and therefore we can't call add() without the cast */
        let column = column.upcast::<gtk::Widget>();
        let column = column.downcast::<gtk::Container>().unwrap();
        column.set_hexpand(true);
        column.set_vexpand(true);

        let typing_label = gtk::Label::new(None);
        typing_label.show();
        typing_label
            .get_style_context()
            .unwrap()
            .add_class("typing_label");
        typing_label.set_xalign(0.0);
        typing_label.set_ellipsize(pango::EllipsizeMode::End);
        typing_label.set_visible(false);

        let column_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column_box.add(&messages);
        column_box.add(&typing_label);
        column_box.show();
        column.add(&column_box);
        column.show();

        messages
            .get_style_context()
            .unwrap()
            .add_class("messages-history");
        messages.show();

        container
            .get_style_context()
            .unwrap()
            .add_class("messages-box");
        container.add(&column);

        /* add a load more Spinner */
        let spinner = gtk::Spinner::new();
        messages.add(&create_load_more_spn(&spinner));

        Widgets {
            container: main_container,
            view,
            button,
            btn_revealer,
            listbox: messages,
            spinner,
            typing_label,
        }
    }
}

impl ScrollWidget {
    pub fn new(action: Option<Action>, room_id: String) -> ScrollWidget {
        let builder = gtk::Builder::new();

        builder
            .add_from_resource("/org/gnome/Fractal/ui/scroll_widget.ui")
            .expect("Can't load ui file: scroll_widget.ui");

        let widgets = Widgets::new(builder);
        let upper = widgets
            .view
            .get_vadjustment()
            .and_then(|adj| Some(adj.get_upper()))
            .unwrap_or_default();
        let value = widgets
            .view
            .get_vadjustment()
            .and_then(|adj| Some(adj.get_value()))
            .unwrap_or_default();

        let mut scroll = ScrollWidget {
            widgets,
            value: Rc::new(Cell::new(value)),
            upper: Rc::new(Cell::new(upper)),
            autoscroll: Rc::new(Cell::new(false)),
            request_sent: Rc::new(Cell::new(false)),
            balance: Rc::new(Cell::new(None)),
        };
        scroll.connect(action, room_id);
        scroll
    }

    /* Keep the same position if new messages are added */
    pub fn connect(&mut self, action: Option<Action>, room_id: String) -> Option<()> {
        let adj = self.widgets.view.get_vadjustment()?;
        let upper = Rc::downgrade(&self.upper);
        let balance = Rc::downgrade(&self.balance);
        let autoscroll = Rc::downgrade(&self.autoscroll);
        let view = self.widgets.view.downgrade();
        adj.connect_property_upper_notify(move |adj| {
            let check = || -> Option<()> {
                let view = view.upgrade()?;
                let upper = upper.upgrade()?;
                let balance = balance.upgrade()?;
                let autoscroll = autoscroll.upgrade()?;
                let new_upper = adj.get_upper();
                let diff = new_upper - upper.get();
                /* Don't do anything if upper didn't change */
                if diff != 0.0 {
                    upper.set(new_upper);
                    /* Stay at the end of the room history when autoscroll is on */
                    if autoscroll.get() {
                        adj.set_value(adj.get_upper() - adj.get_page_size());
                    } else if balance.take().map_or(false, |x| x == Position::Top) {
                        adj.set_value(adj.get_value() + diff);
                        view.set_kinetic_scrolling(true);
                    }
                }
                Some(())
            }();
            debug_assert!(
                check.is_some(),
                "Upper notify callback couldn't acquire a strong pointer"
            );
        });

        let autoscroll = Rc::downgrade(&self.autoscroll);
        let request_sent = Rc::downgrade(&self.request_sent);
        let revealer = self.widgets.btn_revealer.downgrade();
        let spinner = self.widgets.spinner.downgrade();
        let action_weak = action.map(|a| a.downgrade());
        adj.connect_value_changed(move |adj| {
            let check = || -> Option<()> {
                let autoscroll = autoscroll.upgrade()?;
                let r = revealer.upgrade()?;

                let bottom = adj.get_upper() - adj.get_page_size();
                if adj.get_value() == bottom {
                    r.set_reveal_child(false);
                    autoscroll.set(true);
                } else {
                    r.set_reveal_child(true);
                    autoscroll.set(false);
                }
                Some(())
            }();
            debug_assert!(
                check.is_some(),
                "Value changed callback couldn't acquire a strong pointer"
            );

            let action_weak = action_weak.clone();
            let check = || -> Option<()> {
                let request_sent = request_sent.upgrade()?;
                if !request_sent.get() {
                    let action = action_weak?.upgrade()?;
                    let spinner = spinner.upgrade()?;
                    /* the page size twice to detect if the user gets close the edge */
                    if adj.get_value() < adj.get_page_size() * 2.0 {
                        /* Load more messages once the user is nearly at the end of the history */
                        spinner.start();
                        let data = glib::Variant::from(&room_id);
                        action.activate(&data);
                        request_sent.set(true);
                    }
                }

                Some(())
            }();
            debug_assert!(check.is_some(), "Can't request more messages");
        });

        let autoscroll = Rc::downgrade(&self.autoscroll);
        let revealer = self.widgets.btn_revealer.downgrade();
        let scroll = self.widgets.view.downgrade();
        self.widgets.button.connect_clicked(move |_| {
            let check = || -> Option<()> {
                let autoscroll = autoscroll.upgrade()?;
                let r = revealer.upgrade()?;
                let s = scroll.upgrade()?;
                r.set_reveal_child(false);
                autoscroll.set(true);
                scroll_down(&s, true);
                Some(())
            }();
            debug_assert!(
                check.is_some(),
                "Scroll down button onclick callback couldn't acquire a strong pointer"
            );
        });

        None
    }

    pub fn set_balance_top(&self) {
        /* FIXME: Workaround: https://gitlab.gnome.org/GNOME/gtk/merge_requests/395 */
        self.widgets.view.set_kinetic_scrolling(false);
        self.balance.set(Some(Position::Top));
    }
    pub fn get_listbox(&self) -> gtk::ListBox {
        self.widgets.listbox.clone()
    }
    pub fn get_container(&self) -> gtk::Widget {
        self.widgets.container.clone()
    }
    pub fn reset_request_sent(&self) {
        self.request_sent.set(false);
        self.widgets.spinner.stop();
    }

    pub fn typing_notification(&self, typing_str: &str) {
        if typing_str.len() == 0 {
            self.widgets.typing_label.set_visible(false);
        } else {
            self.widgets.typing_label.set_visible(true);
            self.widgets.typing_label.set_text(typing_str);
        }
    }
}

/* Functions to animate the scroll */
fn scroll_down(ref view: &gtk::ScrolledWindow, animate: bool) -> Option<()> {
    let adj = view.get_vadjustment()?;
    if animate {
        let clock = view.get_frame_clock()?;
        let duration = 200;
        let start = adj.get_value();
        let start_time = clock.get_frame_time();
        let end_time = start_time + 1000 * duration;
        view.add_tick_callback(move |view, clock| {
            let now = clock.get_frame_time();
            if let Some(adj) = view.get_vadjustment() {
                let end = adj.get_upper() - adj.get_page_size();
                if now < end_time && adj.get_value().round() != end.round() {
                    let mut t = (now - start_time) as f64 / (end_time - start_time) as f64;
                    t = ease_out_cubic(t);
                    adj.set_value(start + t * (end - start));
                    return glib::Continue(true);
                } else {
                    adj.set_value(end);
                    return glib::Continue(false);
                }
            }
            return glib::Continue(false);
        });
    } else {
        adj.set_value(adj.get_upper() - adj.get_page_size());
    }
    None
}

/* From clutter-easing.c, based on Robert Penner's
 * infamous easing equations, MIT license.
 */
fn ease_out_cubic(t: f64) -> f64 {
    let p = t - 1f64;
    return p * p * p + 1f64;
}

/* create load more spinner for the listbox */
fn create_load_more_spn(spn: &gtk::Spinner) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    spn.set_halign(gtk::Align::Center);
    spn.set_margin_top(12);
    spn.set_margin_bottom(12);
    spn.show();
    row.add(spn);
    row.show();
    row
}
