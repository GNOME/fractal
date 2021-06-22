# Fractal

Fractal is a Matrix messaging app for GNOME written in Rust. Its interface is optimized for collaboration in large groups, such as free software projects.

* Come to talk to us on Matrix: <https://matrix.to/#/#fractal:gnome.org>
* Main repository: <https://gitlab.gnome.org/GNOME/fractal/>

![screenshot](https://gitlab.gnome.org/GNOME/fractal/raw/master/screenshots/fractal.png)

## Fractal-next initiative
We are working on rewriting Fractal from scratch using [GTK4](https://www.gtk.org/) and the [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk). This effort is called fractal-next.

We already talked several times in the past about rewriting the application, but for different reasons we didn't do it. Now that the [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) exists, which does a lot of the heavy lifting for us, we have a good starting point to build Fractal without the need to implement every single feature from the Matrix API. Finally with the release of GTK4 we would need to rework most of Fractal's code anyways. Therefore, it just makes sense to start over and build Fractal with all features (e.g end-to-end encryption) we have in mind.

The main development branch is [fractal-next](https://gitlab.gnome.org/GNOME/fractal/-/tree/fractal-next). Issues that target fractal-next should be labeled accordingly as "fractal-next".
Our current work focuses on getting the same level of features as we already have in the stable version. Then fractal-next will replace our current codebase, merging it into the main git branch and becoming the new nightly version. You can follow along our progress towards that goal by looking at the [feature parity milestone](https://gitlab.gnome.org/GNOME/fractal/-/milestones/18).


## Installation instructions

Flatpak is the recommended installation method. You can get the official
Fractal Flatpak on Flathub.

<a href="https://flathub.org/apps/details/org.gnome.Fractal">
<img src="https://flathub.org/assets/badges/flathub-badge-i-en.png" width="190px" />
</a>

## Build Instructions

### Flatpak

Flatpak is the recommended way of building and installing Fractal.

First you need to make sure you have the GNOME SDK and Rust toolchain installed.

```
# Add Flathub and the gnome-nightly repo
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists gnome-nightly https://nightly.gnome.org/gnome-nightly.flatpakrepo

# Install the gnome-nightly Sdk and Platform runtime
flatpak install --user gnome-nightly org.gnome.Sdk org.gnome.Platform

# Install the required rust-stable extension from Flathub
flatpak install --user flathub org.freedesktop.Sdk.Extension.rust-stable//20.08
```

Then you go ahead and build Fractal.

```
flatpak-builder --user --install fractal flatpak/org.gnome.Fractal.json
```

### GNU/Linux

If you decide to ignore our recommendation and build on your host system,
outside of Flatpak, you will need Meson and Ninja (as well as Rust and Cargo).

```sh
meson . _build --prefix=/usr/local
ninja -C _build
sudo ninja -C _build install
```

### macOS

```sh
brew install gtk+3 dbus bash adwaita-icon-theme libhandy gtksourceview4 \
    gspell gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-editing-services
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

Full reference in: <https://matrix.org/docs/spec/client_server/r0.2.0.html#m-room-message-msgtypes>

## Frequently Asked Questions

* Does Fractal have encryption support? Will it ever?

Fractal does not currently have encryption support, but
there is an initiative for it.

We are now partially using matrix-rust-sdk rather than our own implementation. (See https://gitlab.gnome.org/GNOME/fractal/-/issues/636) and are working towards using it completely.

Code and further information for this module can be found at [matrix/matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk).

* Can I run Fractal with the window closed?

Currently Fractal does not support this. Fractal is a
GNOME application, and accordingly adheres GNOME
guidelines and paradigms. This will be revisited if or
when GNOME gets a "Do Not Disturb" feature.

## Code of Conduct

Fractal follows the official GNOME Foundation code of conduct. You can read it [here](/code-of-conduct.md).

