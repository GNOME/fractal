#![deny(unused_extern_crates)]
extern crate glib;
extern crate gio;
extern crate send_cell;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate fractal_api;
use fractal_api::backend;
use fractal_api::types;
use fractal_api::error;

mod globals;
#[macro_use]
mod util;
mod widgets;
mod cache;
mod app;
mod static_resources;

use app::App;


fn main() {
    static_resources::init().expect("GResource initialization failed.");
    App::new();
}
