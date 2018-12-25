#![deny(unused_extern_crates)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

extern crate chrono;
extern crate glib;
extern crate md5;
extern crate regex;
extern crate reqwest;
extern crate tree_magic;
extern crate urlencoding;

extern crate url;

#[macro_use]
mod util;
pub use util::cache_path;
pub mod error;
mod globals;

pub mod backend;
mod cache;
mod model;
pub mod types;

pub(crate) use serde_json::Value as JsonValue;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
