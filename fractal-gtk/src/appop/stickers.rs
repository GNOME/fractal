extern crate glib;
extern crate gtk;
extern crate gdk_pixbuf;
extern crate chrono;

use self::chrono::prelude::*;

use self::gtk::ContainerExt;
use self::gtk::StackExt;
use self::gtk::WidgetExt;
use self::gtk::LabelExt;
use self::gtk::BoxExt;
use self::gtk::ButtonExt;

use self::gdk_pixbuf::Pixbuf;
use self::gtk::ImageExt;
use self::gdk_pixbuf::PixbufExt;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;

use std::cmp::Ordering;

use app::InternalCommand;
use appop::AppOp;

use backend::BKCommand;
use types::StickerGroup;
use types::Sticker;
use types::Message;
use widgets;


impl AppOp {
    pub fn stickers_loaded(&mut self, stickers: Vec<StickerGroup>) {
        self.stickers = stickers;
        self.stickers.sort_by(|x, y| {
            if x.purchased == y.purchased {
                return x.name.cmp(&y.name);
            }

            match x.purchased {
                true => Ordering::Less,
                false => Ordering::Greater,
            }
        });
        self.stickers_loading(false);
    }

    #[allow(dead_code)]
    pub fn stickers_draw(&self) {
        let stickers_box = self.ui.builder
            .get_object::<gtk::Box>("stickers_box")
            .expect("Can't find room_name in ui file.");

        for ch in stickers_box.get_children().iter() {
            stickers_box.remove(ch);
        }

        for sticker in self.stickers.iter() {
            let builder = gtk::Builder::new_from_resource("/org/gnome/Fractal/ui/sticker_group.ui");

            let bx = builder.get_object::<gtk::Box>("widget").expect("Can't find widget in ui file.");
            let container = builder.get_object::<gtk::Container>("container").expect("Can't find container in ui file.");

            builder.get_object::<gtk::Label>("name")
                   .expect("Can't find name in ui file.")
                   .set_text(&sticker.name[..]);
            builder.get_object::<gtk::Label>("desc")
                   .expect("Can't find desc in ui file.")
                   .set_text(&sticker.description[..]);

            if sticker.purchased {
                self.stickers_draw_imgs(&builder, sticker);
            } else {
                let img = builder.get_object::<gtk::Image>("thumb")
                       .expect("Can't find thumb in ui file.");
                self.sticker_thumbnail(sticker.thumbnail.clone(), &img);
                let btn = builder.get_object::<gtk::Button>("btn")
                   .expect("Can't find btn in ui file.");
                let group = sticker.clone();
                let internal = self.internal.clone();
                btn.connect_clicked(move |_| {
                    let command = InternalCommand::PurchaseSticker(group.clone());
                    internal.send(command).unwrap();
                });
            }

            container.remove(&bx);
            stickers_box.add(&bx);
        }
        stickers_box.show_all();
    }

    pub fn stickers_loading(&self, loading: bool) {
        let stack = self.ui.builder
            .get_object::<gtk::Stack>("stickers_stack")
            .expect("Can't find stickers_stack in ui file.");

        match loading {
            true => stack.set_visible_child_name("loading"),
            false => stack.set_visible_child_name("view"),
        };
    }

    pub fn stickers_load(&self) {
        self.stickers_loading(true);
        self.backend.send(BKCommand::ListStickers).unwrap();
    }

    #[allow(dead_code)]
    fn sticker_thumbnail(&self, url: String, img: &gtk::Image) {
        // asyn load
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        self.backend.send(BKCommand::GetFileAsync(url, tx)).unwrap();
        let im = img.clone();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => gtk::Continue(false),
            Ok(fname) => {
                let mut f = fname.clone();
                if let Ok(pix) = Pixbuf::new_from_file_at_scale(&f, 38, 38, true) {
                    let (w, h) = (pix.get_width(), pix.get_height());
                    if w != 38 {
                        if let Ok(pix) = Pixbuf::new_from_file_at_scale(&f, 38, h * 38 / w, true) {
                            im.set_from_pixbuf(&pix);
                        }
                    } else {
                        im.set_from_pixbuf(&pix);
                    }
                } else {
                    im.set_from_file(f);
                }
                gtk::Continue(false)
            }
        });
    }

    #[allow(dead_code)]
    fn stickers_draw_imgs(&self, builder: &gtk::Builder, sticker: &StickerGroup) {
        let size = 50;
        let content = builder
            .get_object::<gtk::Box>("content")
            .expect("Can't find content in ui file.");

        for ch in content.get_children().iter() {
            content.remove(ch);
        }

        let mut bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bx.set_homogeneous(true);
        content.pack_start(&bx, true, true, 6);

        for (i, img) in sticker.stickers.iter().enumerate() {
            if i > 0 && i % 5 == 0 {
                bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                bx.set_homogeneous(true);
                content.pack_start(&bx, true, true, 6);
            }

            let backend = self.backend.clone();
            let image = widgets::image::Image::new(&backend, &img.thumbnail.clone(), Some((size, size)),
                                                   widgets::image::Thumb(true), widgets::image::Circle(false),
                                                   widgets::image::Fixed(true));
            let eb = gtk::EventBox::new();
            eb.add(&image.widget);
            bx.add(&eb);

            let internal = self.internal.clone();
            let im = img.clone();
            let popover: gtk::Popover = self.ui.builder
                .get_object("stickers_popover")
                .expect("Couldn't find stickers_popover in ui file.");
            eb.connect_button_press_event(move |_, _| {
                popover.hide();
                let command = InternalCommand::SendSticker(im.clone());
                internal.send(command).unwrap();
                glib::signal::Inhibit(false)
            });
        }

        content.show_all();
    }

    pub fn send_sticker(&mut self, sticker: Sticker) {
        let roomid = self.active_room.clone().unwrap_or_default();
        self.backend.send(BKCommand::SendSticker(roomid.clone(), sticker.clone())).unwrap();

        let msg = Message {
            sender: self.uid.clone().unwrap_or_default(),
            mtype: "m.sticker".to_string(),
            date: Local::now(),
            room: roomid.clone(),
            id: None,
            body: sticker.body.clone(),
            url: Some(sticker.url.clone()),
            thumb: Some(sticker.thumbnail.clone()),
            formatted_body: None,
            format: None,
        };

        self.add_tmp_room_message(msg);
    }

    pub fn purchase_sticker(&self, group: StickerGroup) {
        self.backend.send(BKCommand::PurchaseSticker(group)).unwrap();
        self.stickers_loading(true);
    }
}
