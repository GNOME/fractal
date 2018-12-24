extern crate chrono;
extern crate fractal_backend;

use fractal_backend::init_local as init;
use fractal_backend::model::Message;
use fractal_backend::model::MessageModel;
use fractal_backend::model::Model;
use fractal_backend::model::Room;

use chrono::prelude::*;

#[test]
fn room_model() {
    let _ = init("").unwrap();

    let created = Room::create_table();
    assert!(created.is_ok());

    let mut r = Room::new("ROOM ID".to_string(), Some("ROOM NAME".to_string()));
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

#[test]
fn message_model() {
    let _ = init("").unwrap();

    let created = Message::create_table();
    assert!(created.is_ok());

    let mut msg = Message::default();
    msg.id = Some("MSGID".to_string());
    let stored = msg.store();
    assert!(stored.is_ok());

    let newm = Message::get("MSGID").unwrap();
    assert_eq!(msg, newm);

    let deleted = msg.delete();
    assert!(deleted.is_ok());

    let really_deleted = Message::get("MSGID");
    assert!(really_deleted.is_err());
}

#[test]
fn message_room_relation() {
    let _ = init("").unwrap();

    let created = Room::create_table();
    assert!(created.is_ok());
    let created = Message::create_table();
    assert!(created.is_ok());

    let r = Room::new("ROOM ID".to_string(), Some("ROOM NAME".to_string()));
    let stored = r.store();
    assert!(stored.is_ok());

    let mut msg = Message::default();
    msg.room = r.id.clone();

    for i in 0..100 {
        msg.id = Some(format!("MSG {}", i));
        msg.date = Local.ymd(1970, 1, 1).and_hms(0, i / 60, i % 60);
        let _ = msg.store();
    }

    msg.room = "ROOM ID 2".to_string();
    for i in 0..100 {
        msg.id = Some(format!("MSG ROOM2 {}", i));
        msg.date = Local.ymd(1970, 1, 1).and_hms(0, i / 60, i % 60);
        let _ = msg.store();
    }

    for i in 0..10 {
        let items = Message::get_range(&r.id, Some(10), Some(i * 10)).unwrap();
        for (j, m) in items.iter().enumerate() {
            let idx = 99 - (10 * i as usize + j);
            assert_eq!(m.id, Some(format!("MSG {}", idx)));
        }
    }

    let items = Message::get_range(&r.id, Some(10), Some(95)).unwrap();
    assert_eq!(items.len(), 5);

    let items = Message::get_range(&r.id, Some(10), Some(100)).unwrap();
    assert_eq!(items.len(), 0);
}
