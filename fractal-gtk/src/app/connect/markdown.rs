extern crate gtk;
extern crate sourceview;

use self::gtk::prelude::*;
use self::sourceview::prelude::*;

use app::App;

impl App {
    pub fn connect_markdown(&self) {
        let md_popover_btn: gtk::MenuButton = self.ui.builder
            .get_object("markdown_button")
            .expect("Couldn't find markdown_button in ui file.");

        let popover: gtk::Popover = self.ui.builder
            .get_object("markdown_popover")
            .expect("Couldn't find markdown_popover in ui file.");

        let markdown_switch: gtk::Switch = self.ui.builder
            .get_object("markdown_switch")
            .expect("Couldn't find markdown_switch in ui file.");

        let txt: gtk::Grid = self.ui.builder
            .get_object("tutorial_text_box")
            .expect("Couldn't find tutorial_text_box in ui file.");

        let md_img = self.ui.builder
            .get_object::<gtk::Image>("md_img")
            .expect("Couldn't find md_img in ui file.");

        let md_lang = sourceview::LanguageManager::get_default()
                                                   .map_or(None, |lm| lm.get_language("markdown"));

        md_popover_btn.set_popover(Some(&popover));

        let op = self.op.clone();
        let ui = self.ui.clone();
        markdown_switch.clone().connect_property_active_notify(move |_| {
            op.lock().unwrap().md_enabled = markdown_switch.get_active();
            if !markdown_switch.get_active() {
                md_img.set_from_icon_name("format-justify-left-symbolic",1);
                txt.get_style_context().unwrap().add_class("dim-label");

                let buffer: sourceview::Buffer = ui.builder
                    .get_object("msg_entry_buffer")
                    .expect("Couldn't find msg_entry_buffer in ui file.");
                buffer.set_highlight_matching_brackets(false);
                buffer.set_language(None);
                buffer.set_highlight_syntax(false);
            } else {
                md_img.set_from_icon_name("format-indent-more-symbolic",1);
                txt.get_style_context().unwrap().remove_class("dim-label");

                if let Some(md_lang) = md_lang.clone() {
                    let buffer: sourceview::Buffer = ui.builder
                        .get_object("msg_entry_buffer")
                        .expect("Couldn't find msg_entry_buffer in ui file.");
                    buffer.set_highlight_matching_brackets(true);
                    buffer.set_language(&md_lang);
                    buffer.set_highlight_syntax(true);
                }
            }
        });
    }
}
