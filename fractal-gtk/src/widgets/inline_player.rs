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


use gst::prelude::*;
use gst::ClockTime;
use gst_player;

use gtk;
use gtk::prelude::*;

// use gio::{File, FileExt};
use glib::SignalHandlerId;

use chrono::NaiveTime;
use failure::Error;
use fragile::Fragile;

use std::ops::Deref;
use std::rc::Rc;
// use std::path::Path;

trait PlayerExt {
    fn play(&self);
    fn pause(&self);
    fn stop(&self);
}

#[derive(Debug, Clone)]
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
        let seconds = duration.seconds().map(|v| v as f64).unwrap_or(0.0);

        self.slider.block_signal(&self.slider_update);
        self.slider.set_range(0.0, seconds);
        self.slider.unblock_signal(&self.slider_update);

        self.duration.set_text(&format_duration(seconds as u32));
    }

    /// Update the `gtk::SclaeBar` when the pipeline position is changed.
    fn on_position_updated(&self, position: Position) {
        let seconds = position.seconds().map(|v| v as f64).unwrap_or(0.0);

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

#[derive(Debug, Clone)]
struct PlayerControls {
    container: gtk::Box,
    play: gtk::Button,
    pause: gtk::Button,
}

#[derive(Debug, Clone)]
pub struct AudioPlayerWidget {
    pub container: gtk::Box,
    player: gst_player::Player,
    controls: PlayerControls,
    timer: PlayerTimes,
}

impl Default for AudioPlayerWidget {
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

        let builder = gtk::Builder::new_from_resource("/org/gnome/Fractal/ui/player_toolbar.ui");
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

        AudioPlayerWidget {
            container,
            player,
            controls,
            timer,
        }
    }
}

impl AudioPlayerWidget {
    pub fn new() -> Rc<Self> {
        let w = Rc::new(Self::default());
        Self::init(&w);
        w
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn init(s: &Rc<Self>) {
        Self::connect_control_buttons(s);
        Self::connect_gst_signals(s);
    }

    pub fn initialize_stream(&self) -> Result<(), Error> {
        unimplemented!()
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    /// Connect the `PlayerControls` buttons to the `PlayerExt` methods.
    fn connect_control_buttons(s: &Rc<Self>) {
        // Connect the play button to the gst Player.
        s.controls.play.connect_clicked(clone!(s => move |_| s.play()));

        // Connect the pause button to the gst Player.
        s.controls.pause.connect_clicked(clone!(s => move |_| s.pause()));
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn connect_gst_signals(s: &Rc<Self>) {
        // Log gst warnings.
        s.player.connect_warning(move |_, warn| warn!("gst warning: {}", warn));

        // Log gst errors.
        // This ideally will never occur.
        s.player.connect_error(move |_, err| error!("gst Error: {}", err));

        // The followign callbacks require `Send` but are handled by the gtk main loop
        let s2 = Fragile::new(s.clone());

        // Update the duration label and the slider
        s.player.connect_duration_changed(clone!(s2 => move |_, clock| {
            s2.get().timer.on_duration_changed(Duration(clock));
        }));

        // Update the position label and the slider
        s.player.connect_position_updated(clone!(s2 => move |_, clock| {
            s2.get().timer.on_position_updated(Position(clock));
        }));

        // Reset the slider to 0 and show a play button
        s.player.connect_end_of_stream(clone!(s2 => move |_| s2.get().stop()));
    }

    fn connect_update_slider(slider: &gtk::Scale, player: &gst_player::Player) -> SignalHandlerId {
        slider.connect_value_changed(clone!(player => move |slider| {
            let value = slider.get_value() as u64;
            player.seek(ClockTime::from_seconds(value as u64));
        }))
    }
}

impl PlayerExt for AudioPlayerWidget {
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
}
