extern crate gdk;
extern crate gtk;
use self::gtk::prelude::*;
use self::gdk::FrameClockExt;

use glib;

use app::App;

impl App {
    pub fn connect_msg_scroll(&self) {
        let s = self.ui.builder
            .get_object::<gtk::ScrolledWindow>("messages_scroll")
            .expect("Can't find message_scroll in ui file.");
        let btn = self.ui.builder
            .get_object::<gtk::Button>("scroll_btn")
            .expect("Can't find scroll_btn in ui file.");
        let revealer = self.ui.builder
            .get_object::<gtk::Revealer>("scroll_btn_revealer")
            .expect("Can't find scroll_btn_revealer in ui file.");

        /*
        let op = self.op.clone();
        s.connect_edge_overshot(move |_, dir| if dir == gtk::PositionType::Top {
            op.lock().unwrap().load_more_messages();
        });
        */

        /* From clutter-easing.c, based on Robert Penner's
         * infamous easing equations, MIT license.
         */
        fn ease_out_cubic (t: f64) -> f64 {
            let p = t - 1f64;
            return p * p * p + 1f64;
        }

        fn scroll_down(ref view: &gtk::ScrolledWindow, animate: bool) {
            if let Some(adj) = view.get_vadjustment() {
                if animate {
                    if let Some(clock) = view.get_frame_clock() {
                        let duration = 200;
                        let start = adj.get_value();
                        let end = adj.get_upper() - adj.get_page_size();
                        let start_time = clock.get_frame_time();
                        let end_time = start_time + 1000 * duration;
                        view.add_tick_callback(move |_view, clock| {
                            let now = clock.get_frame_time();

                            if now < end_time && adj.get_value() != end {
                                let mut t = (now - start_time) as f64 / (end_time - start_time) as f64;
                                t = ease_out_cubic(t);
                                adj.set_value(start + t * (end - start));
                                return glib::Continue(true);
                            }
                            else
                            {
                                adj.set_value (end);
                                return glib::Continue(false);
                            }
                        });
                    }
                }
                else {
                    adj.set_value(adj.get_upper() - adj.get_page_size());
                }
            }
        }

        if let Some(adj) = s.get_vadjustment() {
            let op = self.op.clone();
            adj.connect_changed(clone!(s => move |_| {
                if op.lock().unwrap().autoscroll {
                    scroll_down(&s, false);
                }
            }));

            let op = self.op.clone();
            let r = revealer.clone();
            adj.connect_value_changed(move |adj| {
                let bottom = adj.get_upper() - adj.get_page_size();
                if adj.get_value() == bottom {
                    r.set_reveal_child(false);
                    op.lock().unwrap().autoscroll = true;
                } else {
                    r.set_reveal_child(true);
                    op.lock().unwrap().autoscroll = false;
                }
                /* the scroll view is getting close to the upper edge therefore loading more
                 * messages
                 * FIXME: Load more messages only when the user scrolles to the upper eage */

                if adj.get_value() < 100f64 {
                    op.lock().unwrap().load_more_messages();
                }
            });
        }

        btn.connect_clicked(move |_| {
            revealer.set_reveal_child(false);
            scroll_down(&s, true);
        });
    }
}
