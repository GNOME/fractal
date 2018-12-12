extern crate fractal_backend;

use fractal_backend::init;
use fractal_backend::model::Model;
use fractal_backend::model::Room;
use std::fs::remove_file;

#[test]
fn room_model() {
    let _ = remove_file("/tmp/db.sqlite3");
    let _ = init("/tmp/db.sqlite3").unwrap();

    let mut r = Room::new("ROOM ID".to_string(), Some("ROOM NAME".to_string()));
    let created = Room::create_table();
    assert!(created.is_ok());
    let stored = r.store();
    assert!(stored.is_ok());

    let newr = Room::get("ROOM ID").unwrap();
    assert_eq!(r, newr);

    let deleted = r.delete();
    assert!(deleted.is_ok());

    let really_deleted = Room::get("ROOM ID");
    assert!(really_deleted.is_err());

    for i in 0..10 {
        r.id = format!("ROOM {}", i);
        let _ = r.store();
    }

    let rooms = Room::all().unwrap();
    assert_eq!(rooms.len(), 10);

    for (i, r) in rooms.iter().enumerate() {
        assert_eq!(r.id, format!("ROOM {}", i));
    }
}
