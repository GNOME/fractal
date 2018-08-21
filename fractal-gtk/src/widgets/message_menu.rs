extern crate gdk;
extern crate gtk;
extern crate sourceview;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use self::gtk::prelude::*;
use self::gdk::prelude::*;
use self::sourceview::prelude::*;

use backend::BKCommand;

use uibuilder::UI;
use types::Message;

#[derive(Clone)]
struct SelectedText {
    pub widget: gtk::Label,
    pub text: String,
    pub start: i32,
    pub end: i32,
}

#[derive(Clone)]
pub struct MessageMenu {
    builder: gtk::Builder,
    ui: UI,
    backend: Sender<BKCommand>,
    selected_text: Option<SelectedText>,
    pub msg: Message,
}

impl MessageMenu {
    pub fn new_message_menu(ui: UI,
                            backend: Sender<BKCommand>,
                            msg: Message,
                            event_widget: Option<&gtk::Widget>) -> MessageMenu {
        let builder = gtk::Builder::new();
        builder.add_from_resource("/org/gnome/Fractal/ui/message_menu.ui")
            .expect("Can't load ui file: message_menu.ui");

        let selected_text = get_selected_text(event_widget);

        let menu = MessageMenu {
            builder,
            ui,
            backend,
            selected_text,
            msg,
        };
        menu.connect_message_menu();
        menu.connect_msg_src_window();
        menu
    }

    pub fn show_menu_popover(&self, w: gtk::Widget) {
        let button: gtk::Widget = self.builder
                                      .get_object("copy_selected_text_button")
                                      .expect("Can't find copy_selected_text_button");
        button.set_visible(self.selected_text.is_some());

        gdk::Display::get_default()
            .and_then(|disp| disp.get_default_seat())
            .and_then(|seat| seat.get_pointer())
            .map(|ptr| {
                let win = w.get_window()?;
                let (_, x, y, _) = win.get_device_position(&ptr);

                let menu_popover: gtk::Popover = self.builder
                    .get_object("message_menu_popover")
                    .expect("Can't find message_menu_popover in ui file.");
                let rect = gtk::Rectangle {
                    x,
                    y,
                    width: 0,
                    height: 0,
                };

                menu_popover.set_relative_to(&w);
                menu_popover.set_pointing_to(&rect);
                menu_popover.set_position(gtk::PositionType::Bottom);

                menu_popover.popup();

                Some(true)
            });
    }

    pub fn insert_quote(&self) {
        let msg_entry = &self.ui.sventry.view;

        if let Some(buffer) = msg_entry.get_buffer() {
            let quote = self.msg.body.lines().map(|l| "> ".to_owned() + l)
                                .collect::<Vec<String>>().join("\n") + "\n" + "\n";

            let mut start = buffer.get_start_iter();
            buffer.insert(&mut start, &quote);

            msg_entry.grab_focus();
        }
    }

    pub fn copy_text(&self) {
        let atom = gdk::Atom::intern("CLIPBOARD");
        let clipboard = gtk::Clipboard::get(&atom);

        clipboard.set_text(&self.msg.body);
    }

    pub fn copy_selected_text(&self) {
        let atom = gdk::Atom::intern("CLIPBOARD");
        let clipboard = gtk::Clipboard::get(&atom);

        if let Some(ref s) = self.selected_text {
            clipboard.set_text(&s.text);
            s.widget.select_region(s.start, s.end);
        }
    }

    pub fn display_msg_src_window(&self) {
        let source_buffer: sourceview::Buffer = self.ui.builder
            .get_object("source_buffer")
            .expect("Can't source_buffer in ui file.");

        let msg_src_window: gtk::Window = self.ui.builder
            .get_object("msg_src_window")
            .expect("Can't find msg_src_window in ui file.");

        source_buffer.set_text(self.msg.source.clone()
                                       .unwrap_or("This message has no source.".to_string())
                                       .as_str());

        msg_src_window.show();
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

        let copy_selected_button: gtk::ModelButton = self.builder
            .get_object("copy_selected_text_button")
            .expect("Can't find copy_selected_text_button in ui file.");

        /* since this is used only by the main thread we can just use a simple Rc<RefCell> */
        let this: Rc<RefCell<MessageMenu>> = Rc::new(RefCell::new(self.clone()));

        reply_button.connect_clicked(clone!(this => move |_| {
            this.borrow().insert_quote();
        }));

        copy_text_button.connect_clicked(clone!(this => move |_| {
            this.borrow().copy_text();
        }));

        copy_selected_button.connect_clicked(clone!(this => move |_| {
            this.borrow().copy_selected_text();
        }));

        let backend = self.backend.clone();
        delete_message_button.connect_clicked(clone!(this => move |_| {
            backend.send(BKCommand::SendMsgRedaction(this.borrow().msg.clone())).unwrap();
        }));

        view_source_button.connect_clicked(clone!(this => move |_| {
            this.borrow().display_msg_src_window();
        }));
    }

    pub fn connect_msg_src_window(&self) {
        let msg_src_window: gtk::Window = self.ui.builder
            .get_object("msg_src_window")
            .expect("Can't find msg_src_window in ui file.");

        let copy_src_button: gtk::Button = self.ui.builder
            .get_object("copy_src_button")
            .expect("Can't find copy_src_button in ui file.");

        let close_src_button: gtk::Button = self.ui.builder
            .get_object("close_src_button")
            .expect("Can't find close_src_button in ui file.");

        let source_buffer: sourceview::Buffer = self.ui.builder
            .get_object("source_buffer")
            .expect("Can't find source_buffer in ui file.");

        copy_src_button.connect_clicked(clone!(source_buffer => move |_| {
            let atom = gdk::Atom::intern("CLIPBOARD");
            let clipboard = gtk::Clipboard::get(&atom);

            let start_iter = source_buffer.get_start_iter();
            let end_iter = source_buffer.get_end_iter();

            if let Some(src) = source_buffer.get_text(&start_iter, &end_iter, false) {
                clipboard.set_text(&src);
            }
        }));

        msg_src_window.connect_delete_event(|w, _| {
            Inhibit(w.hide_on_delete())
        });

        let msg_src_window = msg_src_window.clone();
        close_src_button.connect_clicked(move |_| {
            msg_src_window.hide();
        });

        let json_lang = sourceview::LanguageManager::get_default()
                                                   .map_or(None, |lm| lm.get_language("json"));

        source_buffer.set_highlight_matching_brackets(false);
        if let Some(json_lang) = json_lang.clone() {
            source_buffer.set_language(&json_lang);
            source_buffer.set_highlight_syntax(true);

            if let Some(scheme) = sourceview::StyleSchemeManager::get_default().map_or(None, |scm| scm.get_scheme("kate")) {
                source_buffer.set_style_scheme(&scheme);
            }
        }
    }
}


fn get_selected_text(event_widget: Option<&gtk::Widget>) -> Option<SelectedText> {
    let w = event_widget?;
    let w = w.clone().downcast::<gtk::Label>().ok()?;
    match w.get_selection_bounds() {
        Some((s, e)) => {
            let text = w.get_text()?;
            let slice = text.get(s as usize..e as usize)?;
            Some(SelectedText{ widget: w.clone(), text: slice.to_string(), start: s, end: e })
        }
        _ => None
    }
}
