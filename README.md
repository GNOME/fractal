# Fractal

Fractal is a Matrix messaging app for GNOME written in Rust. Its interface is optimized for collaboration in large groups, such as free software projects.

* Come to talk to us on Matrix: <https://matrix.to/#/#fractal-gtk:matrix.org>
* Main repository: <https://gitlab.gnome.org/World/fractal/>

![screenshot](https://gitlab.gnome.org/World/fractal/raw/master/screenshots/fractal.png)

## Installation instructions

You can find Fractal installation instructions through packages on the [GNOME wiki](https://wiki.gnome.org/Apps/Fractal).

## Build Instructions

You need Meson and Ninja (as well as Rust and Cargo) to build Fractal.

### GNU/Linux

```sh
meson . _build --prefix=/usr/local
ninja -C _build
sudo ninja -C _build install
```

### macOS

```sh
brew install gtk+3 dbus bash adwaita-icon-theme
# empirically needs 3.22.19 or later of gtk3+
# ...and run configure as:
/usr/local/bin/bash -c "meson . _build --prefix=/usr/local"
ninja -C _build
sudo ninja -C _build install
```

### Translations

If you want to add a new language you should update the file
`fractal-gtk/po/LINUGAS` and add the new lang to the list.

To generate .pot files you should run:

```
ninja -C _build fractal-pot
```

To generate .po files you should run:

```
ninja -C _build fractal-update-po
```

### Password Storage

Fractal uses Secret Service to store the password so you should have
running some daemon that give that service. If you're using GNOME or KDE
this should work for you out of the box with gnome-keyring or
ksecretservice.

There's a way to avoid the need of secret service and store the password in
a unsecure way, in a plain json file. We don't recommend to use this form,
but if you want, it's possible to configure using gsettings:

```
$ gsettings set org.gnome.Fractal password-storage 'Plain text'
```

Or if you're using flatpak

```
$ flatpak run --command="bash" org.gnome.Fractal
$ gsettings set org.gnome.Fractal password-storage 'Plain text'
$ exit
```

To go back to use Secret service:

```
$ gsettings set org.gnome.Fractal password-storage 'Secret Service'
```

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

Full reference in: <https://matrix.org/docs/spec/client\_server/r0.2.0.html#m-room-message-msgtypes>

## The origin of Fractal

This project is based on Fest <https://github.com/fest-im/fest>, formerly called ruma-gtk.

Instead of using RUMA Client, Fractal calls directly to the matrix.org
REST API.

The first version of this project was called guillotine, based on French revolution,
in relation with the Riot client name, but it's a negative name so we decide
to change for a math one.

The name Fractal was proposed by Regina Bíró.
