use glib::clone;
use glib::source::Continue;
use gtk::prelude::*;
use gtk::RevealerTransitionType;

#[derive(Clone)]
pub struct NewMessageDivider {
    revealer: gtk::Revealer,
    widget: gtk::ListBoxRow,
}

impl NewMessageDivider {
    pub fn new<F: Fn() + 'static>(text: &str, remove_divider: F) -> NewMessageDivider {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(false);

        let divider = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        divider.get_style_context().add_class("divider");
        divider.set_margin_top(24);
        divider.set_margin_bottom(6);

        let left_separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        left_separator.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(Some(text));
        label.set_selectable(false);
        let right_separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        right_separator.set_valign(gtk::Align::Center);

        divider.pack_start(&left_separator, true, true, 0);
        divider.pack_start(&label, false, false, 0);
        divider.pack_start(&right_separator, true, true, 0);

        let revealer = gtk::Revealer::new();

        revealer.add(&divider);
        revealer.set_transition_type(RevealerTransitionType::None);
        revealer.set_reveal_child(true);
        revealer.set_transition_type(RevealerTransitionType::SlideDown);

        row.add(&revealer);
        row.show_all();

        /* Self destruction
         * destroy the NewMessageDivider after it's added to the History with a couple of
         * seconds delay */
        row.connect_parent_set(clone!(@weak revealer => move |_, _| {
            gtk::timeout_add(5000, clone!(
                @weak revealer as r
                => @default-return Continue(false), move || {
                    /* when the user closes the room the divider gets destroyed and this timeout
                     * does nothing, but that's fine */
                    r.set_reveal_child(false);
                    Continue(false)
                }));
        }));
        revealer.connect_property_child_revealed_notify(clone!(
        @weak row as r
        => move |_| {
            r.destroy();
            remove_divider();
        }));
        NewMessageDivider {
            revealer,
            widget: row,
        }
    }

    pub fn get_widget(&self) -> &gtk::ListBoxRow {
        &self.widget
    }
}
