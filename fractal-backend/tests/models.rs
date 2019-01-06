extern crate chrono;
extern crate fractal_backend;

use fractal_backend::init_local as init;
use fractal_backend::model::Member;
use fractal_backend::model::MemberModel;
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

#[test]
fn member_model() {
    let _ = init("").unwrap();

    assert!(Room::create_table().is_ok());
    let created = Member::create_table();
    assert!(created.is_ok());

    let m1 = Member {
        uid: String::from("UID"),
        alias: None,
        avatar: None,
    };
    let m2 = Member {
        uid: String::from("UID2"),
        alias: None,
        avatar: None,
    };
    let m3 = Member {
        uid: String::from("UID3"),
        alias: None,
        avatar: None,
    };
    assert!(m1.store().is_ok());
    assert!(m2.store().is_ok());
    assert!(m3.store().is_ok());

    let newm = Member::get("UID").unwrap();
    assert_eq!(m1, newm);

    let deleted = m1.delete();
    assert!(deleted.is_ok());

    let really_deleted = Member::get("UID");
    assert!(really_deleted.is_err());
}

#[test]
fn member_room_relation() {
    let _ = init("").unwrap();

    let created = Room::create_table();
    assert!(created.is_ok());
    let created = Member::create_table();
    assert!(created.is_ok());
    assert!(Member::create_relation_table().is_ok());

    let r = Room::new("ROOM ID".to_string(), Some("ROOM NAME".to_string()));
    let stored = r.store();
    assert!(stored.is_ok());

    let mut m = Member {
        uid: String::from("UID"),
        alias: None,
        avatar: None,
    };

    for i in 0..100 {
        m.uid = format!("USER {:04}", i);
        assert!(m.store().is_ok());
        assert!(m.store_relation("ROOM ID").is_ok());
    }

    for i in 0..100 {
        m.uid = format!("USER ROOM2 {:04}", i);
        assert!(m.store().is_ok());
        assert!(m.store_relation("ROOM ID 2").is_ok());
    }

    for i in 0..10 {
        let items = Member::get_range(&r.id, Some(10), Some(i * 10)).unwrap();
        for (j, m) in items.iter().enumerate() {
            let idx = 99 - (10 * i as usize + j);
            assert_eq!(m.uid, format!("USER {:04}", idx));
        }
    }

    let items = Member::get_range(&r.id, Some(10), Some(95)).unwrap();
    assert_eq!(items.len(), 5);

    let items = Member::get_range(&r.id, Some(10), Some(100)).unwrap();
    assert_eq!(items.len(), 0);

    let items = Member::get_range("ROOM ID 2", Some(10), None).unwrap();
    assert_eq!(items.len(), 10);
    for m in items {
        assert!(m.delete_relation("ROOM ID 2").is_ok());
    }

    assert_eq!(
        90,
        Member::get_range("ROOM ID 2", None, None).unwrap().len()
    );
    assert!(Member::delete_relations("ROOM ID 2").is_ok());
    assert_eq!(0, Member::get_range("ROOM ID 2", None, None).unwrap().len());
    assert_eq!(100, Member::get_range("ROOM ID", None, None).unwrap().len());
}
