//! This crate defines a singleton for the database connection, the use of
//! `init` or `init_local` will initialize the database and that will be used by
//! all models queries to database
//!
//! If you want to use the connection as a process singleton, shared by all
//! threads, use the `init` function. In other case use the `init_local` that
//! provides a thread local singleton so each thread should call to `init_local`
//! and will have an unique database connection.
//!
//! Don't merge the `init` and the `init_local` function calls in the same
//! program. In any case, the local will take preference, so if you merge those
//! two, the local will be used if it's initialized.

extern crate chrono;
extern crate failure;
extern crate fractal_matrix_api as api;
extern crate rusqlite;
extern crate serde_json;

#[macro_use]
extern crate lazy_static;

pub mod model;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;

use failure::err_msg;
use failure::Error;
use rusqlite::Connection;

// Thread local singleton
thread_local! {
    static CONN_LOCAL: RefCell<Option<Connection>> = RefCell::new(None);
}

// CONN is a singleton
lazy_static! {
    static ref CONN: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
}

/// Function to run a query to the database with a connection
///
/// This function receives a closure that will receive a ref to the connection.
/// The `def` value is the return value used when there's no connection or
/// the connection is not created
pub fn conn<T, F>(f: F, def: T) -> T
where
    T: Sized,
    F: Fn(&Connection) -> T + Sized,
{
    // first we try with the thread local
    if let Some(output) = CONN_LOCAL.with(|c| match *c.borrow() {
        Some(ref c) => Some(f(c)),
        None => None,
    }) {
        return output;
    }

    // If the thread local is none or doesn't exists we check the global
    // singleton
    if let Ok(guard) = CONN.lock() {
        return match guard.as_ref() {
            Some(c) => f(c),
            None => def,
        };
    }

    def
}

/// Initialized the connection database as a singleton, shared by all threads
/// and used in the `conn` function. The `path` should be a correct string for a
/// sqlite database:
/// https://sqlite.org/c3ref/open.html#urifilenamesinsqlite3open
pub fn init(path: &str) -> Result<(), Error> {
    if let Ok(mut guard) = CONN.lock() {
        let conn = Connection::open(path).map_err(|e| err_msg(e.to_string()))?;
        *guard = Some(conn);
    }

    Ok(())
}

/// Initialized the connection database as a local thread variable, and used in
/// the `conn` function. The `path` should be a correct string for a sqlite
/// database:
/// https://sqlite.org/c3ref/open.html#urifilenamesinsqlite3open
pub fn init_local(path: &str) -> Result<(), Error> {
    CONN_LOCAL.with(|c| -> Result<(), Error> {
        let conn: Connection = Connection::open(path).map_err(|e| err_msg(e.to_string()))?;
        *c.borrow_mut() = Some(conn);
        Ok(())
    })
}
