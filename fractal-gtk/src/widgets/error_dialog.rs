use gio::ApplicationExt;
use gtk;
use gtk::prelude::*;

// Shows an error dialog, and if it's fatal it will quit the application once
// the dialog is closed
pub fn new(fatal: bool, text: &str) {
    let app = gio::Application::get_default()
        .expect("No default application")
        .downcast::<gtk::Application>()
        .expect("Default application has wrong type");

    let dialog = gtk::MessageDialog::new(
        app.get_active_window().as_ref(),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        text,
    );

    let app_weak = app.downgrade();
    dialog.connect_response(move |dialog, _| {
        dialog.destroy();

        if fatal {
            let app = upgrade_weak!(app_weak);
            app.quit();
        }
    });

    dialog.set_resizable(false);
    dialog.show_all();
}
