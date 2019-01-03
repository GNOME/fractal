use gdk;

use std::cell::RefCell;
use std::rc::Rc;

use dirs;
use gdk::*;
use glib;
use glib::signal;
use gtk;
use gtk::prelude::*;
use gtk::ResponseType;
use i18n::i18n;

use types::Message;
use types::Room;

use std::fs;

use backend::BKCommand;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use widgets::image;
use widgets::ErrorDialog;

const FLOATING_POINT_ERROR: f64 = 0.01;
const ZOOM_LEVELS: [f64; 7] = [0.025, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0];

#[derive(Debug, Clone)]
pub struct MediaViewer {
    data: Rc<RefCell<Data>>,
    /* gtk widgets we need to have a reference to */
    builder: gtk::Builder,
    backend: Sender<BKCommand>,
}

#[derive(Debug)]
struct Data {
    builder: gtk::Builder,
    main_window: gtk::Window,
    backend: Sender<BKCommand>,

    image: Option<image::Image>,
    media_list: Vec<Message>,
    current_media_index: usize,

    signal_id: Option<signal::SignalHandlerId>,
    prev_batch: Option<String>,
    loading_more_media: bool,
    loading_error: bool,
    no_more_media: bool,
}

impl Data {
    pub fn new(
        backend: Sender<BKCommand>,
        media_list: Vec<Message>,
        current_media_index: usize,
        main_window: gtk::Window,
        builder: gtk::Builder,
    ) -> Data {
        Data {
            media_list: media_list,
            current_media_index: current_media_index,
            prev_batch: None,
            loading_more_media: false,
            loading_error: false,
            no_more_media: false,
            image: None,
            builder,
            backend,
            main_window,
            signal_id: None,
        }
    }

    pub fn save_media(&self) -> Option<()> {
        let image = self.image.clone()?;
        save_file_as(
            &self.main_window,
            image.local_path.lock().unwrap().clone().unwrap_or_default(),
            self.media_list[self.current_media_index].body.clone(),
        );
        None
    }

    pub fn enter_full_screen(&mut self) {
        self.main_window.fullscreen();

        let media_viewer_headerbar_box = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar_box in ui file.");
        let media_viewer_headerbar = self
            .builder
            .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
            .expect("Can't find media_viewer_headerbar in ui file.");
        let headerbar_revealer = self
            .builder
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");
        // gdk::EventMask::ENTER_NOTIFY_MASK = 4096
        headerbar_revealer.add_events(4096);
        // gdk::EventMask::LEAVE_NOTIFY_MASK = 8192
        headerbar_revealer.add_events(8192);

        media_viewer_headerbar_box.remove(&media_viewer_headerbar);

        let media_viewer_back_button = self
            .builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Can't find media_viewer_back_button in ui file.");

        media_viewer_headerbar.remove(&media_viewer_back_button);
        media_viewer_headerbar.set_show_close_button(false);

        let full_screen_button_icon = self
            .builder
            .get_object::<gtk::Image>("full_screen_button_icon")
            .expect("Can't find full_screen_button_icon in ui file.");
        full_screen_button_icon.set_property_icon_name(Some("view-restore-symbolic"));

        headerbar_revealer.add(&media_viewer_headerbar);

        self.redraw_image_in_viewport();
    }

    pub fn leave_full_screen(&mut self) {
        self.main_window.unfullscreen();

        let media_viewer_headerbar_box = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar_box in ui file.");
        let media_viewer_headerbar = self
            .builder
            .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
            .expect("Can't find media_viewer_headerbar in ui file.");
        let headerbar_revealer = self
            .builder
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");
        let media_viewer_back_button = self
            .builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Can't find media_viewer_back_button in ui file.");

        if let Some(ch) = headerbar_revealer.get_child() {
            headerbar_revealer.remove(&ch);
        }

        media_viewer_headerbar.pack_start(&media_viewer_back_button);
        media_viewer_headerbar.set_child_position(&media_viewer_back_button, 0);
        media_viewer_headerbar.set_show_close_button(true);

        let full_screen_button_icon = self
            .builder
            .get_object::<gtk::Image>("full_screen_button_icon")
            .expect("Can't find full_screen_button_icon in ui file.");
        full_screen_button_icon.set_property_icon_name(Some("view-fullscreen-symbolic"));

        media_viewer_headerbar.set_hexpand(true);
        media_viewer_headerbar_box.add(&media_viewer_headerbar);

        self.redraw_image_in_viewport();
    }

    pub fn change_zoom_level(&self) {
        let zoom_entry = self
            .builder
            .get_object::<gtk::EntryBuffer>("zoom_level")
            .expect("Cant find zoom_level in ui file.");

        if let Some(ref image) = self.image {
            match zoom_entry
                .get_text()
                .trim()
                .trim_right_matches('%')
                .parse::<f64>()
            {
                Ok(zlvl) => self.set_zoom_level(zlvl / 100.0),
                Err(_) => {
                    if let Some(zlvl) = *image.zoom_level.lock().unwrap() {
                        update_zoom_entry(&self.builder, zlvl)
                    }
                }
            }
        }
    }

    pub fn set_zoom_level(&self, zlvl: f64) {
        if let Some(ref image) = self.image {
            *image.zoom_level.lock().unwrap() = Some(zlvl);
            image.widget.queue_draw();
        }
    }

    pub fn set_nav_btn_visibility(&self) {
        let previous_media_button = self
            .builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");

        let next_media_button = self
            .builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");

        if self.current_media_index == 0 && self.no_more_media {
            previous_media_button.set_visible(false);
        } else {
            previous_media_button.set_visible(true);
        }

        if self.media_list.len() > 0 && self.current_media_index >= self.media_list.len() - 1 {
            next_media_button.set_visible(false);
        } else {
            next_media_button.set_visible(true);
        }
    }

    pub fn zoom_out(&self) {
        if let Some(ref image) = self.image {
            let zoom_level = *image.zoom_level.lock().unwrap();
            if zoom_level.is_none() || zoom_level.unwrap() <= ZOOM_LEVELS[0] {
                return;
            }
            if let Some(new_zlvl) = ZOOM_LEVELS
                .iter()
                .filter(|zlvl| **zlvl < zoom_level.unwrap())
                .last()
            {
                self.set_zoom_level(*new_zlvl);
            }
        }
    }

    pub fn zoom_in(&self) {
        if let Some(ref image) = self.image {
            let zoom_level = *image.zoom_level.lock().unwrap();
            if zoom_level.is_none() || zoom_level.unwrap() >= ZOOM_LEVELS[ZOOM_LEVELS.len() - 1] {
                return;
            }

            if let Some(new_zlvl) = ZOOM_LEVELS
                .iter()
                .filter(|zlvl| **zlvl > zoom_level.unwrap())
                .nth(0)
            {
                self.set_zoom_level(*new_zlvl);
            }
        }
    }

    pub fn previous_media(&mut self) -> bool {
        if self.no_more_media {
            return true;
        }

        if self.current_media_index == 0 {
            return false;
        } else {
            {
                self.current_media_index -= 1;
                let name = &self.media_list[self.current_media_index].body;
                set_header_title(&self.builder, name);
            }

            self.redraw_image_in_viewport();
            return true;
        }
    }

    pub fn next_media(&mut self) {
        if self.current_media_index >= self.media_list.len() - 1 {
            return;
        }
        {
            self.current_media_index += 1;
            let name = &self.media_list[self.current_media_index].body;
            set_header_title(&self.builder, name);
        }

        self.redraw_image_in_viewport();
    }

    pub fn redraw_image_in_viewport(&mut self) {
        let media_viewport = self
            .builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        if let Some(child) = media_viewport.get_child() {
            media_viewport.remove(&child);
        }

        let url = self.media_list[self.current_media_index]
            .url
            .clone()
            .unwrap_or_default();

        let image = image::Image::new(&self.backend, &url)
            .fit_to_width(true)
            .center(true)
            .build();

        media_viewport.add(&image.widget);
        image.widget.show();

        let ui = self.builder.clone();
        let zoom_level = image.zoom_level.clone();
        image.widget.connect_draw(move |_, _| {
            if let Some(zlvl) = *zoom_level.lock().unwrap() {
                update_zoom_entry(&ui, zlvl);
            }

            Inhibit(false)
        });

        self.set_nav_btn_visibility();

        self.image = Some(image);
    }
}

impl MediaViewer {
    pub fn new(
        backend: Sender<BKCommand>,
        main_window: gtk::Window,
        room: &Room,
        current_media_msg: &Message,
    ) -> MediaViewer {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/media_viewer_menu.ui")
            .expect("Can't load ui file: media_viewer_menu.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/media_viewer.ui")
            .expect("Can't load ui file: media_viewer.ui");

        let media_list: Vec<Message> = room
            .messages
            .clone()
            .into_iter()
            .filter(|msg| msg.mtype == "m.image")
            .collect();

        let current_media_index = media_list
            .iter()
            .position(|media| media.id == current_media_msg.id)
            .unwrap_or_default();

        MediaViewer {
            data: Rc::new(RefCell::new(Data::new(
                backend.clone(),
                media_list,
                current_media_index,
                main_window,
                builder.clone(),
            ))),
            builder,
            backend,
        }
    }

    pub fn create(&mut self) -> Option<(gtk::Box, gtk::Box)> {
        let body = self
            .builder
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Can't find media_viewer_box in ui file.");
        let header = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar in ui file.");
        self.connect_media_viewer_headerbar();
        self.connect_media_viewer_box();

        Some((body, header))
    }

    pub fn display_media_viewer(&mut self, media_msg: Message) {
        let previous_media_revealer = self
            .builder
            .get_object::<gtk::Revealer>("previous_media_revealer")
            .expect("Cant find previous_media_revealer in ui file.");
        previous_media_revealer.set_reveal_child(false);

        let next_media_revealer = self
            .builder
            .get_object::<gtk::Revealer>("next_media_revealer")
            .expect("Cant find next_media_revealer in ui file.");
        next_media_revealer.set_reveal_child(false);

        set_header_title(&self.builder, &media_msg.body);

        let media_viewport = self
            .builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        let image = image::Image::new(&self.backend, &media_msg.url.clone().unwrap_or_default())
            .fit_to_width(true)
            .fixed(true)
            .center(true)
            .build();

        media_viewport.add(&image.widget);
        media_viewport.show_all();

        let ui = self.builder.clone();
        let zoom_level = image.zoom_level.clone();
        image.widget.connect_draw(move |_, _| {
            if let Some(zlvl) = *zoom_level.lock().unwrap() {
                update_zoom_entry(&ui, zlvl);
            }

            Inhibit(false)
        });

        self.data.borrow_mut().image = Some(image);
        self.data.borrow_mut().set_nav_btn_visibility();
    }

    /* connect media viewer headerbar */
    pub fn connect_media_viewer_headerbar(&self) {
        let zoom_entry = self
            .builder
            .get_object::<gtk::Entry>("zoom_entry")
            .expect("Cant find zoom_entry in ui file.");
        let own = self.data.clone();
        zoom_entry.connect_activate(move |_| {
            own.borrow().change_zoom_level();
        });

        let own = self.data.clone();
        let zoom_out_button = self
            .builder
            .get_object::<gtk::Button>("zoom_out_button")
            .expect("Cant find zoom_out_button in ui file.");
        zoom_out_button.connect_clicked(move |_| {
            own.borrow().zoom_out();
        });

        let own = self.data.clone();
        let zoom_in_button = self
            .builder
            .get_object::<gtk::Button>("zoom_in_button")
            .expect("Cant find zoom_in_button in ui file.");
        zoom_in_button.connect_clicked(move |_| {
            own.borrow().zoom_in();
        });

        let own = self.data.clone();
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");
        full_screen_button.connect_clicked(move |_| {
            let main_window = own.borrow().main_window.clone();
            if let Some(win) = main_window.get_window() {
                if !win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                    own.borrow_mut().enter_full_screen();
                } else {
                    own.borrow_mut().leave_full_screen()
                }
            }
        });

        let save_as_button = self
            .builder
            .get_object::<gtk::ModelButton>("save_as_button")
            .expect("Cant find save_as_button in ui file.");
        let data = self.data.clone();
        save_as_button.connect_clicked(move |_| {
            data.borrow().save_media();
        });
    }

    pub fn connect_media_viewer_box(&self) {
        let ui = self.builder.clone();
        let header_hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let nav_hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let headerbar_revealer = ui
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");

        headerbar_revealer.connect_enter_notify_event(clone!(header_hovered => move |_, _| {
            *(header_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));

        headerbar_revealer.connect_leave_notify_event(clone!(header_hovered => move |_, _| {
            *(header_hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let previous_media_button = ui
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");

        previous_media_button.connect_enter_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        previous_media_button.connect_leave_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let next_media_button = ui
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");

        next_media_button.connect_enter_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        next_media_button.connect_leave_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let media_viewer_box = ui
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");

        let source_id: Arc<Mutex<Option<glib::source::SourceId>>> = Arc::new(Mutex::new(None));
        let win = self.data.borrow().main_window.clone();
        media_viewer_box.connect_motion_notify_event(move |_, _| {
            {
                let mut id = source_id.lock().unwrap();
                if let Some(sid) = id.take() {
                    glib::source::source_remove(sid);
                }
            }

            gdk::Display::get_default()
                .and_then(|disp| disp.get_default_seat())
                .and_then(|seat| seat.get_pointer())
                .map(|ptr| {
                    let win = win.get_window()?;
                    let (_, _, y, _) = win.get_device_position(&ptr);
                    if y <= 6 && win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                        headerbar_revealer.set_reveal_child(true);
                    }
                    Some(true)
                });

            let previous_media_revealer = ui
                .get_object::<gtk::Revealer>("previous_media_revealer")
                .expect("Cant find previous_media_revealer in ui file.");
            previous_media_revealer.set_reveal_child(true);

            let next_media_revealer = ui
                .get_object::<gtk::Revealer>("next_media_revealer")
                .expect("Cant find next_media_revealer in ui file.");
            next_media_revealer.set_reveal_child(true);

            let sid = gtk::timeout_add(
                1000,
                clone!(ui, header_hovered, nav_hovered, source_id => move || {
                    if !*header_hovered.lock().unwrap() {
                        let headerbar_revealer = ui
                            .get_object::<gtk::Revealer>("headerbar_revealer")
                            .expect("Can't find headerbar_revealer in ui file.");
                        headerbar_revealer.set_reveal_child(false);
                    }

                    if !*nav_hovered.lock().unwrap() {
                        let previous_media_revealer = ui
                            .get_object::<gtk::Revealer>("previous_media_revealer")
                            .expect("Cant find previous_media_revealer in ui file.");
                        previous_media_revealer.set_reveal_child(false);

                        let next_media_revealer = ui
                            .get_object::<gtk::Revealer>("next_media_revealer")
                            .expect("Cant find next_media_revealer in ui file.");
                        next_media_revealer.set_reveal_child(false);
                    }

                    *(source_id.lock().unwrap()) = None;
                    gtk::Continue(false)
                }),
            );

            *(source_id.lock().unwrap()) = Some(sid);
            Inhibit(false)
        });

        let own = self.data.clone();
        let builder = self.builder.clone();
        let backend = self.backend.clone();
        let previous_media_button = self
            .builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");
        previous_media_button.connect_clicked(move |_| {
            if !own.borrow_mut().previous_media() {
                load_more_media(own.clone(), builder.clone(), backend.clone());
            }
        });

        let own = self.data.clone();
        let next_media_button = self
            .builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");
        next_media_button.connect_clicked(move |_| {
            own.borrow_mut().next_media();
        });
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");

        let id = self
            .data
            .borrow()
            .main_window
            .connect_key_press_event(move |w, k| {
                match k.get_keyval() {
                    gdk::enums::key::Escape => {
                        // leave full screen only if we're currently in fullscreen
                        if let Some(win) = w.get_window() {
                            if win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                                full_screen_button.clicked();
                                return Inhibit(true);
                            }
                        }

                        Inhibit(false)
                    }
                    gdk::enums::key::Left => {
                        previous_media_button.clicked();
                        Inhibit(true)
                    }
                    gdk::enums::key::Right => {
                        next_media_button.clicked();
                        Inhibit(true)
                    }
                    _ => Inhibit(false),
                }
            });
        self.data.borrow_mut().signal_id = Some(id);

        // Remove the keyboard signal management on hide
        let data = self.data.clone();
        media_viewer_box.connect_unmap(move |_| {
            let id = data.borrow_mut().signal_id.take();
            let main_window = &data.borrow().main_window;
            if let Some(id) = id {
                signal::signal_handler_disconnect(main_window, id);
            }
        });
    }
}

fn update_zoom_entry(ui: &gtk::Builder, zoom_level: f64) {
    let zoom_entry = ui
        .get_object::<gtk::EntryBuffer>("zoom_level")
        .expect("Cant find zoom_level in ui file.");
    zoom_entry.set_text(&format!("{:.0}%", zoom_level * 100.0));
    set_zoom_btn_sensitivity(ui, zoom_level);
}

fn set_zoom_btn_sensitivity(builder: &gtk::Builder, zlvl: f64) -> Option<()> {
    let zoom_out_button = builder
        .get_object::<gtk::Button>("zoom_out_button")
        .expect("Cant find zoom_out_button in ui file.");

    let zoom_in_button = builder
        .get_object::<gtk::Button>("zoom_in_button")
        .expect("Cant find zoom_in_button in ui file.");

    let min_lvl = ZOOM_LEVELS.first()?.clone();
    let max_lvl = ZOOM_LEVELS.last()?.clone();
    zoom_out_button.set_sensitive(!(zlvl <= min_lvl + FLOATING_POINT_ERROR));
    zoom_in_button.set_sensitive(!(zlvl >= max_lvl - FLOATING_POINT_ERROR));
    None
}

fn set_header_title(ui: &gtk::Builder, title: &str) {
    let media_viewer_headerbar = ui
        .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
        .expect("Cant find media_viewer_headerbar in ui file.");
    media_viewer_headerbar.set_title(title);
}

fn loading_state(ui: &gtk::Builder, val: bool) -> bool {
    let notification: gtk::Revealer = ui
        .get_object("media_viewer_notify_revealer")
        .expect("Can't find media_viewer_notify_revealer in ui file.");
    notification.set_reveal_child(val);

    let previous_media_button = ui
        .get_object::<gtk::Button>("previous_media_button")
        .expect("Cant find previous_media_button in ui file.");
    previous_media_button.set_sensitive(!val);

    let next_media_button = ui
        .get_object::<gtk::Button>("next_media_button")
        .expect("Cant find next_media_button in ui file.");
    next_media_button.set_sensitive(!val);

    val
}

fn load_more_media(data: Rc<RefCell<Data>>, builder: gtk::Builder, backend: Sender<BKCommand>) {
    data.borrow_mut().loading_more_media = loading_state(&builder, true);

    let msg = data.borrow().media_list[data.borrow().current_media_index].clone();
    let roomid = msg.room.clone();
    let first_media_id = Some(msg.id.clone());
    let prev_batch = data.borrow().prev_batch.clone();

    let (tx, rx): (
        Sender<(Vec<Message>, String)>,
        Receiver<(Vec<Message>, String)>,
    ) = channel();
    backend
        .send(BKCommand::GetMediaListAsync(
            roomid,
            first_media_id,
            prev_batch,
            tx,
        ))
        .unwrap();

    let ui = builder.clone();
    let data = data.clone();
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => {
            data.borrow_mut().loading_error = true;
            let err = i18n("Error while loading previous media");
            ErrorDialog::new(false, &err);

            gtk::Continue(false)
        }
        Ok((msgs, prev_batch)) => {
            if msgs.len() == 0 {
                data.borrow_mut().no_more_media = true;
                return gtk::Continue(false);
            }

            let media_list = data.borrow().media_list.clone();
            let img_msgs: Vec<Message> = msgs
                .into_iter()
                .filter(|msg| msg.mtype == "m.image")
                .collect();
            let img_msgs_count = img_msgs.len();
            let new_media_list: Vec<Message> =
                img_msgs.into_iter().chain(media_list.into_iter()).collect();

            data.borrow_mut().media_list = new_media_list;
            data.borrow_mut().prev_batch = Some(prev_batch);

            if img_msgs_count == 0 {
                load_more_media(data.clone(), builder.clone(), backend.clone());
            } else {
                data.borrow_mut().current_media_index += img_msgs_count;
                data.borrow_mut().previous_media();
                data.borrow_mut().loading_more_media = loading_state(&ui, false);
            }

            gtk::Continue(false)
        }
    });
}

/* FIXME: The following two functions should be moved to a different file,
 * so that they can be used again in different locations */
fn save_file_as(main_window: &gtk::Window, src: String, name: String) {
    let file_chooser = gtk::FileChooserNative::new(
        Some(i18n("Save media as").as_str()),
        Some(main_window),
        gtk::FileChooserAction::Save,
        Some(i18n("_Save").as_str()),
        Some(i18n("_Cancel").as_str()),
    );

    file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());
    file_chooser.set_current_name(&name);

    file_chooser.connect_response(move |fcd, res| {
        if ResponseType::from(res) == ResponseType::Accept {
            if let Err(_) = fs::copy(src.clone(), fcd.get_filename().unwrap_or_default()) {
                let err = i18n("Could not save the file");
                ErrorDialog::new(false, &err);
            }
        }
    });

    file_chooser.run();
}
