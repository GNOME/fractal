use crate::app::App;
use crate::backend::BKCommand;

use gtk::prelude::*;

// The TextBufferExt alias is necessary to avoid conflict with gtk's TextBufferExt
use gspell::{CheckerExt, TextBuffer, TextBufferExt as GspellTextBufferExt};

impl App {
    pub fn connect_language(&self) {
        let textview = self.ui.sventry.view.upcast_ref::<gtk::TextView>();
        if let Some(checker) = textview
            .get_buffer()
            .and_then(|gtk_buffer| TextBuffer::get_from_gtk_text_buffer(&gtk_buffer))
            .and_then(|gs_buffer| gs_buffer.get_spell_checker())
        {
            let op = self.op.clone();
            let _signal_handler = checker.connect_property_language_notify(move |checker| {
                if let Some(lang_code) = checker
                    .get_language()
                    .and_then(|lang| lang.get_code())
                    .map(|lang_code| String::from(lang_code))
                {
                    /*If the checker is modified by fn set_language in fractal-gtk/src/appop/room.rs
                    due to the user switching rooms, the op mutex is locked already.
                    If the checker is modified by gtk due to the user switching the language, the op mutex is unlocked. */
                    if let Ok(op) = op.try_lock() {
                        if let Some(active_room) = &op.active_room {
                            op.backend
                                .send(BKCommand::ChangeLanguage(lang_code, active_room.clone()))
                                .unwrap();
                        }
                    }
                }
            });
        }
    }
}
