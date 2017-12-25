extern crate gtk;
extern crate gdk;
extern crate gdk_pixbuf;

use self::gtk::prelude::*;
pub use self::gtk::DrawingArea;
use self::gdk_pixbuf::Pixbuf;
use self::gdk::ContextExt;

use std::collections::HashMap;
use std::sync::Mutex;
use send_cell::SendCell;

pub type Avatar = gtk::Box;

pub trait AvatarExt {
    fn avatar_new(size: Option<i32>) -> gtk::Box;
    fn circle_avatar(path: String, size: Option<i32>) -> gtk::Box;
    fn clean(&self);
    fn create_da(&self, size: Option<i32>) -> DrawingArea;

    fn circle(&self, path: String, size: Option<i32>);
    fn default(&self, icon: String, size: Option<i32>);
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

    fn circle_avatar(path: String, size: Option<i32>) -> gtk::Box {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        b.create_da(size);
        b.circle(path, size);
        b.show_all();
        if let Some(style) = b.get_style_context() {
            style.add_class("avatar");
        }

        b
    }

    fn default(&self, icon: String, size: Option<i32>) {
        self.clean();
        let da = self.create_da(size);
        let s = size.unwrap_or(40);

        let pixbuf = match gtk::IconTheme::get_default() {
            None => None,
            Some(i1) => match i1.load_icon(&icon[..], s, gtk::IconLookupFlags::empty()) {
                Err(_) => None,
                Ok(i2) => i2,
            }
        };

        da.connect_draw(move |da, g| {
            use std::f64::consts::PI;

            let width = s as f64;
            let height = s as f64;

            let context = da.get_style_context().unwrap();

            gtk::render_background(&context, g, 0.0, 0.0, width, height);

            if let Some(ref pb) = pixbuf {
                let hpos: f64 = (width - (pb.get_height()) as f64) / 2.0;

                g.arc(width / 2.0, height / 2.0, width.min(height) / 2.5, 0.0, 2.0 * PI);
                g.clip();

                g.set_source_pixbuf(&pb, 0.0, hpos);
                g.rectangle(0.0, 0.0, width, height);
                g.fill();
            }

            Inhibit(false)
        });
    }

    fn circle(&self, path: String, size: Option<i32>) {
        if path.starts_with("mxc:") {
            self.default(String::from("image-loading-symbolic"), size);
            return;
        }

        self.clean();
        let da = self.create_da(size);
        let s = size.unwrap_or(40);

        let pixbuf = get_pixbuf_from_cache(&path, s);

        da.connect_draw(move |da, g| {
            use std::f64::consts::PI;

            let width = s as f64;
            let height = s as f64;

            let context = da.get_style_context().unwrap();

            gtk::render_background(&context, g, 0.0, 0.0, width, height);

            if let Some(ref pb) = pixbuf {
                let hpos: f64 = (width - (pb.get_height()) as f64) / 2.0;

                g.arc(width / 2.0, height / 2.0, width.min(height) / 2.0, 0.0, 2.0 * PI);
                g.clip();

                g.set_source_pixbuf(&pb, 0.0, hpos);
                g.rectangle(0.0, 0.0, width, height);
                g.fill();
            }

            Inhibit(false)
        });
    }
}

lazy_static! {
    static ref CACHED_PIXBUFS: Mutex<HashMap<(String, i32), Mutex<SendCell<Pixbuf>>>> = {
        Mutex::new(HashMap::new())
    };
}

// Since gdk_pixbuf::Pixbuf is refference counted and every avatar,
// use the cover of the cached image, We can only create a Pixbuf
// cover per person and pass around the Rc pointer.
//
// GObjects do not implement Send trait, so SendCell is a way around that.
// Note: SendCell will panic at runtime if it's accessed from any other thread than
// the one it was created.
// Also lazy_static requires Sync trait, so that's what the mutexes are.
fn get_pixbuf_from_cache(path: &str, width: i32) -> Option<Pixbuf> {
    let mut hashmap = CACHED_PIXBUFS.lock().unwrap();
    {
        // Query the cached hashmap
        let res = hashmap.get(&(path.to_owned(), width));
        if let Some(px) = res {
            let m = px.lock().unwrap();
            return Some(m.clone().into_inner());
        }
    }

    // Else, create the pixbuf
    let px = Pixbuf::new_from_file_at_scale(path, width as i32, -1, true).ok();
    if let Some(px) = px {
        // Insert it into the Hashmap cache
        hashmap.insert((path.to_owned(), width), Mutex::new(SendCell::new(px.clone())));
        return Some(px);
    }
    None
}