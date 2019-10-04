use letter_avatar;
use std::cell::RefCell;
use std::rc::Rc;

use cairo;
use fractal_api::util::cache_dir_path;
use gdk::ContextExt;
use gdk_pixbuf::Pixbuf;
use gtk;
use gtk::prelude::*;
pub use gtk::DrawingArea;

pub enum AvatarBadgeColor {
    Gold,
    Silver,
    Grey,
}

pub type Avatar = gtk::Overlay;

pub struct AvatarData {
    uid: String,
    username: Option<String>,
    size: i32,
    scale: i32,
    cache: Option<Pixbuf>,
    pub widget: gtk::DrawingArea,
    fallback: cairo::ImageSurface,
}

impl AvatarData {
    pub fn redraw_fallback(&mut self, username: Option<String>) {
        self.username = username.clone();
        /* This function should never fail */
        self.fallback = letter_avatar::generate::new(
            self.uid.clone(),
            username,
            (self.size * self.scale) as f64,
        )
        .expect("this function should never fail");
        self.widget.queue_draw();
    }

    pub fn redraw_pixbuf(&mut self) {
        let path = cache_dir_path(None, &self.uid).unwrap_or_default();
        self.cache = load_pixbuf(&path, self.size);
        self.widget.queue_draw();
    }
}

pub trait AvatarExt {
    fn avatar_new(size: Option<i32>) -> gtk::Overlay;
    fn clean(&self);
    fn create_da(&self, size: Option<i32>) -> DrawingArea;
    fn circle(
        &self,
        uid: String,
        username: Option<String>,
        size: i32,
        badge: Option<AvatarBadgeColor>,
        badge_size: Option<i32>,
    ) -> Rc<RefCell<AvatarData>>;
}

impl AvatarExt for gtk::Overlay {
    fn clean(&self) {
        for ch in self.get_children().iter() {
            self.remove(ch);
        }
    }

    fn create_da(&self, size: Option<i32>) -> DrawingArea {
        let da = DrawingArea::new();

        let s = size.unwrap_or(40);
        da.set_size_request(s, s);
        self.add(&da);
        self.show_all();

        da
    }

    fn avatar_new(size: Option<i32>) -> gtk::Overlay {
        let b = gtk::Overlay::new();
        b.create_da(size);
        b.show_all();
        b.get_style_context().add_class("avatar");

        b
    }
    /// # Arguments
    /// * `uid` - Matrix ID
    /// * `username` - Full name
    /// * `size` - Size of the avatar
    /// * `badge_color` - Badge color. None for no badge
    /// * `badge_size` - Badge size. None for size / 3
    fn circle(
        &self,
        uid: String,
        username: Option<String>,
        size: i32,
        badge_color: Option<AvatarBadgeColor>,
        badge_size: Option<i32>,
    ) -> Rc<RefCell<AvatarData>> {
        self.clean();
        let da = self.create_da(Some(size));
        let scale = da.get_scale_factor();
        let path = cache_dir_path(None, &uid).unwrap_or_default();
        let user_avatar = load_pixbuf(&path, size * scale);
        let uname = username.clone();
        /* remove IRC postfix from the username */
        let username = if let Some(u) = username {
            Some(u.trim_end_matches(" (IRC)").to_owned())
        } else {
            None
        };
        /* This function should never fail */
        let fallback = letter_avatar::generate::new(uid.clone(), username, (size * scale) as f64)
            .expect("this function should never fail");

        // Power level badge setup
        let has_badge = badge_color.is_some();
        let badge_size = badge_size.unwrap_or(size / 3);
        if let Some(color) = badge_color {
            let badge = gtk::Box::new(gtk::Orientation::Vertical, 0);
            badge.set_size_request(badge_size, badge_size);
            badge.set_valign(gtk::Align::Start);
            badge.set_halign(gtk::Align::End);
            badge.get_style_context().add_class("badge-circle");
            badge.get_style_context().add_class(match color {
                AvatarBadgeColor::Gold => "badge-gold",
                AvatarBadgeColor::Silver => "badge-silver",
                AvatarBadgeColor::Grey => "badge-grey",
            });
            self.add_overlay(&badge);
        }

        let data = AvatarData {
            uid: uid.clone(),
            username: uname,
            size: size,
            scale: scale,
            cache: user_avatar,
            fallback: fallback,
            widget: da.clone(),
        };
        let avatar_cache: Rc<RefCell<AvatarData>> = Rc::new(RefCell::new(data));

        let user_cache = avatar_cache.clone();
        da.connect_draw(move |da, g| {
            use std::f64::consts::PI;
            let width = size as f64;
            let height = size as f64;

            g.set_antialias(cairo::Antialias::Best);

            {
                g.set_fill_rule(cairo::FillRule::EvenOdd);
                g.arc(
                    width / 2.0,
                    height / 2.0,
                    width.min(height) / 2.0,
                    0.0,
                    2.0 * PI,
                );
                if has_badge {
                    g.clip_preserve();
                    g.new_sub_path();
                    let badge_radius = badge_size as f64 / 2.0;
                    g.arc(
                        width - badge_radius,
                        badge_radius,
                        badge_radius * 1.4,
                        0.0,
                        2.0 * PI,
                    );
                }
                g.clip();

                let data = user_cache.borrow();
                if let Some(ref pb) = data.cache {
                    let context = da.get_style_context();
                    gtk::render_background(&context, g, 0.0, 0.0, width, height);

                    let hpos: f64 = (width - (pb.get_height()) as f64) / 2.0;
                    g.set_source_pixbuf(&pb, 0.0, hpos);
                } else {
                    /* use fallback */
                    g.set_source_surface(&data.fallback, 0f64, 0f64);
                }
            }

            let scale_f = scale as f64;
            g.scale(1.0 / scale_f, 1.0 / scale_f);
            g.rectangle(0.0, 0.0, width, height);
            g.fill();

            Inhibit(false)
        });

        avatar_cache
    }
}

fn load_pixbuf(path: &str, size: i32) -> Option<Pixbuf> {
    if let Some(pixbuf) = Pixbuf::new_from_file(&path).ok() {
        // FIXME: We end up loading the file twice but we need to load the file first to find out its dimentions to be
        // able to decide wether to scale by width or height and gdk doesn't provide simple API to scale a loaded
        // pixbuf while preserving aspect ratio.
        if pixbuf.get_width() > pixbuf.get_height() {
            Pixbuf::new_from_file_at_scale(&path, -1, size, true).ok()
        } else {
            Pixbuf::new_from_file_at_scale(&path, size, -1, true).ok()
        }
    } else {
        None
    }
}
