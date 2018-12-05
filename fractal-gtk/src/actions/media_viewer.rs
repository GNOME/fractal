use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
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

use widgets::media_viewer::Data;
use widgets::ErrorDialog;
use widgets::SourceDialog;

/* This creates all actions the media viewer can perform
 * the actions are actually the same as the room history but we need to change the data source
 * because the media_list isn't stored outside the viewer */
pub fn new(
    backend: Sender<BKCommand>,
    parent: &gtk::Window,
    data: &Rc<RefCell<Data>>,
) -> gio::SimpleActionGroup {
    let actions = SimpleActionGroup::new();
    let open_with = SimpleAction::new("open_with", glib::VariantTy::new("s").ok());
    let save_as = SimpleAction::new("save_as", glib::VariantTy::new("s").ok());
    let copy_image = SimpleAction::new("copy_image", glib::VariantTy::new("s").ok());
    let show_source = SimpleAction::new("show_source", glib::VariantTy::new("s").ok());

    actions.add_action(&open_with);
    actions.add_action(&save_as);
    actions.add_action(&copy_image);
    actions.add_action(&show_source);

    let parent_weak = parent.downgrade();
    let store = data.clone();
    show_source.connect_activate(move |_, data| {
        let parent = upgrade_weak!(parent_weak);
        let viewer = SourceDialog::new();
        viewer.set_parent_window(&parent);
        if let Some(m) = get_message(&store, data) {
            let error = i18n("This message has no source.");
            let source = m.source.as_ref().unwrap_or(&error);

            viewer.show(source);
        }
    });

    let b = backend.clone();
    let store = data.clone();
    open_with.connect_activate(move |_, data| {
        if let Some(m) = get_message(&store, data) {
            let url = m.url.unwrap_or_default();
            let _ = b.send(BKCommand::GetMedia(url));
        }
    });

    let b = backend.clone();
    let parent_weak = parent.downgrade();
    let store = data.clone();
    save_as.connect_activate(move |_, data| {
        if let Some(m) = get_message(&store, data) {
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
        } else {
            println!("No message found");
        }
    });

    let b = backend.clone();
    let parent_weak = parent.downgrade();
    let store = data.clone();
    copy_image.connect_activate(move |_, data| {
        if let Some(m) = get_message(&store, data) {
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
    actions
}

/* FIXME: replace this once we have real storage */
fn get_message(store: &Rc<RefCell<Data>>, data: &Option<glib::Variant>) -> Option<Message> {
    let id = data.as_ref()?.get_str()?;
    store.borrow().get_message(id)
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
