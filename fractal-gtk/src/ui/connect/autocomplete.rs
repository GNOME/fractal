use gtk::prelude::*;

use crate::widgets;

use crate::app::AppRuntime;
use crate::ui::UI;

pub fn connect(ui: &UI, app_runtime: AppRuntime) {
    let popover = ui
        .builder
        .get_object::<gtk::Popover>("autocomplete_popover")
        .expect("Can't find autocomplete_popover in ui file.");
    let listbox = ui
        .builder
        .get_object::<gtk::ListBox>("autocomplete_listbox")
        .expect("Can't find autocomplete_listbox in ui file.");
    let window: gtk::Window = ui
        .builder
        .get_object("main_window")
        .expect("Can't find main_window in ui file.");

    widgets::Autocomplete::new(
        app_runtime,
        window,
        ui.sventry.view.clone(),
        popover,
        listbox,
    )
    .connect();
}
