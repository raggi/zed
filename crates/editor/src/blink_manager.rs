use crate::EditorSettings;
use gpui::ModelContext;
use settings::Settings;
use settings::SettingsStore;
use smol::Timer;
use std::time::Duration;

pub struct BlinkManager {
    blink_interval: Duration,

    blink_epoch: usize,
    blinking_paused: bool,
    visible: bool,
    enabled: bool,
}

impl BlinkManager {
    pub fn new(blink_interval: Duration, model: &Model<Self>, cx: &mut AppContext) -> Self {
        // Make sure we blink the cursors if the setting is re-enabled
        cx.observe_global::<SettingsStore>(move |this, cx| {
            this.blink_cursors(this.blink_epoch, cx)
        })
        .detach();

        Self {
            blink_interval,

            blink_epoch: 0,
            blinking_paused: false,
            visible: true,
            enabled: false,
        }
    }

    fn next_blink_epoch(&mut self) -> usize {
        self.blink_epoch += 1;
        self.blink_epoch
    }

    pub fn pause_blinking(&mut self, model: &Model<Self>, cx: &mut AppContext) {
        self.show_cursor(cx);

        let epoch = self.next_blink_epoch();
        let interval = self.blink_interval;
        model
            .spawn(cx, |this, mut cx| async move {
                Timer::after(interval).await;
                this.update(&mut cx, |this, model, cx| {
                    this.resume_cursor_blinking(epoch, cx)
                })
            })
            .detach();
    }

    fn resume_cursor_blinking(&mut self, epoch: usize, model: &Model<Self>, cx: &mut AppContext) {
        if epoch == self.blink_epoch {
            self.blinking_paused = false;
            self.blink_cursors(epoch, model, cx);
        }
    }

    fn blink_cursors(&mut self, epoch: usize, model: &Model<Self>, cx: &mut AppContext) {
        if EditorSettings::get_global(cx).cursor_blink {
            if epoch == self.blink_epoch && self.enabled && !self.blinking_paused {
                self.visible = !self.visible;
                model.notify(cx);

                let epoch = self.next_blink_epoch();
                let interval = self.blink_interval;
                model
                    .spawn(cx, |this, mut cx| async move {
                        Timer::after(interval).await;
                        if let Some(this) = this.upgrade() {
                            this.update(&mut cx, |this, model, cx| this.blink_cursors(epoch, cx))
                                .ok();
                        }
                    })
                    .detach();
            }
        } else {
            self.show_cursor(cx);
        }
    }

    pub fn show_cursor(&mut self, model: &Model<Self>, cx: &mut AppContext) {
        if !self.visible {
            self.visible = true;
            model.notify(cx);
        }
    }

    pub fn enable(&mut self, model: &Model<Self>, cx: &mut AppContext) {
        if self.enabled {
            return;
        }

        self.enabled = true;
        // Set cursors as invisible and start blinking: this causes cursors
        // to be visible during the next render.
        self.visible = false;
        self.blink_cursors(self.blink_epoch, model, cx);
    }

    pub fn disable(&mut self, model: &Model<Self>, _cx: &mut AppContext) {
        self.visible = false;
        self.enabled = false;
    }

    pub fn visible(&self) -> bool {
        self.visible
    }
}
