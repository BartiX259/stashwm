use crate::backend::shell::WindowElement;
use keyframe::{ease, functions::EaseOutCubic};
use smithay::utils::{Logical, Point, Rectangle, Size};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct WindowTransition {
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
    start_time: Instant,
    duration: Duration,
}

impl WindowTransition {
    fn current_rect(&self, now: Instant) -> Rectangle<i32, Logical> {
        let elapsed = now.saturating_duration_since(self.start_time);
        let progress = (elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0);

        let x = ease(
            EaseOutCubic,
            self.from.loc.x as f32,
            self.to.loc.x as f32,
            progress,
        ) as i32;
        let y = ease(
            EaseOutCubic,
            self.from.loc.y as f32,
            self.to.loc.y as f32,
            progress,
        ) as i32;
        let w = ease(
            EaseOutCubic,
            self.from.size.w as f32,
            self.to.size.w as f32,
            progress,
        ) as i32;
        let h = ease(
            EaseOutCubic,
            self.from.size.h as f32,
            self.to.size.h as f32,
            progress,
        ) as i32;

        Rectangle::new(Point::new(x, y), Size::new(w.max(1), h.max(1)))
    }

    fn is_finished(&self, now: Instant) -> bool {
        now >= self.start_time + self.duration
    }
}

#[derive(Debug, Clone)]
struct StashTransition {
    from: f32,
    to: f32,
    start_time: Instant,
    duration: Duration,
}

impl StashTransition {
    fn current_progress(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.start_time);
        let progress = (elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0);
        ease(EaseOutCubic, self.from, self.to, progress)
    }

    fn is_finished(&self, now: Instant) -> bool {
        now >= self.start_time + self.duration
    }
}

pub struct AnimationManager {
    window_transitions: Vec<(WindowElement, WindowTransition)>,
    exiting_transitions: Vec<(WindowElement, WindowTransition)>,
    stash_transition: Option<StashTransition>,
    stash_open: bool,
    window_duration: Duration,
    stash_duration: Duration,
}

impl std::fmt::Debug for AnimationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationManager").finish()
    }
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            window_transitions: Vec::new(),
            exiting_transitions: Vec::new(),
            stash_transition: None,
            stash_open: false,
            window_duration: Duration::from_millis(180),
            stash_duration: Duration::from_millis(200),
        }
    }

    pub fn set_origin(&mut self, win: WindowElement, origin: Rectangle<i32, Logical>) {
        self.window_transitions.retain(|(w, _)| w != &win);
        self.window_transitions.push((
            win,
            WindowTransition {
                from: origin,
                to: origin,
                start_time: Instant::now(),
                duration: self.window_duration,
            },
        ));
    }

    pub fn sync_layout(
        &mut self,
        old_windows: &[WindowElement],
        old_rects: &[Rectangle<i32, Logical>],
        new_windows: &[WindowElement],
        new_rects: &[Rectangle<i32, Logical>],
    ) {
        let now = Instant::now();

        for (win, &target_rect) in new_windows.iter().zip(new_rects.iter()) {
            let start_rect =
                if let Some((_, trans)) = self.window_transitions.iter().find(|(w, _)| w == win) {
                    trans.current_rect(now)
                } else if let Some(old_pos) = old_windows.iter().position(|w| w == win) {
                    old_rects.get(old_pos).copied().unwrap_or(target_rect)
                } else {
                    let inset_w = target_rect.size.w * 15 / 100;
                    let inset_h = target_rect.size.h * 15 / 100;
                    Rectangle::new(
                        Point::new(
                            target_rect.loc.x + inset_w / 2,
                            target_rect.loc.y + inset_h / 2,
                        ),
                        Size::new(target_rect.size.w - inset_w, target_rect.size.h - inset_h),
                    )
                };

            self.window_transitions.retain(|(w, _)| w != win);

            if start_rect != target_rect {
                self.window_transitions.push((
                    win.clone(),
                    WindowTransition {
                        from: start_rect,
                        to: target_rect,
                        start_time: now,
                        duration: self.window_duration,
                    },
                ));
            }
        }

        self.window_transitions
            .retain(|(w, _)| new_windows.contains(w));
    }

    pub fn get_animating_rect(&mut self, win: &WindowElement) -> Option<Rectangle<i32, Logical>> {
        let pos = self.window_transitions.iter().position(|(w, _)| w == win)?;
        let now = Instant::now();

        if self.window_transitions[pos].1.is_finished(now) {
            self.window_transitions.swap_remove(pos);
            None
        } else {
            Some(self.window_transitions[pos].1.current_rect(now))
        }
    }

    pub fn is_animating(&self, win: &WindowElement) -> bool {
        self.window_transitions.iter().any(|(w, _)| w == win)
    }

    pub fn start_exiting(
        &mut self,
        win: WindowElement,
        from: Rectangle<i32, Logical>,
        to: Rectangle<i32, Logical>,
    ) {
        self.exiting_transitions.retain(|(w, _)| w != &win);
        self.exiting_transitions.push((
            win,
            WindowTransition {
                from,
                to,
                start_time: Instant::now(),
                duration: self.window_duration,
            },
        ));
    }

    pub fn get_exiting_rects(&mut self) -> Vec<(WindowElement, Rectangle<i32, Logical>)> {
        let now = Instant::now();
        self.exiting_transitions
            .retain(|(_, t)| !t.is_finished(now));
        self.exiting_transitions
            .iter()
            .map(|(w, t)| (w.clone(), t.current_rect(now)))
            .collect()
    }

    pub fn open_stash(&mut self) {
        let now = Instant::now();
        let current = self.stash_progress();
        self.stash_transition = Some(StashTransition {
            from: current,
            to: 1.0,
            start_time: now,
            duration: self.stash_duration,
        });
        self.stash_open = true;
    }

    pub fn close_stash(&mut self) {
        let now = Instant::now();
        let current = self.stash_progress();
        self.stash_transition = Some(StashTransition {
            from: current,
            to: 0.0,
            start_time: now,
            duration: self.stash_duration,
        });
        self.stash_open = false;
    }

    pub fn stash_progress(&self) -> f32 {
        let now = Instant::now();
        if let Some(ref trans) = self.stash_transition {
            if trans.is_finished(now) {
                trans.to
            } else {
                trans.current_progress(now)
            }
        } else if self.stash_open {
            1.0
        } else {
            0.0
        }
    }
}
