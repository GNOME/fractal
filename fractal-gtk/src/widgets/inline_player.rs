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

use crate::backend::{media, ThreadPool};
use crate::clone;

use gst::prelude::*;
use gst::ClockTime;
use gstreamer_pbutils::Discoverer;
use log::{error, info, warn};

use gtk::prelude::*;
use gtk::ButtonExt;

// use gio::{File, FileExt};
use glib::source::Continue;
use glib::SignalHandlerId;

use chrono::NaiveTime;
use fragile::Fragile;

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use fractal_api::url::Url;

use crate::app::App;
use crate::error::Error;
use crate::i18n::i18n;

pub trait PlayerExt {
    fn play(&self);
    fn pause(&self);
    fn stop(&self);
    fn initialize_stream(
        player: &Rc<Self>,
        media_url: &str,
        server_url: &Url,
        thread_pool: ThreadPool,
        bx: &gtk::Box,
        start_playing: bool,
    );
    fn get_controls_container(player: &Rc<Self>) -> Option<gtk::Box>;
    fn get_player(player: &Rc<Self>) -> gst_player::Player;
    fn switch_mute_state(player: &Rc<Self>, button: &gtk::Button);
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

#[derive(Debug, Clone)]
struct PlayButtons {
    container: gtk::Box,
    play: gtk::Button,
    pause: gtk::Button,
}

#[derive(Debug, Clone)]
pub struct PlayerControls {
    container: gtk::Box,
    buttons: PlayButtons,
    timer: PlayerTimes,
}

pub trait MediaPlayer {
    fn get_player(&self) -> gst_player::Player;
    fn get_controls(&self) -> Option<PlayerControls>;
    fn get_local_path_access(&self) -> Rc<RefCell<Option<String>>>;
}

trait ControlsConnection {
    fn init(s: &Rc<Self>);
    fn connect_control_buttons(s: &Rc<Self>);
    fn connect_gst_signals(s: &Rc<Self>);
}

#[derive(Debug, Clone)]
pub struct AudioPlayerWidget {
    player: gst_player::Player,
    controls: PlayerControls,
    local_path: Rc<RefCell<Option<String>>>,
}

impl Default for AudioPlayerWidget {
    fn default() -> Self {
        let dispatcher = gst_player::PlayerGMainContextSignalDispatcher::new(None);
        let player = gst_player::Player::new(
            None,
            // Use the gtk main thread
            Some(&dispatcher.upcast::<gst_player::PlayerSignalDispatcher>()),
        );

        player.set_video_track_enabled(false);

        let mut config = player.get_config();
        config.set_position_update_interval(250);
        player.set_config(config).unwrap();

        // Log gst warnings.
        player.connect_warning(move |_, warn| warn!("gst warning: {}", warn));

        // Log gst errors.
        // This ideally will never occur.
        player.connect_error(move |_, err| error!("gst Error: {}", err));

        let controls = create_controls(&player);

        AudioPlayerWidget {
            player,
            controls,
            local_path: Rc::new(RefCell::new(None)),
        }
    }
}

impl AudioPlayerWidget {
    pub fn new() -> Rc<Self> {
        let w = Rc::new(Self::default());

        // When the widget is attached to a parent,
        // since it's a rust struct and not a widget the
        // compiler drops the refference to it at the end of
        // scope. That's cause we only attach the `self.controls.container`
        // to the parent.
        //
        // So this callback keeps a refference to the Rust Struct
        // so the compiler won't drop it which would cause to also drop
        // the `gst_player`.
        //
        // When the widget is detached from it's parent which happens
        // when we drop the room widget, this callback runs freeing
        // the last refference we were holding.
        let widget = RefCell::new(Some(w.clone()));
        w.controls.container.connect_remove(move |_, _| {
            widget.borrow_mut().take();
        });

        w
    }
}

impl MediaPlayer for AudioPlayerWidget {
    fn get_player(&self) -> gst_player::Player {
        self.player.clone()
    }

    fn get_controls(&self) -> Option<PlayerControls> {
        Some(self.controls.clone())
    }

    fn get_local_path_access(&self) -> Rc<RefCell<Option<String>>> {
        self.local_path.clone()
    }
}

#[derive(Debug, Clone)]
pub struct VideoPlayerWidget {
    player: gst_player::Player,
    controls: Option<PlayerControls>,
    local_path: Rc<RefCell<Option<String>>>,
    dimensions: Rc<RefCell<Option<(i32, i32)>>>,
    state: Rc<RefCell<Option<gst_player::PlayerState>>>,
}

impl Default for VideoPlayerWidget {
    fn default() -> Self {
        let dispatcher = gst_player::PlayerGMainContextSignalDispatcher::new(None);
        let sink = gst::ElementFactory::make("gtksink", None)
            .expect("Missing dependency: element gtksink is needed (usually, in gstreamer-plugins-good or in gst-plugin-gtk).");
        let renderer = gst_player::PlayerVideoOverlayVideoRenderer::new_with_sink(&sink).upcast();
        let player = gst_player::Player::new(
            Some(&renderer),
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

        VideoPlayerWidget {
            player,
            controls: None,
            local_path: Rc::new(RefCell::new(None)),
            dimensions: Rc::new(RefCell::new(None)),
            state: Rc::new(RefCell::new(None)),
        }
    }
}

impl VideoPlayerWidget {
    pub fn new(with_controls: bool) -> Rc<Self> {
        let mut player_widget = Self::default();

        if with_controls {
            let controls = create_controls(&player_widget.player);
            player_widget.controls = Some(controls);
        }

        let w = Rc::new(player_widget);

        /* The followign callbacks require `Send` but is handled by the gtk main loop */
        let player_weak = Fragile::new(Rc::downgrade(&w));
        w.player.connect_state_changed(move |_, state| {
            if let Some(player) = player_weak.get().upgrade() {
                *player.state.borrow_mut() = Some(state);
            }
        });
        let dimensions_weak = Fragile::new(Rc::downgrade(&w.dimensions));
        w.player
            .connect_video_dimensions_changed(move |_, video_width, video_height| {
                if let Some(dimensions) = dimensions_weak.get().upgrade() {
                    *dimensions.borrow_mut() = Some((video_width, video_height));
                }
            });

        w
    }

    pub fn get_video_widget(&self) -> gtk::Widget {
        let pipeline = self.player.get_pipeline();
        pipeline
            .get_property("video-sink")
            .unwrap()
            .get::<gst::Element>()
            .expect("The player of a VideoPlayerWidget should not use the default sink.")
            .unwrap()
            .get_property("widget")
            .unwrap()
            .get::<gtk::Widget>()
            .unwrap()
            .unwrap()
    }

    pub fn is_playing(&self) -> bool {
        if let Some(state) = *self.state.borrow() {
            match state {
                gst_player::PlayerState::Playing => true,
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn auto_adjust_video_dimensions(player_widget: &Rc<Self>) {
        /* The followign callback requires `Send` but is handled by the gtk main loop */
        let player_weak = Fragile::new(Rc::downgrade(&player_widget));
        player_widget.player.connect_video_dimensions_changed(
            move |_, video_width, video_height| {
                if video_width != 0 {
                    if let Some(player) = player_weak.get().upgrade() {
                        let widget = player.get_video_widget();
                        let allocated_width = widget.get_allocated_width();
                        let adjusted_height = allocated_width * video_height / video_width;
                        widget.set_size_request(-1, adjusted_height);
                    }
                }
            },
        );
        let player_weak = Rc::downgrade(&player_widget);
        player_widget
            .get_video_widget()
            .connect_size_allocate(move |_, allocation| {
                if let Some(player) = player_weak.upgrade() {
                    if let Some((video_width, video_height)) = *player.dimensions.borrow() {
                        if video_width != 0
                            && allocation.height * video_width != allocation.width * video_height
                        {
                            let adjusted_height = allocation.width * video_height / video_width;
                            player
                                .get_video_widget()
                                .set_size_request(-1, adjusted_height);
                        }
                    }
                }
            });

        /* Sometimes, set_size_request() doesn't get captured visually. The following timeout takes care of that. */
        let player_weak = Rc::downgrade(&player_widget);
        gtk::timeout_add_seconds(1, move || {
            if let Some(player) = player_weak.upgrade() {
                let (_, height) = player.get_video_widget().get_size_request();
                player.get_video_widget().set_size_request(-1, height - 1);
                player.get_video_widget().set_size_request(-1, height);
            }
            Continue(true)
        });
    }

    pub fn auto_adjust_box_size_to_video_dimensions(
        bx: &gtk::Box,
        player: &Rc<VideoPlayerWidget>,
    ) -> (glib::SignalHandlerId, glib::SignalHandlerId) {
        /* When gtk allocates a different size to the video widget than its minimal preferred size
        (set by set_size_request()), the method auto_adjust_video_dimensions() does not have any effect.
        When that happens and furthermore, the video widget is embedded in a vertically oriented box,
        this function here can be called. Here, the widget's height gets adjusted as a consequence of
        adjusting the distance between the top/bottom of the widget and the top/bottom of the box,
        rather than through the widget's preferred height. */

        /* The following callback requires `Send` but is handled by the gtk main loop */
        let bx_weak = Fragile::new(bx.downgrade());
        let dimension_id =
            player
                .player
                .connect_video_dimensions_changed(move |_, video_width, video_height| {
                    if let Some(bx) = bx_weak.get().upgrade() {
                        adjust_box_margins_to_video_dimensions(&bx, video_width, video_height);
                    }
                });
        let player_weak = Rc::downgrade(player);
        let size_id = bx.connect_size_allocate(move |bx, _| {
            if let Some(player) = player_weak.upgrade() {
                if let Some((video_width, video_height)) = *player.dimensions.borrow() {
                    /* The timeout is necessary for the edge cases, i.e. when resizing to minimum width or height.
                    When approaching the minimum fast, the last connect_size_allocate signal gets emitted before
                    reaching the minimum size. So without timeout, the values used to adjust the the video size
                    are bigger than they should be. */
                    gtk::timeout_add(
                        50,
                        clone!(bx, video_width, video_height => move || {
                            adjust_box_margins_to_video_dimensions(&bx, video_width, video_height);
                            Continue(false)
                        }),
                    );
                }
            }
        });
        (dimension_id, size_id)
    }

    /* As soon as there's an implementation for that in gst::Player, we should take that one instead. */
    pub fn play_in_loop(&self) -> SignalHandlerId {
        self.player.set_mute(true);
        self.player.play();
        self.player.connect_end_of_stream(|player| {
            player.play();
        })
    }

    pub fn stop_loop(&self, id: SignalHandlerId) {
        self.player.set_mute(false);
        self.player.stop();
        self.player.disconnect(id);
    }

    pub fn switch_play_pause_state(player: &Rc<Self>) {
        match *player.state.borrow() {
            Some(gst_player::PlayerState::Paused) => {
                player.play();
            }
            _ => {
                player.pause();
            }
        }
    }
}

impl PartialEq for VideoPlayerWidget {
    fn eq(&self, other: &Self) -> bool {
        self.player == other.player
    }
}

impl MediaPlayer for VideoPlayerWidget {
    fn get_player(&self) -> gst_player::Player {
        self.player.clone()
    }

    fn get_controls(&self) -> Option<PlayerControls> {
        self.controls.clone()
    }

    fn get_local_path_access(&self) -> Rc<RefCell<Option<String>>> {
        self.local_path.clone()
    }
}

impl<T: MediaPlayer + 'static> PlayerExt for T {
    fn play(&self) {
        if let Some(controls) = self.get_controls() {
            controls.buttons.pause.show();
            controls.buttons.play.hide();
        }
        self.get_player().play();
    }

    fn pause(&self) {
        if let Some(controls) = self.get_controls() {
            controls.buttons.pause.hide();
            controls.buttons.play.show();
        }
        self.get_player().pause();
    }

    #[rustfmt::skip]
    fn stop(&self) {
        if let Some(controls) = self.get_controls() {
            controls.buttons.pause.hide();
            controls.buttons.play.show();
            // Reset the slider position to 0
            controls.timer.on_position_updated(Position(ClockTime::from_seconds(0)));
        }

        self.get_player().stop();
    }

    fn initialize_stream(
        player: &Rc<Self>,
        media_url: &str,
        server_url: &Url,
        thread_pool: ThreadPool,
        bx: &gtk::Box,
        start_playing: bool,
    ) {
        bx.set_opacity(0.3);
        let (tx, rx): (
            Sender<Result<String, Error>>,
            Receiver<Result<String, Error>>,
        ) = channel();
        media::get_media_async(thread_pool, server_url.clone(), media_url.to_string(), tx);
        let local_path = player.get_local_path_access();
        gtk::timeout_add(
            50,
            clone!(player, bx => move || {
                match rx.try_recv() {
                    Err(TryRecvError::Empty) => Continue(true),
                    Err(TryRecvError::Disconnected) => {
                        let msg = i18n("Could not retrieve file URI");
                        /* FIXME: don't use APPOP! */
                        APPOP!(show_error, (msg));
                        Continue(true)
                    },
                    Ok(Ok(path)) => {
                        info!("MEDIA PATH: {}", &path);
                        *local_path.borrow_mut() = Some(path.clone());
                        if ! start_playing {
                            if let Some(controls) = player.get_controls() {
                                if let Ok(duration) = get_media_duration(&path) {
                                    controls.timer.on_duration_changed(Duration(duration))
                                }
                            }
                        }
                        let uri = format!("file://{}", path);
                        player.get_player().set_uri(&uri);
                        if player.get_controls().is_some() {
                            ControlsConnection::init(&player);
                        }
                        bx.set_opacity(1.0);
                        if start_playing {
                            player.play();
                        }
                        Continue(false)
                    }
                    Ok(Err(err)) => {
                        error!("Media path could not be found due to error: {:?}", err);
                        Continue(false)
                    }
                }
            }),
        );
    }

    fn get_controls_container(player: &Rc<Self>) -> Option<gtk::Box> {
        player.get_controls().map(|controls| controls.container)
    }

    fn get_player(player: &Rc<Self>) -> gst_player::Player {
        player.get_player()
    }

    fn switch_mute_state(player_widget: &Rc<Self>, button: &gtk::Button) {
        let player = player_widget.get_player();
        if player.get_mute() {
            player.set_mute(false);
            let image = gtk::Image::new_from_icon_name(
                Some("audio-volume-high-symbolic"),
                gtk::IconSize::Button,
            );
            button.set_image(Some(&image));
        } else {
            player.set_mute(true);
            let image = gtk::Image::new_from_icon_name(
                Some("audio-volume-muted-symbolic"),
                gtk::IconSize::Button,
            );
            button.set_image(Some(&image));
        }
    }
}

impl<T: MediaPlayer + 'static> ControlsConnection for T {
    #[rustfmt::skip]
    fn init(s: &Rc<Self>) {
        Self::connect_control_buttons(s);
        Self::connect_gst_signals(s);
    }
    #[rustfmt::skip]
    /// Connect the `PlayerControls` buttons to the `PlayerEssentials` methods.
    fn connect_control_buttons(s: &Rc<Self>) {
        if s.get_controls().is_some() {
            let weak = Rc::downgrade(s);

            // Connect the play button to the gst Player.
            s.get_controls().unwrap().buttons.play.connect_clicked(clone!(weak => move |_| {
                if let Some(p) = weak.upgrade() { p.play() }
            }));

            // Connect the pause button to the gst Player.
            s.get_controls().unwrap().buttons.pause.connect_clicked(clone!(weak => move |_| {
                if let Some(p) = weak.upgrade() { p.pause() }
            }));
        }
    }
    #[rustfmt::skip]
    fn connect_gst_signals(s: &Rc<Self>) {
        if s.get_controls().is_some() {
            // The followign callbacks require `Send` but are handled by the gtk main loop
            let weak = Fragile::new(Rc::downgrade(s));

            // Update the duration label and the slider
            s.get_player().connect_duration_changed(clone!(weak => move |_, clock| {
                if let Some(p) = weak.get().upgrade() { p.get_controls().unwrap().timer.on_duration_changed(Duration(clock)) }
            }));

            // Update the position label and the slider
            s.get_player().connect_position_updated(clone!(weak => move |_, clock| {
                if let Some(p) = weak.get().upgrade() { p.get_controls().unwrap().timer.on_position_updated(Position(clock)) }
            }));

            // Reset the slider to 0 and show a play button
            s.get_player().connect_end_of_stream(clone!(weak => move |_| {
                if let Some(p) = weak.get().upgrade() { p.stop() }
            }));
        }
    }
}

fn create_controls(player: &gst_player::Player) -> PlayerControls {
    let builder = gtk::Builder::new_from_resource("/org/gnome/Fractal/ui/audio_player.ui");
    let container = builder.get_object("container").unwrap();

    let buttons_container = builder.get_object("buttons").unwrap();
    let play = builder.get_object("play_button").unwrap();
    let pause = builder.get_object("pause_button").unwrap();

    let buttons = PlayButtons {
        container: buttons_container,
        play,
        pause,
    };

    let timer_container = builder.get_object("timer").unwrap();
    let progressed = builder.get_object("progress_time_label").unwrap();
    let duration = builder.get_object("total_duration_label").unwrap();
    let slider: gtk::Scale = builder.get_object("seek").unwrap();
    slider.set_range(0.0, 1.0);
    let slider_update = Rc::new(connect_update_slider(&slider, player));
    let timer = PlayerTimes {
        container: timer_container,
        progressed,
        duration,
        slider,
        slider_update,
    };
    PlayerControls {
        container,
        buttons,
        timer,
    }
}

fn connect_update_slider(slider: &gtk::Scale, player: &gst_player::Player) -> SignalHandlerId {
    slider.connect_value_changed(clone!(player => move |slider| {
        let value = slider.get_value() as u64;
        player.seek(ClockTime::from_seconds(value));
    }))
}

fn adjust_box_margins_to_video_dimensions(bx: &gtk::Box, video_width: i32, video_height: i32) {
    if let Some(parent) = bx.get_parent() {
        let parent_height = parent.get_allocated_height();
        let parent_width = parent.get_allocated_width();
        if video_height * parent_width >= parent_height * video_width
            && parent_height < video_height
        {
            if video_height != 0 {
                let adjusted_width = if parent_height < video_height {
                    let box_height = bx.get_allocated_height();
                    box_height * video_width / video_height
                } else {
                    video_width
                };
                let margin = (parent_width - adjusted_width) / 2;
                bx.set_margin_start(margin);
                bx.set_margin_end(margin);
                bx.set_margin_top(0);
                bx.set_margin_bottom(0);
            }
        } else if video_width != 0 {
            let adjusted_height = if parent_width < video_width {
                let box_width = bx.get_allocated_width();
                box_width * video_height / video_width
            } else {
                video_height
            };
            let margin = (parent_height - adjusted_height) / 2;
            bx.set_margin_top(margin);
            bx.set_margin_bottom(margin);
            bx.set_margin_start(0);
            bx.set_margin_end(0);
        }
    }
}

pub fn get_media_duration(file: &str) -> Result<ClockTime, glib::Error> {
    let timeout = ClockTime::from_seconds(1);
    let discoverer = Discoverer::new(timeout)?;
    let info = discoverer.discover_uri(&format!("file://{}", file))?;
    Ok(info.get_duration())
}
