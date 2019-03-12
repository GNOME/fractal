#[macro_use]
pub mod util;
pub mod error;
pub mod globals;

pub mod backend;
pub mod cache;
pub mod identity;
mod model;
pub mod r0;
mod ser;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
