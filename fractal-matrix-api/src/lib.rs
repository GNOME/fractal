#![deny(unused_extern_crates)]

#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

extern crate rayon;

#[macro_use]
pub mod util;
pub mod error;
pub mod globals;

mod model;
pub mod types;
pub mod cache;
pub mod backend;

lazy_static! {
    pub static ref MEDIA_POOL: rayon::ThreadPool = {
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .build()
            .expect("Failed to initialize rayon threadpool.")
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
