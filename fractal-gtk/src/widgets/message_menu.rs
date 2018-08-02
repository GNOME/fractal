extern crate gdk;
extern crate gtk;
extern crate sourceview;

use std::cell::RefCell;
use std::rc::Rc;

use self::gtk::prelude::*;
use self::sourceview::prelude::*;
use backend::BKCommand;
use std::sync::mpsc::Sender;

use types::Message;

#[derive(Clone)]
pub struct MessageMenu {
    builder: gtk::Builder,
    backend: Sender<BKCommand>,
    source_dialog: gtk::MessageDialog,
    source_buffer: sourceview::Buffer,
    msg_entry: sourceview::View,
    pub msg: Message,
}

impl MessageMenu {
    /* FIXME: Remove widget references, but I'm not sure how we can do that */
    pub fn new_message_menu(source_dialog: gtk::MessageDialog, source_buffer: sourceview::Buffer, msg_entry: sourceview::View, backend: Sender<BKCommand>, msg: Message) -> MessageMenu {
        let builder = gtk::Builder::new();
        builder.add_from_resource("/org/gnome/Fractal/ui/message_menu.ui")
            .expect("Can't load ui file: message_menu.ui");

        let menu = MessageMenu {
            builder,
            backend,
            source_dialog,
            source_buffer,
            msg_entry,
            msg,
        };
        menu.connect_message_menu();
        menu
    }

    pub fn show_menu_popover(&self, w: gtk::Widget) {
        let menu_popover: gtk::Popover = self.builder
            .get_object("message_menu_popover")
            .expect("Can't find message_menu_popover in ui file.");
        menu_popover.set_relative_to(&w);

        menu_popover.popup();
    }

    pub fn insert_quote(&self) {
        if let Some(buffer) = self.msg_entry.get_buffer() {
            let quote = self.msg.body.lines().map(|l| "> ".to_owned() + l + "\n")
                                .collect::<Vec<String>>().join("\n") + "\n";

            let mut start = buffer.get_start_iter();
            buffer.insert(&mut start, &quote);

            self.msg_entry.grab_focus();
        }
    }

    pub fn copy_text(&self) {
        let atom = gdk::Atom::intern("CLIPBOARD");
        let clipboard = gtk::Clipboard::get(&atom);

        clipboard.set_text(&self.msg.body);
    }

    pub fn display_source_dialog(&self) {
        let json_lang = sourceview::LanguageManager::get_default()
                                                   .map_or(None, |lm| lm.get_language("json"));

        self.source_buffer.set_highlight_matching_brackets(false);
        if let Some(json_lang) = json_lang.clone() {
            self.source_buffer.set_language(&json_lang);
            self.source_buffer.set_highlight_syntax(true);
        }

        self.source_buffer.set_text(self.msg.source.clone()
                                       .unwrap_or("This message has no source.".to_string())
                                       .as_str());

        let source_buffer = self.source_buffer.clone();
        self.source_dialog.connect_response(move |d, res| {
            if gtk::ResponseType::from(res) == gtk::ResponseType::Accept {
                let atom = gdk::Atom::intern("CLIPBOARD");
                let clipboard = gtk::Clipboard::get(&atom);

                let start_iter = source_buffer.get_start_iter();
                let end_iter = source_buffer.get_end_iter();

                if let Some(src) = source_buffer.get_text(&start_iter, &end_iter, false) {
                    clipboard.set_text(&src);
                }
            } else {
                d.hide();
            }
        });

        self.source_dialog.show();
    }

    pub fn connect_message_menu(&self) {
        let reply_button: gtk::ModelButton = self.builder
            .get_object("reply_button")
            .expect("Can't find reply_button in ui file.");

        let copy_text_button: gtk::ModelButton = self.builder
            .get_object("copy_text_button")
            .expect("Can't find copy_text_button in ui file.");

        let delete_message_button: gtk::ModelButton = self.builder
            .get_object("delete_message_button")
            .expect("Can't find delete_message_button in ui file.");

        let view_source_button: gtk::ModelButton = self.builder
            .get_object("view_source_button")
            .expect("Can't find view_source_button in ui file.");

        /* since this is used only by the main thread we can just use a simple Rc<RefCell> */
        let this: Rc<RefCell<MessageMenu>> = Rc::new(RefCell::new(self.clone()));

        reply_button.connect_clicked(clone!(this => move |_| {
            this.borrow().insert_quote();
        }));

        copy_text_button.connect_clicked(clone!(this => move |_| {
            this.borrow().copy_text();
        }));

        let backend = self.backend.clone();
        delete_message_button.connect_clicked(clone!(this => move |_| {
            backend.send(BKCommand::SendMsgRedaction(this.borrow().msg.clone())).unwrap();
        }));

        view_source_button.connect_clicked(clone!(this => move |_| {
            this.borrow().display_source_dialog();
        }));
    }
}
