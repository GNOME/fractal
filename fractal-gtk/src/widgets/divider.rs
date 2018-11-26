use gtk;
use gtk::prelude::*;
use gtk::RevealerTransitionType;

#[derive(Clone)]
pub struct NewMessageDivider {
    revealer: gtk::Revealer,
    widget: gtk::ListBoxRow,
}

impl NewMessageDivider {
    pub fn new(text: &str) -> NewMessageDivider {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(false);

        let divider = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if let Some(style) = divider.get_style_context() {
            style.add_class("divider");
        }
        divider.set_margin_top(24);
        divider.set_margin_bottom(6);

        let left_separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        left_separator.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(text);
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
        revealer.set_transition_duration(100);

        row.add(&revealer);
        row.show_all();

        /* Self destruction
         * destroy the NewMessageDivider after it's added to the History with a couple of
         * secounds delay */
        let revealer_weak = revealer.downgrade();
        row.connect_parent_set(move |w, _| {
            || -> Option<()> {
                let revealer = revealer_weak.upgrade()?;
                let revealer_weak = revealer.downgrade();
                gtk::timeout_add(5000, move || {
                    /* when the user closes the room the divider gets destroyed and this tiemout
                     * does nothing, but that's fine */
                    || -> Option<()> {
                        let r = revealer_weak.upgrade()?;
                        r.set_reveal_child(false);
                        None
                    }();
                    glib::Continue(false)
                });
                let row_weak = w.downgrade();
                revealer.connect_property_child_revealed_notify(move |_| {
                    || -> Option<()> {
                        let r = row_weak.upgrade()?;
                        r.destroy();
                        None
                    }();
                });
                None
            }();
        });
        NewMessageDivider {
            revealer: revealer,
            widget: row,
        }
    }

    pub fn get_widget(&self) -> &gtk::ListBoxRow {
        &self.widget
    }
}
