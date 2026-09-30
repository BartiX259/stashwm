use super::layout::*;
use super::rebound::Edge;
use crate::protocol::*;
use crate::wm::ActionButton;
use crate::wm::StashArea;
use crate::wm::WMEngine;
use crate::wm::WMState;
use smithay::utils::{Logical, Point};

impl WMEngine {
    pub fn handle_pointer_motion(
        &mut self,
        pos: Point<i32, Logical>,
        time: std::time::Instant,
        effects: &mut Vec<BackendEffect>,
    ) -> EventResult {
        let under = self.window_at(pos);
        match &mut self.state {
            WMState::Normal { active_window } => {
                if let Some(under) = under {
                    if active_window.is_none() || under != active_window.clone().unwrap() {
                        self.normal_state_under_cursor(effects);
                    }
                }
                if let Some(edge) = self.rebound.on_motion(pos, self.screen_size, time) {
                    match edge {
                        Edge::Bottom => {
                            self.toggle_stash(effects);
                            return EventResult::consumed(std::mem::take(effects));
                        }
                        Edge::Left | Edge::Right => {
                            if let Some(target) = self.get_window_for_edge(edge, pos.y) {
                                self.select_window(target, effects);
                                return EventResult::consumed(std::mem::take(effects));
                            }
                        }
                        _ => {}
                    }
                }
                EventResult::forwarded(std::mem::take(effects))
            }
            WMState::WindowSelected { window, .. } => {
                let current_window = window.clone();
                let win_rect = self
                    .get_window_geometry(&current_window)
                    .unwrap_or_default();
                let buttons = get_action_buttons(win_rect);
                let mut new_button = None;
                for (action, rect) in buttons {
                    if rect.contains(pos) {
                        new_button = Some(action);
                        break;
                    }
                }
                let mut new_window = None;
                if new_button.is_none() {
                    for other in &self.visible_windows {
                        if *other != current_window {
                            if let Some(rect) = self.get_window_geometry(other) {
                                if rect.contains(pos) {
                                    new_window = Some(other.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
                if let WMState::WindowSelected {
                    hovered_window,
                    hovered_button,
                    ..
                } = &mut self.state
                {
                    *hovered_button = new_button;
                    *hovered_window = new_window;
                }

                EventResult::consumed(std::mem::take(effects))
            }
            WMState::StashOpened { mouse_area } => {
                let layout = calculate_stash_layout(self.screen_size, self.stashed_windows.len());

                *mouse_area = if layout.restore_all.contains(pos) {
                    StashArea::RestoreAll
                } else if layout.close_all.contains(pos) {
                    StashArea::CloseAll
                } else {
                    let mut found = None;
                    for (win, preview) in self.stashed_windows.iter().zip(layout.previews.iter()) {
                        if preview.close_button.contains(pos) {
                            found = Some(StashArea::PreviewClose(win.clone()));
                            break;
                        } else if preview.main_rect.contains(pos) {
                            found = Some(StashArea::Preview(win.clone()));
                            break;
                        }
                    }
                    if let Some(f) = found {
                        f
                    } else if layout.main_rect.contains(pos) {
                        StashArea::Background
                    } else {
                        StashArea::Outside
                    }
                };

                EventResult::consumed(std::mem::take(effects))
            }
        }
    }

    pub fn handle_pointer_button(
        &mut self,
        button: u32,
        pressed: bool,
        _pos: Point<i32, Logical>,
        _time: std::time::Instant,
        effects: &mut Vec<BackendEffect>,
    ) -> EventResult {
        if !pressed {
            return EventResult::forwarded(std::mem::take(effects));
        }

        // Right-click or extra buttons dismiss modals
        if button == 0x111 {
            if !matches!(self.state, WMState::Normal { .. }) {
                self.normal_state_under_cursor(effects);
                return EventResult::consumed(std::mem::take(effects));
            }
        }

        // Left Click (0x110)
        if button == 0x110 {
            match self.state.clone() {
                WMState::Normal { .. } => EventResult::forwarded(std::mem::take(effects)),
                WMState::WindowSelected {
                    window,
                    hovered_window,
                    hovered_button,
                } => {
                    if let Some(btn) = hovered_button {
                        match btn {
                            ActionButton::Close => self.close_window(window, effects),
                            ActionButton::Minimize => self.minimize_window(window, effects),
                            ActionButton::Maximize => self.maximize_window(window, effects),
                        }
                    } else if let Some(target) = hovered_window {
                        self.swap_windows(window, target, effects);
                    }

                    self.normal_state_under_cursor(effects);
                    EventResult::consumed(std::mem::take(effects))
                }
                WMState::StashOpened { mouse_area } => {
                    match mouse_area {
                        StashArea::Preview(window) => {
                            self.restore_stashed(window, effects);
                        }
                        StashArea::PreviewClose(window) => self.close_stashed(window, effects),
                        StashArea::RestoreAll => {
                            self.restore_all_stashed(effects);
                        }
                        StashArea::CloseAll => {
                            self.close_all_stashed(effects);
                        }
                        StashArea::Background => {}
                        StashArea::Outside => {
                            self.normal_state_under_cursor(effects);
                        }
                    }
                    self.rebound.reset();
                    EventResult::consumed(std::mem::take(effects))
                }
            }
        } else {
            EventResult::forwarded(std::mem::take(effects))
        }
    }
}
