Fractal
=======

Fractal is a Matrix messaging app for GNOME written in Rust. Its interface is optimized for collaboration in large groups, such as free software projects.


 * Come to talk to us on Matrix: https://matrix.to/#/#fractal-gtk:matrix.org
 * Main repository: https://gitlab.gnome.org/World/fractal/

![screenshot](https://gitlab.gnome.org/World/fractal/raw/master/screenshots/fractal.png)

## Build Instructions

You need Meson and Ninja (as well as Rust and Cargo) to build Fractal.

### GNU/Linux

```
meson . _build --prefix=/usr/local
ninja -C _build
```

### macOS

```
brew install gtk3+ dbus bash
# empirically needs 3.22.19 or later of gtk3+
# ...and run configure as:
/usr/local/bin/bash -c meson . _build --prefix=/usr/local
```

You may also need to comment out the `notification.show` block in
`./fractal-gtk/src/app.rs` as apparently `notification.wait_for_action`
is missing on MacOS.

## Supported m.room.message (msgtypes)

msgtypes          | Recv                | Send
--------          | -----               | ------
m.text            | Done                | Done
m.emote           | Done                | Done
m.notice          |                     |
m.image           | Done                | Done
m.file            | Done                | Done
m.location        |                     |
m.video           | Done                | Done
m.audio           | Done                | Done

Full reference in: https://matrix.org/docs/spec/client\_server/r0.2.0.html#m-room-message-msgtypes

The origin of Fractal
---------------------

This project is based on ruma-gtk https://github.com/jplatte/ruma-gtk

Instead of using RUMA Client, Fractal calls directly to the matrix.org
REST API.

The first version of this project was called guillotine, based on french revolution,
in relation with the Riot client name, but it's a negative name so we decide
to change for a math one.

The name Fractal was proposed by Regina Bíró.
