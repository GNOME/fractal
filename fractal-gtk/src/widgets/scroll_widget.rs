use std::cell::RefCell;
use std::rc::Rc;

use gdk::FrameClockExt;
use gtk;
use gtk::prelude::*;

use libhandy;
use libhandy::ColumnExt;
use App;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
enum Position {
    Top,
    Bottom,
}

#[allow(dead_code)]
pub struct ScrollWidget {
    upper: Rc<RefCell<f64>>,
    value: Rc<RefCell<f64>>,
    balance: Rc<RefCell<Option<Position>>>,
    autoscroll: Rc<RefCell<bool>>,
    widgets: Widgets,
}

pub struct Widgets {
    container: gtk::Widget,
    view: gtk::ScrolledWindow,
    button: gtk::Button,
    btn_revealer: gtk::Revealer,
    listbox: gtk::ListBox,
    spinner: gtk::Spinner,
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
        column.add(&messages);
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
        }
    }
}

impl ScrollWidget {
    pub fn new() -> ScrollWidget {
        let builder = gtk::Builder::new();

        builder
            .add_from_resource("/org/gnome/Fractal/ui/scroll_widget.ui")
            .expect("Can't load ui file: scroll_widget.ui");

        let widgets = Widgets::new(builder);
        let upper = widgets
            .view
            .get_vadjustment()
            .and_then(|adj| Some(adj.get_upper()))
            .unwrap_or(0.0);
        let value = widgets
            .view
            .get_vadjustment()
            .and_then(|adj| Some(adj.get_value()))
            .unwrap_or(0.0);

        ScrollWidget {
            widgets,
            value: Rc::new(RefCell::new(value)),
            upper: Rc::new(RefCell::new(upper)),
            autoscroll: Rc::new(RefCell::new(true)),
            balance: Rc::new(RefCell::new(None)),
        }
    }

    pub fn create(&mut self) -> Option<()> {
        self.connect();
        None
    }

    /* Keep the same position if new messages are added */
    pub fn connect(&mut self) -> Option<()> {
        let adj = self.widgets.view.get_vadjustment()?;
        let upper = self.upper.clone();
        let balance = self.balance.clone();
        let autoscroll = self.autoscroll.clone();
        adj.connect_property_upper_notify(move |adj| {
            let new_upper = adj.get_upper();
            let diff = new_upper - *upper.borrow();
            *upper.borrow_mut() = new_upper;
            /* Stay at the end of the room history when autoscroll is on */
            if *autoscroll.borrow() {
                adj.set_value(adj.get_upper() - adj.get_page_size());
                return;
            }
            if balance
                .borrow()
                .clone()
                .and_then(|x| Some(x == Position::Top))
                .unwrap_or(false)
            {
                adj.set_value(adj.get_value() + diff);
                *balance.borrow_mut() = None;
            }
        });

        let autoscroll = self.autoscroll.clone();
        let r = self.widgets.btn_revealer.clone();
        let spinner = self.widgets.spinner.clone();
        adj.connect_value_changed(move |adj| {
            /* the page size twice to detect if the user gets close the edge */
            if adj.get_value() < adj.get_page_size() * 2.0 {
                /* Load more messages once the user is nearly at the end of the history */
                spinner.start();
                load_more_messages();
            }

            let bottom = adj.get_upper() - adj.get_page_size();
            if adj.get_value() == bottom {
                r.set_reveal_child(false);
                *autoscroll.borrow_mut() = true;
            } else {
                r.set_reveal_child(true);
                *autoscroll.borrow_mut() = false;
            }
        });

        let r = self.widgets.btn_revealer.clone();
        let s = self.widgets.view.clone();
        let autoscroll = self.autoscroll.clone();
        self.widgets.button.connect_clicked(move |_| {
            r.set_reveal_child(false);
            *autoscroll.borrow_mut() = true;
            scroll_down(&s, true);
        });

        None
    }

    pub fn set_balance_top(&self) {
        *self.balance.borrow_mut() = Some(Position::Top);
    }
    pub fn get_listbox(&self) -> gtk::ListBox {
        return self.widgets.listbox.clone();
    }
    pub fn get_container(&self) -> gtk::Widget {
        return self.widgets.container.clone();
    }
    pub fn get_loading_spinner(&self) -> gtk::Spinner {
        return self.widgets.spinner.clone();
    }
}

fn load_more_messages() {
    /* Todo: remove APPOP! and make the call directly */
    APPOP!(load_more_messages);
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
                if now < end_time && adj.get_value() != end {
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
