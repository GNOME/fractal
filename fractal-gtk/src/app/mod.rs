extern crate gtk;
extern crate gdk;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gettextrs::{setlocale, LocaleCategory, bindtextdomain, textdomain};
use std::env;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use gio;
use glib;
use gio::ApplicationExt;
use gio::ApplicationExtManual;

use backend::Backend;
use backend::BKResponse;
use appop::AppOp;

use globals;
use uibuilder;

mod connect;
mod actions;

pub use self::appop_loop::InternalCommand;


static mut OP: Option<Arc<Mutex<AppOp>>> = None;
#[macro_export]
macro_rules! APPOP {
    ($fn: ident, ($($x:ident),*) ) => {{
        if let Some(ctx) = glib::MainContext::default() {
            ctx.invoke(move || {
                $( let $x = $x.clone(); )*
                if let Some(op) = App::get_op() {
                    op.lock().unwrap().$fn($($x),*);
                }
            });
        }
    }};
    ($fn: ident) => {{
        APPOP!($fn, ( ) );
    }}
}

mod appop_loop;
mod backend_loop;

pub use self::backend_loop::backend_loop;
use self::appop_loop::appop_loop;


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
        let appid = match env::var("FRACTAL_ID") {
            Ok(id) => id,
            Err(_) => globals::APP_ID.to_string(),
        };

        let gtk_app = gtk::Application::new(Some(&appid[..]), gio::ApplicationFlags::empty())
            .expect("Failed to initialize GtkApplication");

        gtk_app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);

        gtk_app.connect_startup(move |gtk_app| {
            let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();
            let (itx, irx): (Sender<InternalCommand>, Receiver<InternalCommand>) = channel();

            let bk = Backend::new(tx);
            let apptx = bk.run();

            // Set up the textdomain for gettext
            setlocale(LocaleCategory::LcAll, "");
            bindtextdomain("fractal", globals::LOCALEDIR);
            textdomain("fractal");


            let ui = uibuilder::UI::new();
            let window: gtk::Window = ui.builder
                .get_object("main_window")
                .expect("Couldn't find main_window in ui file.");
            window.set_application(gtk_app);

            /* we have to overwrite the default behavior for valign of the title widget
             * since it is force to be centered */
            ui.builder
            .get_object::<gtk::MenuButton>("room_menu_button")
            .expect("Can't find back_button in ui file.").set_valign(gtk::Align::Fill);

            let stack = ui.builder
                .get_object::<gtk::Stack>("main_content_stack")
                .expect("Can't find main_content_stack in ui file.");
            let stack_header = ui.builder
                .get_object::<gtk::Stack>("headerbar_stack")
                .expect("Can't find headerbar_stack in ui file.");

            /* Add account settings view to the main stack */
            let child = ui.builder
                .get_object::<gtk::Box>("account_settings_box")
                .expect("Can't find account_settings_box in ui file.");
            let child_header = ui.builder
                .get_object::<gtk::Box>("account_settings_headerbar")
                .expect("Can't find account_settings_headerbar in ui file.");
            stack.add_named(&child, "account-settings");
            stack_header.add_named(&child_header, "account-settings");

            /* Add media viewer to the main stack */
            let child = ui.builder
                .get_object::<gtk::Box>("media_viewer_box")
                .expect("Can't find media_viewer_box in ui file.");
            let child_header = ui.builder
                .get_object::<gtk::Box>("media_viewer_headerbar_box")
                .expect("Can't find media_viewer_headerbar_box in ui file.");
            stack.add_named(&child, "media-viewer");
            stack_header.add_named(&child_header, "media-viewer");


            let op = Arc::new(Mutex::new(
                AppOp::new(gtk_app.clone(), ui.clone(), apptx, itx)
            ));

            unsafe {
                OP = Some(op.clone());
            }

            backend_loop(rx);
            appop_loop(irx);

            let app = App {
                ui: ui,
                op: op.clone(),
            };

            gtk_app.connect_activate(move |_| { op.lock().unwrap().activate() });

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
        gtk::StyleContext::add_provider_for_screen(&gdk::Screen::get_default().unwrap(), &provider, 600);
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
