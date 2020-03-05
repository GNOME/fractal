use fractal_api::clone;
use fractal_api::identifiers::RoomId;
use fractal_api::r0::AccessToken;
use log::error;
use std::convert::TryFrom;
use std::fs;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use crate::backend::BKCommand;
use crate::error::Error;
use crate::i18n::i18n;
use crate::types::Message;
use crate::uibuilder::UI;
use crate::App;
use fractal_api::url::Url;
use gio::ActionMapExt;
use gio::SimpleAction;
use gio::SimpleActionGroup;
use glib::source::Continue;
use gtk;
use gtk::prelude::*;

use crate::widgets::ErrorDialog;
use crate::widgets::FileDialog::save;
use crate::widgets::SourceDialog;

/* This creates all actions the room history can perform */
pub fn new(
    backend: Sender<BKCommand>,
    server_url: Url,
    access_token: AccessToken,
    ui: UI,
) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    /* Action for each message */
    let reply = SimpleAction::new("reply", glib::VariantTy::new("s").ok());
    let open_with = SimpleAction::new("open_with", glib::VariantTy::new("s").ok());
    let save_as = SimpleAction::new("save_as", glib::VariantTy::new("s").ok());
    let copy_image = SimpleAction::new("copy_image", glib::VariantTy::new("s").ok());
    let copy_text = SimpleAction::new("copy_text", glib::VariantTy::new("s").ok());
    let delete = SimpleAction::new("delete", glib::VariantTy::new("s").ok());
    let show_source = SimpleAction::new("show_source", glib::VariantTy::new("s").ok());

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
    open_with.connect_activate(clone!(server_url => move |_, data| {
        if let Some(m) = get_message(data) {
            let url = m.url.unwrap_or_default();
            let _ = b.send(BKCommand::GetMedia(server_url.clone(), url));
        }
    }));

    let b = backend.clone();
    let parent_weak = parent.downgrade();
    save_as.connect_activate(clone!(server_url => move |_, data| {
        if let Some(m) = get_message(data) {
            let name = m.body;
            let url = m.url.unwrap_or_default();

            let (tx, rx): (Sender<Result<String, Error>>, Receiver<Result<String, Error>>) = channel();
            let _ = b.send(BKCommand::GetMediaAsync(server_url.clone(), url, tx));

            let parent_weak = parent_weak.clone();
            gtk::timeout_add(
                50,
                clone!(name => move || match rx.try_recv() {
                    Err(TryRecvError::Empty) => Continue(true),
                    Err(TryRecvError::Disconnected) => {
                        let msg = i18n("Could not download the file");
                        ErrorDialog::new(false, &msg);

                        Continue(true)
                    },
                    Ok(Ok(fname)) => {
                        let window = upgrade_weak!(parent_weak, Continue(true));
                        if let Some(path) = save(&window, &name, &[]) {
                            // TODO use glib to copy file
                            if let Err(_) = fs::copy(fname.clone(), path) {
                                ErrorDialog::new(false, &i18n("Couldn’t save file"));
                            }
                        }
                        Continue(false)
                    }
                    Ok(Err(err)) => {
                        error!("Media path could not be found due to error: {:?}", err);
                        Continue(false)
                    }
                }),
            );
        }
    }));

    let b = backend.clone();
    copy_image.connect_activate(clone!(server_url => move |_, data| {
        if let Some(m) = get_message(data) {
            let url = m.url.unwrap_or_default();

            let (tx, rx): (
                Sender<Result<String, Error>>,
                Receiver<Result<String, Error>>,
            ) = channel();

            let _ = b.send(BKCommand::GetMediaAsync(server_url.clone(), url.clone(), tx));

            gtk::timeout_add(50, move || match rx.try_recv() {
                Err(TryRecvError::Empty) => Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not download the file");
                    ErrorDialog::new(false, &msg);

                    Continue(true)
                }
                Ok(Ok(fname)) => {
                    if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::new_from_file(fname) {
                        let atom = gdk::Atom::intern("CLIPBOARD");
                        let clipboard = gtk::Clipboard::get(&atom);

                        clipboard.set_image(&pixbuf);
                    }

                    Continue(false)
                }
                Ok(Err(err)) => {
                    error!("Image path could not be found due to error: {:?}", err);
                    Continue(false)
                }
            });
        }
    }));

    copy_text.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let atom = gdk::Atom::intern("CLIPBOARD");
            let clipboard = gtk::Clipboard::get(&atom);

            clipboard.set_text(&m.body);
        }
    });

    let b = backend.clone();
    let u = server_url.clone();
    let tk = access_token.clone();
    delete.connect_activate(move |_, data| {
        if let Some(m) = get_message(data) {
            let _ = b.send(BKCommand::SendMsgRedaction(u.clone(), tk.clone(), m));
        }
    });

    load_more_messages.connect_activate(clone!(server_url, access_token => move |_, data| {
        let id = get_room_id(data);
        request_more_messages(&backend, server_url.clone(), access_token.clone(), id);
    }));

    actions
}

fn get_message(data: Option<&glib::Variant>) -> Option<Message> {
    get_message_by_id(data?.get_str()?)
}

/* TODO: get message from stroage once implemented */
fn get_message_by_id(id: &str) -> Option<Message> {
    let op = App::get_op()?;
    let op = op.lock().unwrap();
    let room_id = op.active_room.as_ref()?;
    op.get_message_by_id(room_id, id)
}

fn get_room_id(data: Option<&glib::Variant>) -> Option<RoomId> {
    data.as_ref()?
        .get_str()
        .and_then(|rid| RoomId::try_from(rid).ok())
}

fn request_more_messages(
    backend: &Sender<BKCommand>,
    server_url: Url,
    access_token: AccessToken,
    id: Option<RoomId>,
) -> Option<()> {
    let op = App::get_op()?;
    let op = op.lock().unwrap();
    let id = id?;
    let r = op.rooms.get(&id)?;
    if let Some(prev_batch) = r.prev_batch.clone() {
        backend
            .send(BKCommand::GetRoomMessages(
                server_url,
                access_token,
                id,
                prev_batch,
            ))
            .unwrap();
    } else if let Some(msg) = r.messages.iter().next() {
        // no prev_batch so we use the last message to calculate that in the backend
        backend
            .send(BKCommand::GetRoomMessagesFromMsg(
                server_url,
                access_token,
                id,
                msg.clone(),
            ))
            .unwrap();
    } else if let Some(from) = op.since.clone() {
        // no messages and no prev_batch so we use the last since
        backend
            .send(BKCommand::GetRoomMessages(
                server_url,
                access_token,
                id,
                from,
            ))
            .unwrap();
    }
    None
}
