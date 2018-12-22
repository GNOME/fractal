# Fractal

Fractal is a Matrix messaging app for GNOME written in Rust. Its interface is optimized for collaboration in large groups, such as free software projects.

* Come to talk to us on Matrix: <https://matrix.to/#/#fractal-gtk:matrix.org>
* Main repository: <https://gitlab.gnome.org/GNOME/fractal/>

![screenshot](https://gitlab.gnome.org/GNOME/fractal/raw/master/screenshots/fractal.png)

## Installation instructions

Flatpak is the recommended installation method. You can get the official
Fractal flatpak on flathub.

<a href="https://flathub.org/apps/details/org.gnome.Fractal">
<img src="https://flathub.org/assets/badges/flathub-badge-i-en.png" width="190px" />
</a>

## Build Instructions

### Flatpak

Flatpak is the recommended way of building and installing Fractal.

First you need to make sure you have the GNOME SDK and Rust toolchain installed.

```
# Add flathub and the gnome-nightly repo
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists gnome-nightly https://sdk.gnome.org/gnome-nightly.flatpakrepo

# Install the gnome-nightly Sdk and Platform runtime
flatpak install --user gnome-nightly org.gnome.Sdk org.gnome.Platform

# Install the required rust-stable extension from flathub
flatpak install --user flathub org.freedesktop.Sdk.Extension.rust-stable//18.08
```

Then you go ahead and build Fractal.

```
flatpak-builder --user --install fractal org.gnome.Fractal.json
```

### GNU/Linux

If you decide to ignore our recommendation and build on your host system,
outside of flatpak, you will need Meson and Ninja (as well as Rust and Cargo).

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

Fractal is translated by the GNOME translation team on
[Damned lies](https://l10n.gnome.org/).

If you want to add *a new language* you should update the file
`fractal-gtk/po/LINGUAS` and add the code for that language
to the list.

Get the pot file from [the Fractal module page on Damned lies](https://l10n.gnome.org/module/fractal/).

### Password Storage

Fractal uses [Secret Service](https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/)
to store the password so you should have something providing 
that service on your system. If you're using GNOME or KDE
this should work for you out of the box with gnome-keyring or
ksecretservice.

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

## Frequently Asked Questions

* Does Fractal have encryption support? Will it ever?

Fractal does not currently have encryption support, but
there is an initiative for it.

The current effort is to create a monolithic module for Fractal to interact with.
Said module will handle all cryptographic tasks.
The goal is to reduce the amount of work required for implementing end-to-end encryption
by having one thing to interact with, instead of a huge API surface.

Code and further information for this module can be found at [World/metaolm](https://gitlab.gnome.org/World/metaolm).

* Can I run Fractal with the window closed?

Currently Fractal does not support this. Fractal is a
GNOME application, and accordingly adheres GNOME
guidelines and paradigms. This will be revisited if or
when GNOME gets a "Do Not Disturb" feature.

## The origin of Fractal

This project is based on Fest <https://github.com/fest-im/fest>, formerly called ruma-gtk.

Instead of using RUMA Client, Fractal calls directly to the matrix.org
REST API.

The first version of this project was called guillotine, based on French revolution,
in relation with the Riot client name, but it's a negative name so we decide
to change for a math one.

The name Fractal was proposed by Regina Bíró.
