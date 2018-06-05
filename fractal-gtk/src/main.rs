#![deny(unused_extern_crates)]
extern crate glib;
extern crate gio;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate fractal_matrix_api as fractal_api;

extern crate html2pango;

extern crate gspell;


use fractal_api::backend;
use fractal_api::types;
use fractal_api::error;

mod globals;
#[macro_use]
mod util;
mod cache;
mod uibuilder;
mod static_resources;
mod passwd;
#[macro_use]
mod app;
mod widgets;

mod appop;

use app::App;


fn main() {
    static_resources::init().expect("GResource initialization failed.");
    App::new();
}
