#[macro_use]
pub mod util;
pub mod error;
pub mod globals;

pub mod backend;
pub mod cache;
mod client;
pub mod identity;
mod model;
pub mod r0;
mod serde;
pub mod types;

pub use ruma_identifiers as identifiers;
pub use url;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
