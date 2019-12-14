// inline_player.rs
//
// Copyright 2018 Jordan Petridis <jordanpetridis@protonmail.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

use fractal_api::clone;
use gst::prelude::*;
use gst::ClockTime;
use gst_player;
use log::{error, info, warn};

use gtk;
use gtk::prelude::*;

// use gio::{File, FileExt};
use glib::SignalHandlerId;

use chrono::NaiveTime;
use fragile::Fragile;

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use url::Url;

use crate::app::App;
use crate::backend::BKCommand;
use crate::i18n::i18n;

pub trait PlayerExt {
    fn play(&self);
    fn pause(&self);
    fn stop(&self);
    fn set_uri(&self, uri: &str);
}

#[derive(Debug, Clone)]
pub enum MediaType {
    Audio,
    Video,
}

#[derive(Debug, Clone, PartialEq)]
struct PlayerTimes {
    container: gtk::Box,
    progressed: gtk::Label,
    duration: gtk::Label,
    slider: gtk::Scale,
    slider_update: Rc<SignalHandlerId>,
}

#[derive(Debug, Clone, Copy)]
struct Duration(ClockTime);

impl Deref for Duration {
    type Target = ClockTime;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Position(ClockTime);

impl Deref for Position {
    type Target = ClockTime;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PlayerTimes {
    /// Update the duration `gtk::Label` and the max range of the `gtk::SclaeBar`.
    fn on_duration_changed(&self, duration: Duration) {
        let seconds = duration.seconds().map(|v| v as f64).unwrap_or_default();

        self.slider.block_signal(&self.slider_update);
        self.slider.set_range(0.0, seconds);
        self.slider.unblock_signal(&self.slider_update);

        self.duration.set_text(&format_duration(seconds as u32));
    }

    /// Update the `gtk::SclaeBar` when the pipeline position is changed.
    fn on_position_updated(&self, position: Position) {
        let seconds = position.seconds().map(|v| v as f64).unwrap_or_default();

        self.slider.block_signal(&self.slider_update);
        self.slider.set_value(seconds);
        self.slider.unblock_signal(&self.slider_update);

        self.progressed.set_text(&format_duration(seconds as u32));
    }
}

fn format_duration(seconds: u32) -> String {
    let time = NaiveTime::from_num_seconds_from_midnight(seconds, 0);

    if seconds >= 3600 {
        time.format("%T").to_string()
    } else {
        time.format("%M:%S").to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PlayerControls {
    container: gtk::Box,
    play: gtk::Button,
    pause: gtk::Button,
}

#[derive(Debug, Clone)]
pub struct MediaPlayerWidget {
    pub container: gtk::Box,
    player: gst_player::Player,
    controls: PlayerControls,
    timer: PlayerTimes,
    pub local_path: Rc<RefCell<Option<String>>>,
}

impl PartialEq for MediaPlayerWidget {
    fn eq(&self, other: &Self) -> bool {
        self.player == other.player
    }
}

impl Default for MediaPlayerWidget {
    fn default() -> Self {
        let dispatcher = gst_player::PlayerGMainContextSignalDispatcher::new(None);
        let player = gst_player::Player::new(
            None,
            // Use the gtk main thread
            Some(&dispatcher.upcast::<gst_player::PlayerSignalDispatcher>()),
        );

        let mut config = player.get_config();
        config.set_position_update_interval(250);
        player.set_config(config).unwrap();

        // Log gst warnings.
        player.connect_warning(move |_, warn| warn!("gst warning: {}", warn));

        // Log gst errors.
        // This ideally will never occur.
        player.connect_error(move |_, err| error!("gst Error: {}", err));

        let builder = gtk::Builder::new_from_resource("/org/gnome/Fractal/ui/audio_player.ui");
        let container = builder.get_object("container").unwrap();

        let buttons = builder.get_object("buttons").unwrap();
        let play = builder.get_object("play_button").unwrap();
        let pause = builder.get_object("pause_button").unwrap();

        let controls = PlayerControls {
            container: buttons,
            play,
            pause,
        };

        let timer_container = builder.get_object("timer").unwrap();
        let progressed = builder.get_object("progress_time_label").unwrap();
        let duration = builder.get_object("total_duration_label").unwrap();
        let slider: gtk::Scale = builder.get_object("seek").unwrap();
        slider.set_range(0.0, 1.0);
        let slider_update = Rc::new(Self::connect_update_slider(&slider, &player));
        let timer = PlayerTimes {
            container: timer_container,
            progressed,
            duration,
            slider,
            slider_update,
        };

        MediaPlayerWidget {
            container,
            player,
            controls,
            timer,
            local_path: Rc::new(RefCell::new(None)),
        }
    }
}

impl MediaPlayerWidget {
    pub fn new(media_type: MediaType) -> Rc<Self> {
        let player_widget = Self::default();

        match media_type {
            MediaType::Video => {
                let sink = gst::ElementFactory::make("gtksink", None).unwrap();
                let pipeline = player_widget.player.get_pipeline();
                pipeline.set_property("video-sink", &sink).unwrap();
            }
            MediaType::Audio => player_widget.player.set_video_track_enabled(false),
        }

        let w = Rc::new(player_widget);

        // When the widget is attached to a parent,
        // since it's a rust struct and not a widget the
        // compiler drops the refference to it at the end of
        // scope. That's cause we only attach the `self.container`
        // to the parent.
        //
        // So this callback keeps a refference to the Rust Struct
        // so the compiler won't drop it which would cause to also drop
        // the `gst_player`.
        //
        // When the widget is detached from it's parent which happens
        // when we drop the room widget, this callback runs freeing
        // the last refference we were holding.
        let foo = RefCell::new(Some(w.clone()));
        w.container.connect_remove(move |_, _| {
            foo.borrow_mut().take();
        });

        w
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub fn init(s: &Rc<Self>) {
        Self::connect_control_buttons(s);
        Self::connect_gst_signals(s);
    }

    pub fn initialize_stream(&self, uri: &str) {
        self.set_uri(uri)
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    /// Connect the `PlayerControls` buttons to the `PlayerExt` methods.
    fn connect_control_buttons(s: &Rc<Self>) {
        let weak = Rc::downgrade(s);

        // Connect the play button to the gst Player.
        s.controls.play.connect_clicked(clone!(weak => move |_| {
            weak.upgrade().map(|p| p.play());
        }));

        // Connect the pause button to the gst Player.
        s.controls.pause.connect_clicked(clone!(weak => move |_| {
            weak.upgrade().map(|p| p.pause());
        }));
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn connect_gst_signals(s: &Rc<Self>) {
        // The followign callbacks require `Send` but are handled by the gtk main loop
        let weak = Fragile::new(Rc::downgrade(s));

        // Update the duration label and the slider
        s.player.connect_duration_changed(clone!(weak => move |_, clock| {
            weak.get().upgrade().map(|p| p.timer.on_duration_changed(Duration(clock)));
        }));

        // Update the position label and the slider
        s.player.connect_position_updated(clone!(weak => move |_, clock| {
            weak.get().upgrade().map(|p| p.timer.on_position_updated(Position(clock)));
        }));

        // Reset the slider to 0 and show a play button
        s.player.connect_end_of_stream(clone!(weak => move |_| {
            weak.get().upgrade().map(|p| p.stop());
        }));
    }

    fn connect_update_slider(slider: &gtk::Scale, player: &gst_player::Player) -> SignalHandlerId {
        slider.connect_value_changed(clone!(player => move |slider| {
            let value = slider.get_value() as u64;
            player.seek(ClockTime::from_seconds(value));
        }))
    }

    pub fn backend_set_up(
        player: &Rc<Self>,
        backend: &Sender<BKCommand>,
        media_url: String,
        server_url: &Url,
        bx: &gtk::Box,
    ) {
        bx.set_opacity(0.3);
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        backend
            .send(BKCommand::GetMediaAsync(
                server_url.clone(),
                media_url.clone(),
                tx,
            ))
            .unwrap();
        let local_path = player.local_path.clone();
        gtk::timeout_add(
            50,
            clone!(player, bx => move || {
                match rx.try_recv() {
                    Err(TryRecvError::Empty) => gtk::Continue(true),
                    Err(TryRecvError::Disconnected) => {
                        let msg = i18n("Could not retrieve file URI");
                        /* FIXME: don't use APPOP! */
                        APPOP!(show_error, (msg));
                        gtk::Continue(true)
                    },
                    Ok(path) => {
                        info!("MEDIA PATH: {}", &path);
                        *local_path.borrow_mut() = Some(path.clone());
                        let uri = format!("file://{}", path);
                        player.initialize_stream(&uri);
                        MediaPlayerWidget::init(&player);
                        bx.set_opacity(1.0);
                        gtk::Continue(false)
                    }
                }
            }),
        );
    }

    pub fn create_control_box(&self) -> gtk::Box {
        let control_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        control_box.pack_start(&self.container, false, true, 0);
        control_box
    }

    pub fn get_video_widget(&self) -> Option<gtk::Widget> {
        let pipeline = self.player.get_pipeline();
        pipeline
            .get_property("video-sink")
            .unwrap()
            .get::<gst::Element>()?
            .get_property("widget")
            .unwrap()
            .get::<gtk::Widget>()
    }

    pub fn play_in_loop(&self) -> SignalHandlerId {
        self.player.set_mute(true);
        self.player.play();
        self.player.connect_end_of_stream(|player| {
            player.stop();
            player.play();
        })
    }

    pub fn stop_loop(&self, id: SignalHandlerId) {
        self.player.set_mute(false);
        self.player.stop();
        self.player.disconnect(id);
    }

    pub fn get_player(&self) -> gst_player::Player {
        self.player.clone()
    }
}

impl PlayerExt for MediaPlayerWidget {
    fn play(&self) {
        self.controls.pause.show();
        self.controls.play.hide();

        self.player.play();
    }

    fn pause(&self) {
        self.controls.pause.hide();
        self.controls.play.show();

        self.player.pause();
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn stop(&self) {
        self.controls.pause.hide();
        self.controls.play.show();

        self.player.stop();

        // Reset the slider position to 0
        self.timer.on_position_updated(Position(ClockTime::from_seconds(0)));
    }

    fn set_uri(&self, uri: &str) {
        self.player.set_uri(uri)
    }
}
