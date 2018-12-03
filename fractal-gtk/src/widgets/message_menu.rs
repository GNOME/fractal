use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use gdk;
use gdk::prelude::*;
use gdk_pixbuf;
use glib;
use gtk;
use gtk::prelude::*;

use app::App;
use backend::BKCommand;
use i18n::i18n;

use uibuilder::UI;
use uitypes::{MessageContent, RowType};
use widgets::SourceViewer;

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
    pub msg: MessageContent,
}

impl MessageMenu {
    pub fn new_message_menu(
        ui: UI,
        backend: Sender<BKCommand>,
        msg: MessageContent,
        event_widget: Option<&gtk::Label>,
    ) -> MessageMenu {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/message_menu.ui")
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
        menu
    }

    pub fn show_menu_popover(&self, w: gtk::Widget) {
        let copy_button: gtk::Widget = self
            .builder
            .get_object("copy_selected_text_button")
            .expect("Can't find copy_selected_text_button");
        copy_button.set_visible(self.selected_text.is_some());

        let delete_button: gtk::Widget = self
            .builder
            .get_object("delete_message_button")
            .expect("Can't find delete_message_button");
        let message_menu_separator: gtk::Widget = self
            .builder
            .get_object("message_menu_separator")
            .expect("Can't find message_menu_separator");
        delete_button.set_visible(self.msg.redactable);
        message_menu_separator.set_visible(self.msg.redactable);

        let open_with_button: gtk::Widget = self
            .builder
            .get_object("open_with_button")
            .expect("Can't find open_with_button");
        open_with_button.set_visible(self.msg.mtype == RowType::Image);

        let save_image_as_button: gtk::Widget = self
            .builder
            .get_object("save_image_as_button")
            .expect("Can't find save_image_as_button");
        save_image_as_button.set_visible(self.msg.mtype == RowType::Image);

        let copy_image_button: gtk::Widget = self
            .builder
            .get_object("copy_image_button")
            .expect("Can't find copy_image_button");
        copy_image_button.set_visible(self.msg.mtype == RowType::Image);

        let copy_text_button: gtk::Widget = self
            .builder
            .get_object("copy_text_button")
            .expect("Can't find copy_text_button");
        copy_text_button.set_visible(self.msg.mtype != RowType::Image);

        gdk::Display::get_default()
            .and_then(|disp| disp.get_default_seat())
            .and_then(|seat| seat.get_pointer())
            .map(|ptr| {
                let win = w.get_window()?;
                let (_, x, y, _) = win.get_device_position(&ptr);

                let menu_popover: gtk::Popover = self
                    .builder
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
            let quote = self
                .msg
                .body
                .lines()
                .map(|l| "> ".to_owned() + l)
                .collect::<Vec<String>>()
                .join("\n")
                + "\n"
                + "\n";

            let mut start = buffer.get_start_iter();
            buffer.insert(&mut start, &quote);

            msg_entry.grab_focus();
        }
    }

    pub fn open_with(&self) {
        let url = self.msg.url.clone().unwrap_or_default();

        self.backend.send(BKCommand::GetMedia(url)).unwrap();
    }

    pub fn save_image_as(&self) {
        let name = self.msg.body.clone();
        let url = self.msg.url.clone().unwrap_or_default();
        let backend = self.backend.clone();

        let (tx, rx): (Sender<String>, Receiver<String>) = channel();

        backend
            .send(BKCommand::GetMediaAsync(url.clone(), tx))
            .unwrap();

        gtk::timeout_add(
            50,
            clone!(name => move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not download the file");
                    APPOP!(show_error, (msg));

                    gtk::Continue(true)
                },
                Ok(fname) => {
                    let name = name.clone();
                    APPOP!(save_file_as, (fname, name));

                    gtk::Continue(false)
                }
            }),
        );
    }

    pub fn copy_image(&self) {
        let url = self.msg.url.clone().unwrap_or_default();
        let backend = self.backend.clone();

        let (tx, rx): (Sender<String>, Receiver<String>) = channel();

        backend
            .send(BKCommand::GetMediaAsync(url.clone(), tx))
            .unwrap();

        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => {
                let msg = i18n("Could not download the file");
                APPOP!(show_error, (msg));

                gtk::Continue(true)
            }
            Ok(fname) => {
                if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::new_from_file(fname) {
                    let atom = gdk::Atom::intern("CLIPBOARD");
                    let clipboard = gtk::Clipboard::get(&atom);

                    clipboard.set_image(&pixbuf);
                }

                gtk::Continue(false)
            }
        });
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
        let error = i18n("This message has no source.");
        let parent : gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        let source = self.msg
            .msg
            .source
            .as_ref()
            .unwrap_or(&error);

        let viewer = SourceViewer::new();
        viewer.set_parent_window(&parent);
        viewer.show(source.as_str());
    }

    pub fn connect_message_menu(&self) {
        let reply_button: gtk::ModelButton = self
            .builder
            .get_object("reply_button")
            .expect("Can't find reply_button in ui file.");

        let open_with_button: gtk::ModelButton = self
            .builder
            .get_object("open_with_button")
            .expect("Can't find open_with_button in ui file.");

        let save_image_as_button: gtk::ModelButton = self
            .builder
            .get_object("save_image_as_button")
            .expect("Can't find save_image_as_button in ui file.");

        let copy_image_button: gtk::ModelButton = self
            .builder
            .get_object("copy_image_button")
            .expect("Can't find copy_image_button in ui file.");

        let copy_text_button: gtk::ModelButton = self
            .builder
            .get_object("copy_text_button")
            .expect("Can't find copy_text_button in ui file.");

        let delete_message_button: gtk::ModelButton = self
            .builder
            .get_object("delete_message_button")
            .expect("Can't find delete_message_button in ui file.");

        let view_source_button: gtk::ModelButton = self
            .builder
            .get_object("view_source_button")
            .expect("Can't find view_source_button in ui file.");

        let copy_selected_button: gtk::ModelButton = self
            .builder
            .get_object("copy_selected_text_button")
            .expect("Can't find copy_selected_text_button in ui file.");

        /* since this is used only by the main thread we can just use a simple Rc<RefCell> */
        let this: Rc<RefCell<MessageMenu>> = Rc::new(RefCell::new(self.clone()));

        reply_button.connect_clicked(clone!(this => move |_| {
            this.borrow().insert_quote();
        }));

        open_with_button.connect_clicked(clone!(this => move |_| {
            this.borrow().open_with();
        }));

        save_image_as_button.connect_clicked(clone!(this => move |_| {
            this.borrow().save_image_as();
        }));

        copy_image_button.connect_clicked(clone!(this => move |_| {
            this.borrow().copy_image();
        }));

        copy_text_button.connect_clicked(clone!(this => move |_| {
            this.borrow().copy_text();
        }));

        copy_selected_button.connect_clicked(clone!(this => move |_| {
            this.borrow().copy_selected_text();
        }));

        let backend = self.backend.clone();
        delete_message_button.connect_clicked(clone!(this => move |_| {
            backend.send(BKCommand::SendMsgRedaction(this.borrow().msg.msg.clone())).unwrap();
        }));

        view_source_button.connect_clicked(clone!(this => move |_| {
            this.borrow().display_msg_src_window();
        }));
    }
}

fn get_selected_text(event_widget: Option<&gtk::Label>) -> Option<SelectedText> {
    let w = event_widget?;
    match w.get_selection_bounds() {
        Some((s, e)) => {
            let text = w.get_text()?;
            let slice: String = text.chars().take(e as usize).skip(s as usize).collect();
            Some(SelectedText {
                widget: w.clone(),
                text: slice,
                start: s,
                end: e,
            })
        }
        _ => None,
    }
}
