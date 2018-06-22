#![deny(unused_extern_crates)]

#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

extern crate url;

#[macro_use]
pub mod util;
pub mod error;
pub mod globals;

mod model;
pub mod types;
pub mod cache;
pub mod backend;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
