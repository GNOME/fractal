extern crate gtk;
extern crate gdk;
extern crate glib;
extern crate gdk_pixbuf;
extern crate cairo;
extern crate letter_avatar;

use std::cell::RefCell;
use std::rc::Rc;

use self::gtk::prelude::*;
pub use self::gtk::DrawingArea;
use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use self::gdk::ContextExt;
use fractal_api::util::cache_path;

pub type Avatar = gtk::Box;

pub trait AvatarExt {
    fn avatar_new(size: Option<i32>) -> gtk::Box;
    fn clean(&self);
    fn create_da(&self, size: Option<i32>) -> DrawingArea;
    fn circle(&self, uid: String, username: Option<String>, size: i32);
}

impl AvatarExt for gtk::Box {
    fn clean(&self) {
        for ch in self.get_children().iter() {
            self.remove(ch);
        }
    }

    fn create_da(&self, size: Option<i32>) -> DrawingArea {
        let da = DrawingArea::new();

        let s = size.unwrap_or(40);
        da.set_size_request(s, s);
        self.pack_start(&da, true, true, 0);
        self.show_all();

        da
    }

    fn avatar_new(size: Option<i32>) -> gtk::Box {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        b.create_da(size);
        b.show_all();
        if let Some(style) = b.get_style_context() {
            style.add_class("avatar");
        }

        b
    }

    fn circle(&self, uid: String, username: Option<String>, size: i32) {
        struct Data {
            cache : Result<Pixbuf, glib::Error>,
            fallback: cairo::ImageSurface
        }

        self.clean();
        let da = self.create_da(Some(size));
        let path = cache_path(&uid).unwrap_or(String::from(""));
        let user_avatar = Pixbuf::new_from_file_at_scale(&path, size, -1, true);
        /* remove IRC postfix from the username */
        let username =  if let Some(u) = username {
            Some(u.trim_right_matches(" (IRC)").to_owned())
        }else {
            None
        };
        /* This function should never fail */
        let fallback = letter_avatar::generate::new(uid, username, size as f64).unwrap();

        let user_cache: Rc<RefCell<Data>> = Rc::new(RefCell::new(Data {cache: user_avatar, fallback: fallback}));

        let user_cache = user_cache.clone();
        da.connect_draw(move |da, g| {
            use std::f64::consts::PI;
            let width = size as f64;
            let height = size as f64;

            g.set_antialias(cairo::Antialias::Best);

            {
                let data = user_cache.borrow();
                if let Ok(ref pb) = data.cache {
                    let context = da.get_style_context().unwrap();
                    gtk::render_background(&context, g, 0.0, 0.0, width, height);

                    g.arc(width / 2.0, height / 2.0, width.min(height) / 2.0, 0.0, 2.0 * PI);
                    g.clip();

                    let hpos: f64 = (width - (pb.get_height()) as f64) / 2.0;
                    g.set_source_pixbuf(&pb, 0.0, hpos);
                } else {
                    /* use fallback */
                    g.set_source_surface(&data.fallback, 0f64, 0f64);
                }
            }
            /* we should look into the cache only if the cache has changed, but this information is
             * not yet available, we could also create our own signal and not use the draw signal*/
            {
                let mut data = user_cache.borrow_mut();
                let new_avatar = Pixbuf::new_from_file_at_scale(&path, size, -1, true);
                data.cache = new_avatar; 
            }

            g.rectangle(0.0, 0.0, width, height);
            g.fill();

            Inhibit(false)
        });
    }
}

pub enum AdminColor {
    Gold,
    Silver,
}

pub fn admin_badge(kind: AdminColor, size: Option<i32>) -> gtk::DrawingArea {
    let s = size.unwrap_or(10);

    let da = DrawingArea::new();
    da.set_size_request(s, s);

    let color = match kind {
        AdminColor::Gold => (237.0, 212.0, 0.0),
        AdminColor::Silver => (186.0, 186.0, 186.0),
    };

    let border = match kind {
        AdminColor::Gold => (107.0, 114.0, 0.0),
        AdminColor::Silver => (137.0, 137.0, 137.0),
    };

    da.connect_draw(move |da, g| {
        use std::f64::consts::PI;
        g.set_antialias(cairo::Antialias::Best);

        let width = s as f64;
        let height = s as f64;

        let context = da.get_style_context().unwrap();
        gtk::render_background(&context, g, 0.0, 0.0, width, height);

        g.set_source_rgba(color.0 / 256.0, color.1 / 256.0, color.2 / 256.0, 1.);
        g.arc(width / 2.0, height / 2.0, width.min(height) / 2.5, 0.0, 2.0 * PI);
        g.fill();

        g.set_source_rgba(border.0 / 256.0, border.1 / 256.0, border.2 / 256.0, 0.5);
        g.arc(width / 2.0, height / 2.0, width.min(height) / 2.5, 0.0, 2.0 * PI);
        g.stroke();

        Inhibit(false)
    });

    da
}
