use gdk;
use gettextrs::{bindtextdomain, setlocale, textdomain, LocaleCategory};
use gio;
use gio::ApplicationExt;
use gio::ApplicationExtManual;
use glib;
use gtk;
use gtk::prelude::*;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use appop::AppOp;
use backend::BKResponse;
use backend::Backend;

use globals;
use uibuilder;

mod connect;

static mut OP: Option<Arc<Mutex<AppOp>>> = None;
#[macro_export]
macro_rules! APPOP {
    ($fn: ident, ($($x:ident),*) ) => {{
        let ctx = glib::MainContext::default();
        ctx.invoke(move || {
            $( let $x = $x.clone(); )*
            if let Some(op) = App::get_op() {
                op.lock().unwrap().$fn($($x),*);
            }
        });
    }};
    ($fn: ident) => {{
        APPOP!($fn, ( ) );
    }}
}

mod backend_loop;

pub use self::backend_loop::backend_loop;

/// State for the main thread.
///
/// It takes care of starting up the application and for loading and accessing the
/// UI.
pub struct App {
    ui: uibuilder::UI,

    op: Arc<Mutex<AppOp>>,
}

impl App {
    /// Create an App instance
    pub fn new() {
        let appid = globals::APP_ID
            .unwrap_or("org.gnome.FractalDevel")
            .to_string();

        let gtk_app = gtk::Application::new(Some(&appid[..]), gio::ApplicationFlags::empty())
            .expect("Failed to initialize GtkApplication");

        let path = "/org/gnome/Fractal".to_string();
        gtk_app.set_property_resource_base_path(Some(&path));

        gtk_app.connect_startup(move |gtk_app| {
            let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();

            let bk = Backend::new(tx);
            let apptx = bk.run();

            // Set up the textdomain for gettext
            setlocale(LocaleCategory::LcAll, "");
            bindtextdomain("fractal", globals::LOCALEDIR.unwrap_or("./fractal-gtk/po"));
            textdomain("fractal");

            let ui = uibuilder::UI::new();
            let window: gtk::Window = ui
                .builder
                .get_object("main_window")
                .expect("Couldn't find main_window in ui file.");
            window.set_application(gtk_app);

            if appid.ends_with("Devel") {
                window.get_style_context().map(|c| c.add_class("devel"));
            }

            let stack = ui
                .builder
                .get_object::<gtk::Stack>("main_content_stack")
                .expect("Can't find main_content_stack in ui file.");
            let stack_header = ui
                .builder
                .get_object::<gtk::Stack>("headerbar_stack")
                .expect("Can't find headerbar_stack in ui file.");

            /* Add account settings view to the main stack */
            let child = ui
                .builder
                .get_object::<gtk::Box>("account_settings_box")
                .expect("Can't find account_settings_box in ui file.");
            let child_header = ui
                .builder
                .get_object::<gtk::Box>("account_settings_headerbar")
                .expect("Can't find account_settings_headerbar in ui file.");
            stack.add_named(&child, "account-settings");
            stack_header.add_named(&child_header, "account-settings");

            let op = Arc::new(Mutex::new(AppOp::new(gtk_app.clone(), ui.clone(), apptx)));

            unsafe {
                OP = Some(op.clone());
            }

            backend_loop(rx);

            let app = App {
                ui: ui,
                op: op.clone(),
            };

            gtk_app.connect_activate(move |_| op.lock().unwrap().activate());

            app.connect_gtk();
            app.run();
        });

        gtk_app.run(&[]);
    }

    pub fn run(&self) {
        self.op.lock().unwrap().init();

        glib::set_application_name("fractal");
        glib::set_prgname(Some("fractal"));

        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/org/gnome/Fractal/app.css");
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().unwrap(),
            &provider,
            600,
        );
    }

    pub fn get_op() -> Option<Arc<Mutex<AppOp>>> {
        unsafe {
            match OP {
                Some(ref m) => Some(m.clone()),
                None => None,
            }
        }
    }
}
