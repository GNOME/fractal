use crate::appop::attach;
use fractal_api::clone;
use gdk;
use gtk;
use gtk::prelude::*;
use sourceview4::BufferExt;

use crate::actions::activate_action;
use crate::app::App;

const MAX_INPUT_HEIGHT: i32 = 100;

impl App {
    pub fn connect_send(&self) {
        self.ui.sventry.container.set_redraw_on_allocate(true);
        let msg_entry = self.ui.sventry.view.clone();
        let buffer = &self.ui.sventry.buffer;
        buffer.set_highlight_matching_brackets(false);

        let msg_entry_box = self.ui.sventry.entry_box.clone();
        msg_entry_box.set_redraw_on_allocate(true);

        if let Some(adjustment) = self.ui.sventry.scroll.get_vadjustment() {
            adjustment.connect_value_changed(clone!(msg_entry => move |adj| {
                if msg_entry.get_allocated_height() < MAX_INPUT_HEIGHT {
                    adj.set_value(0.0);
                }
            }));
        }

        let autocomplete_popover = self
            .ui
            .builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");

        msg_entry.connect_key_press_event(move |_, key| match key.get_keyval() {
            gdk::enums::key::Return | gdk::enums::key::KP_Enter
                if !key.get_state().contains(gdk::ModifierType::SHIFT_MASK)
                    && !autocomplete_popover.is_visible() =>
            {
                activate_action("app", "send-message");
                Inhibit(true)
            }
            _ => Inhibit(false),
        });

        let mut op = self.op.clone();
        msg_entry.connect_key_release_event(move |_, _| {
            op.lock().unwrap().send_typing();
            Inhibit(false)
        });

        op = self.op.clone();
        msg_entry.connect_paste_clipboard(move |_| {
            attach::paste(op.clone());
        });

        msg_entry.connect_focus_in_event(clone!(msg_entry_box => move |_, _| {
            msg_entry_box.get_style_context().add_class("message-input-focused");

            Inhibit(false)
        }));

        msg_entry.connect_focus_out_event(clone!(msg_entry_box => move |_, _| {
            msg_entry_box.get_style_context().remove_class("message-input-focused");

            Inhibit(false)
        }));
    }
}
