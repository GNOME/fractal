extern crate glib;
extern crate gdk;
extern crate gdk_pixbuf;
extern crate gtk;
extern crate gettextrs;

use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use failure::Error;
use failure::err_msg;

use self::gtk::prelude::*;
use self::gettextrs::gettext;

use appop::AppOp;
use app::InternalCommand;

use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use util::get_pixbuf_data;


impl AppOp {
    pub fn paste(&self) {
        if let Some(display) = gdk::Display::get_default() {
            if let Some(clipboard) = gtk::Clipboard::get_default(&display) {
                if clipboard.wait_is_image_available() {
                    if let Some(pixb) = clipboard.wait_for_image() {
                        self.draw_image_paste_dialog(&pixb);

                        // removing text from clipboard
                        clipboard.set_text("");
                        clipboard.set_image(&pixb);
                    }
                } else {
                    // TODO: manage code pasting
                }
            }
        }
    }

    fn draw_image_paste_dialog(&self, pixb: &Pixbuf) {
        let w = pixb.get_width();
        let h = pixb.get_height();
        let scaled;
        if w > 600 {
            scaled = pixb.scale_simple(600, h*600/w, gdk_pixbuf::InterpType::Bilinear);
        } else {
            scaled = Some(pixb.clone());
        }

        if let Some(pb) = scaled {
            let window: gtk::ApplicationWindow = self.ui.builder
                .get_object("main_window")
                .expect("Can't find main_window in ui file.");
            let img = gtk::Image::new();
            let dialog = gtk::Dialog::new_with_buttons(
                Some(gettext("Image from Clipboard").as_str()),
                Some(&window),
                gtk::DialogFlags::MODAL|
                gtk::DialogFlags::USE_HEADER_BAR|
                gtk::DialogFlags::DESTROY_WITH_PARENT,
                &[]);

            img.set_from_pixbuf(&pb);
            img.show();
            dialog.get_content_area().add(&img);
            dialog.present();

            if let Some(hbar) = dialog.get_header_bar() {
                let bar = hbar.downcast::<gtk::HeaderBar>().unwrap();
                let closebtn = gtk::Button::new_with_label(gettext("Cancel").as_str());
                let okbtn = gtk::Button::new_with_label(gettext("Send").as_str());
                okbtn.get_style_context().unwrap().add_class("suggested-action");

                bar.set_show_close_button(false);
                bar.pack_start(&closebtn);
                bar.pack_end(&okbtn);
                bar.show_all();

                closebtn.connect_clicked(clone!(dialog => move |_| {
                    dialog.destroy();
                }));
                let internal = self.internal.clone();
                okbtn.connect_clicked(clone!(pixb, dialog => move |_| {
                    if let Ok(file) = store_pixbuf(&pixb) {
                        internal.send(InternalCommand::AttachMessage(file)).unwrap();
                    }
                    dialog.destroy();
                }));

                okbtn.grab_focus();
            }
        }
    }
}

fn store_pixbuf(pixb: &Pixbuf) -> Result<String, Error> {
    let data = get_pixbuf_data(pixb)?;
    let mut path = glib::get_tmp_dir().unwrap_or(PathBuf::from("/tmp"));
    path.push("fractal-pasted-image");
    let file = path.into_os_string().into_string().map_err(|_| err_msg("bad string"))?;
    let mut f = File::create(file.clone())?;
    f.write_all(&data)?;
    f.sync_data()?;

    Ok(file)
}
