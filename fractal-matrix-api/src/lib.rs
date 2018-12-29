#![deny(unused_extern_crates)]

#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

extern crate chrono;
extern crate directories;
extern crate md5;
extern crate regex;
extern crate reqwest;
extern crate tree_magic;
extern crate urlencoding;

extern crate url;

#[macro_use]
pub mod util;
pub mod error;
pub mod globals;

pub mod backend;
pub mod cache;
mod model;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
