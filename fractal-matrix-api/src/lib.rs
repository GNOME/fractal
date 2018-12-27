#[macro_use]
mod util;
pub mod backend;
mod cache;
pub mod error;
mod globals;
mod model;
pub mod types;

pub use crate::util::cache_path;
pub(crate) use serde_json::Value as JsonValue;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
