extern crate gtk;

use self::gtk::prelude::*;

pub fn new(text: &str) -> gtk::Box {
    let divider = gtk::Box::new(gtk::Orientation::Horizontal, 6);

    let left_separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    left_separator.set_valign(gtk::Align::Center);
    let label = gtk::Label::new(text);
    label.set_selectable(false);
    let right_separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    right_separator.set_valign(gtk::Align::Center);

    divider.pack_start(&left_separator, true, true, 0);
    divider.pack_start(&label, false, false, 0);
    divider.pack_start(&right_separator, true, true, 0);

    divider.show_all();
    divider
}
