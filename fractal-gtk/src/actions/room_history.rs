use std::fs;
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
use gtk::ResponseType;
use i18n::i18n;
use types::Message;
use uibuilder::UI;
use App;

use widgets::ErrorDialog;
use widgets::SourceDialog;

/* This creates all actions the room history can perform */
pub fn new(backend: Sender<BKCommand>, ui: UI) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    /* Action for each message */
    let reply = SimpleAction::new("reply", glib::VariantTy::new("s").ok());
    let open_with = SimpleAction::new("open_with", glib::VariantTy::new("s").ok());
    let save_as = SimpleAction::new("save_as", glib::VariantTy::new("s").ok());
    let copy_image = SimpleAction::new("copy_image", glib::VariantTy::new("s").ok());
    let copy_text = SimpleAction::new("copy_text", glib::VariantTy::new("s").ok());
    let delete = SimpleAction::new("delete", glib::VariantTy::new("s").ok());
    let show_source = SimpleAction::new("show_source", glib::VariantTy::new("s").ok());
    let open_media_viewer = SimpleAction::new("open-media-viewer", glib::VariantTy::new("s").ok());
    /* Actions for the room history */

    /* TODO: use statefull action to keep  track if the user already reqeusted new messages */
    let load_more_messages =
        SimpleAction::new("request_older_messages", glib::VariantTy::new("s").ok());

    actions.add_action(&reply);
    actions.add_action(&open_with);
    actions.add_action(&save_as);
    actions.add_action(&copy_image);
    actions.add_action(&copy_text);
    actions.add_action(&delete);
    actions.add_action(&show_source);
    actions.add_action(&open_media_viewer);
    actions.add_action(&load_more_messages);

    let parent: gtk::Window = ui
        .builder
        .get_object("main_window")
        .expect("Can't find main_window in ui file.");
    let parent_weak = parent.downgrade();
    show_source.connect_activate(move |_, data| {
        let parent = upgrade_weak!(parent_weak);
        let viewer = SourceDialog::new();
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
                msg_entry.grab_focus();
            }
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
    let parent_weak = parent.downgrade();
    save_as.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let name = m.body;
            let url = m.url.unwrap_or_default();

            let (tx, rx): (Sender<String>, Receiver<String>) = channel();
            let _ = b.send(BKCommand::GetMediaAsync(url, tx));

            let parent_weak = parent_weak.clone();
            gtk::timeout_add(
                50,
                clone!(name => move || match rx.try_recv() {
                    Err(TryRecvError::Empty) => gtk::Continue(true),
                    Err(TryRecvError::Disconnected) => {
                        let msg = i18n("Could not download the file");
                        let parent = upgrade_weak!(parent_weak, gtk::Continue(true));
                        ErrorDialog::new(&parent, &msg);

                        gtk::Continue(true)
                    },
                    Ok(fname) => {
                        let parent = upgrade_weak!(parent_weak, gtk::Continue(true));
                        open_save_as_dialog(&parent, fname, &name);

                        gtk::Continue(false)
                    }
                }),
            );
        }
    });

    let b = backend.clone();
    let parent_weak = parent.downgrade();
    copy_image.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let url = m.url.unwrap_or_default();

            let (tx, rx): (Sender<String>, Receiver<String>) = channel();

            let _ = b.send(BKCommand::GetMediaAsync(url.clone(), tx));

            let parent_weak = parent_weak.clone();
            gtk::timeout_add(50, move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not download the file");
                    let parent = upgrade_weak!(parent_weak, gtk::Continue(true));
                    ErrorDialog::new(&parent, &msg);

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

    open_media_viewer.connect_activate(move |_, data| {
        open_viewer(data);
    });

    load_more_messages.connect_activate(move |_, data| {
        let id = get_room_id(data);
        request_more_messages(&backend, id);
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

fn get_room_id(data: &Option<glib::Variant>) -> Option<String> {
    data.as_ref()?.get_str().map(|s| s.to_string())
}

fn open_viewer(data: &Option<glib::Variant>) -> Option<()> {
    let msg = get_message(data)?;
    let op = App::get_op()?;
    let mut op = op.lock().unwrap();
    op.create_media_viewer(msg);
    None
}

fn open_save_as_dialog(parent: &gtk::Window, src: String, name: &str) {
    let file_chooser = gtk::FileChooserNative::new(
        Some(i18n("Save media as").as_str()),
        Some(parent),
        gtk::FileChooserAction::Save,
        Some(i18n("_Save").as_str()),
        Some(i18n("_Cancel").as_str()),
    );

    file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());
    file_chooser.set_current_name(name);

    let parent_weak = parent.downgrade();
    file_chooser.connect_response(move |fcd, res| {
        if ResponseType::from(res) == ResponseType::Accept {
            if let Err(_) = fs::copy(src.clone(), fcd.get_filename().unwrap_or_default()) {
                let msg = i18n("Could not save the file");
                let parent = upgrade_weak!(parent_weak);
                ErrorDialog::new(&parent, &msg);
            }
        }
    });

    file_chooser.run();
}

fn request_more_messages(backend: &Sender<BKCommand>, id: Option<String>) -> Option<()> {
    let op = App::get_op()?;
    let op = op.lock().unwrap();
    let id = id?;
    let r = op.rooms.get(&id)?;
    if let Some(prev_batch) = r.prev_batch.clone() {
        backend
            .send(BKCommand::GetRoomMessages(id, prev_batch))
            .unwrap();
    } else if let Some(msg) = r.messages.iter().next() {
        // no prev_batch so we use the last message to calculate that in the backend
        backend
            .send(BKCommand::GetRoomMessagesFromMsg(id, msg.clone()))
            .unwrap();
    } else if let Some(from) = op.since.clone() {
        // no messages and no prev_batch so we use the last since
        backend.send(BKCommand::GetRoomMessages(id, from)).unwrap();
    }
    None
}
