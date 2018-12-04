use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use backend::BKCommand;
use gio::ActionMapExt;
use gio::SimpleAction;
use gio::SimpleActionExt;
use gio::SimpleActionGroup;
use gtk;
use gtk::prelude::*;
use i18n::i18n;
use types::Message;
use uibuilder::UI;
use App;

use widgets::SourceViewer;

/* This creates all actions the room history can perform */
pub fn new(backend: Sender<BKCommand>, ui: UI) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    let reply = SimpleAction::new("reply", glib::VariantTy::new("s").ok());
    let open_with = SimpleAction::new("open_with", glib::VariantTy::new("s").ok());
    let save_image_as = SimpleAction::new("save_image_as", glib::VariantTy::new("s").ok());
    let copy_image = SimpleAction::new("copy_image", glib::VariantTy::new("s").ok());
    let copy_text = SimpleAction::new("copy_text", glib::VariantTy::new("s").ok());
    let delete = SimpleAction::new("delete", glib::VariantTy::new("s").ok());
    let show_source = SimpleAction::new("show_source", glib::VariantTy::new("s").ok());

    actions.add_action(&reply);
    actions.add_action(&open_with);
    actions.add_action(&save_image_as);
    actions.add_action(&copy_image);
    actions.add_action(&copy_text);
    actions.add_action(&delete);
    actions.add_action(&show_source);

    let parent: gtk::Window = ui
        .builder
        .get_object("main_window")
        .expect("Can't find main_window in ui file.");
    let parent = parent.downgrade();
    show_source.connect_activate(move |_, data| {
        let parent = upgrade_weak!(parent);
        let viewer = SourceViewer::new();
        viewer.set_parent_window(&parent);
        if let Some(m) = get_message(data) {
            let error = i18n("This message has no source.");
            let source = m.source.as_ref().unwrap_or(&error);

            viewer.show(source);
        }
    });

    let msg_entry = ui.sventry.view.downgrade();
    reply.connect_activate(move |_, data| {
        let msg_entry = upgrade_weak!(msg_entry);
        if let Some(buffer) = msg_entry.get_buffer() {
            let mut start = buffer.get_start_iter();
            if let Some(m) = get_message(data) {
                let quote = m
                    .body
                    .lines()
                    .map(|l| "> ".to_owned() + l)
                    .collect::<Vec<String>>()
                    .join("\n")
                    + "\n"
                    + "\n";

                buffer.insert(&mut start, &quote);
            }
            msg_entry.grab_focus();
        }
    });

    let b = backend.clone();
    open_with.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let url = m.url.unwrap_or_default();
            let _ = b.send(BKCommand::GetMedia(url));
        }
    });

    let b = backend.clone();
    save_image_as.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let name = m.body;
            let url = m.url.unwrap_or_default();

            let (tx, rx): (Sender<String>, Receiver<String>) = channel();
            let _ = b.send(BKCommand::GetMediaAsync(url, tx));

            gtk::timeout_add(
                50,
                clone!(name => move || match rx.try_recv() {
                    Err(TryRecvError::Empty) => gtk::Continue(true),
                    Err(TryRecvError::Disconnected) => {
                        let msg = i18n("Could not download the file");
                        /* FIXME: this should be an action */
                        APPOP!(show_error, (msg));

                        gtk::Continue(true)
                    },
                    Ok(fname) => {
                        let name = name.clone();
                        /* FIXME: this should be an action */
                        APPOP!(save_file_as, (fname, name));

                        gtk::Continue(false)
                    }
                }),
            );
        }
    });

    let b = backend.clone();
    copy_image.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let url = m.url.unwrap_or_default();

            let (tx, rx): (Sender<String>, Receiver<String>) = channel();

            let _ = b.send(BKCommand::GetMediaAsync(url.clone(), tx));

            gtk::timeout_add(50, move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not download the file");
                    /*FIXME: this should be an action */
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
    });

    copy_text.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let atom = gdk::Atom::intern("CLIPBOARD");
            let clipboard = gtk::Clipboard::get(&atom);

            clipboard.set_text(&m.body);
        }
    });

    actions
}

fn get_message(data: &Option<glib::Variant>) -> Option<Message> {
    get_message_by_id(data.as_ref()?.get_str()?)
}

/* TODO: get message from stroage once implemented */
fn get_message_by_id(id: &str) -> Option<Message> {
    let op = App::get_op()?;
    let op = op.lock().unwrap();
    let room_id = op.active_room.as_ref()?;
    op.get_message_by_id(room_id, id)
}
