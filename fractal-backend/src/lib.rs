extern crate failure;
extern crate fractal_matrix_api as api;
extern crate rusqlite;

#[macro_use]
extern crate lazy_static;

pub mod model;

use std::sync::Arc;
use std::sync::Mutex;

use failure::err_msg;
use failure::Error;
use rusqlite::Connection;

// CONN is a singleton
lazy_static! {
    static ref CONN: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
}

pub fn conn<T, F>(f: F, def: T) -> T
where
    T: Sized,
    F: Fn(&Connection) -> T + Sized,
{
    if let Ok(guard) = CONN.lock() {
        return match guard.as_ref() {
            Some(c) => f(c),
            None => def,
        };
    }

    def
}

pub fn init(path: &str) -> Result<(), Error> {
    if let Ok(mut guard) = CONN.lock() {
        let conn = Connection::open(path).map_err(|e| err_msg(e.to_string()))?;
        *guard = Some(conn);
    }

    Ok(())
}
