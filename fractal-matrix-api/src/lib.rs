#[macro_use]
pub mod identity;
pub mod r0;

pub use reqwest;
pub use ruma_identifiers as identifiers;
pub use url;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
