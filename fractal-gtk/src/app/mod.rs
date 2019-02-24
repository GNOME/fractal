use gettextrs::{bindtextdomain, setlocale, textdomain, LocaleCategory};
use gio::prelude::*;
use gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::ops;
use std::rc::{Rc, Weak};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, Weak as SyncWeak};
use gio::{Settings};

use crate::appop::AppOp;
use crate::backend::BKResponse;
use crate::backend::Backend;

use crate::actions;
use crate::globals;
use crate::uibuilder;

mod connect;
mod windowstate;

use windowstate::WindowState;

static mut OP: Option<SyncWeak<Mutex<AppOp>>> = None;
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

// Our refcounted application struct for containing all the state we have to carry around.
// TODO: subclass gtk::Application once possible
pub struct App(Rc<AppInner>);

pub struct AppInner {
    main_window: gtk::ApplicationWindow,
    /* Add widget directly here in place of uibuilder::UI*/
    ui: uibuilder::UI,

    // TODO: Remove op needed in connect, but since it is global we could remove it form here
    op: Arc<Mutex<AppOp>>,
}

// Deref into the contained struct to make usage a bit more ergonomic
impl ops::Deref for App {
    type Target = AppInner;

    fn deref(&self) -> &AppInner {
        &*self.0
    }
}

// Weak reference to our application struct
pub struct AppWeak(Weak<AppInner>);

impl AppWeak {
    // Upgrade to a strong reference if it still exists
    pub fn upgrade(&self) -> Option<App> {
        self.0.upgrade().map(App)
    }
}

impl App {
    pub fn new(gtk_app: &gtk::Application) -> App {
        let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();

        let bk = Backend::new(tx);
        let apptx = bk.run();

        // Set up the textdomain for gettext
        setlocale(LocaleCategory::LcAll, "");
        bindtextdomain("fractal", globals::LOCALEDIR.unwrap_or("./fractal-gtk/po"));
        textdomain("fractal");

        glib::set_application_name("fractal");
        glib::set_prgname(Some("fractal"));

        // Add style provider
        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/org/gnome/Fractal/app.css");
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let ui = uibuilder::UI::new();
        let window: gtk::ApplicationWindow = ui
            .builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        window.set_application(gtk_app);

        window.set_title("Fractal");

        let settings: gio::Settings = gio::Settings::new("org.gnome.Fractal");
        let window_state = WindowState::load_from_gsettings(&settings);
        window.set_default_size(window_state.width, window_state.height);
        if window_state.is_maximized == true {
            window.maximize();
        }
        else if window_state.x > 0 && window_state.y > 0 {
            window.move_(window_state.x, window_state.y);
        }
        window.show_all();

        if gtk_app
            .get_application_id()
            .map_or(false, |s| s.ends_with("Devel"))
        {
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

        let op = Arc::new(Mutex::new(AppOp::new(ui.clone(), apptx)));

        unsafe {
            OP = Some(Arc::downgrade(&op));
        }

        backend_loop(rx);

        actions::Global::new(gtk_app, &op);

        let app = App(Rc::new(AppInner {
            main_window: window,
            ui,
            op,
        }));

        app.connect_gtk();

        app
    }

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> AppWeak {
        AppWeak(Rc::downgrade(&self.0))
    }

    pub fn on_startup(gtk_app: &gtk::Application) {
        // Create application
        let app = App::new(gtk_app);

        let app_weak = app.downgrade();
        gtk_app.connect_activate(move |_| {
            let app = upgrade_weak!(app_weak);
            app.on_activate();
        });

        let app_weak = app.downgrade();
        app.main_window
            .connect_property_has_toplevel_focus_notify(move |_| {
                let app = upgrade_weak!(app_weak);
                app.op.lock().unwrap().mark_active_room_messages();
            });

        let app_clone = gtk_app.clone();
        app.main_window
            .connect_delete_event(move |window, _| {
                let settings: Settings = Settings::new("org.gnome.Fractal");
                let window_state = WindowState::from_window(window);
                window_state.save_in_gsettings(&settings);
                app_clone.quit();
                Inhibit(false)
            });

        app.op.lock().unwrap().init();

        // When the application is shut down we drop our app struct
        let app_container = RefCell::new(Some(app));
        gtk_app.connect_shutdown(move |_| {
            let app = app_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            app.on_shutdown();
        });
    }

    fn on_activate(&self) {
        self.main_window.show();
        // FIXME: present() dosen't work currently on wayland because of
        // https://gitlab.gnome.org/GNOME/gtk/issues/624
        self.main_window
            .present_with_time((glib::get_monotonic_time() / 1000) as u32)
    }

    fn on_shutdown(self) {
        self.op.lock().unwrap().quit();
    }

    // Legazy function to get AppOp
    // This shouldn't be used in new code
    pub fn get_op() -> Option<Arc<Mutex<AppOp>>> {
        unsafe { OP.as_ref().and_then(|x| x.upgrade()) }
    }
}
