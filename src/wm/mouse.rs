use super::events::*;
use super::layout::*;
use crate::wm::StashArea;
use crate::wm::WMEngine;
use crate::wm::WMState;
use smithay::utils::{Logical, Point, Size};

impl WMEngine {
    pub fn handle_pointer_motion(
        &mut self,
        pos: Point<i32, Logical>,
        time: std::time::Instant,
        effects: &mut Vec<WMEffect>,
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
                            self.state = WMState::StashOpened {
                                mouse_area: StashArea::Outside,
                            };
                            effects.push(WMEffect::SetFocus(None));
                            return EventResult::consumed(std::mem::take(effects));
                        }
                        Edge::Left | Edge::Right => {
                            if let Some(target) = self.get_window_for_edge(edge, pos.y) {
                                self.state = WMState::WindowSelected {
                                    window: target,
                                    hovered_window: None,
                                    hovered_button: None,
                                };
                                effects.push(WMEffect::SetFocus(None));
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
        effects: &mut Vec<WMEffect>,
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
                            ActionButton::Close => {
                                self.visible_windows.retain(|w| w != &window);
                                effects.push(WMEffect::CloseWindow(window));
                                self.recompute_layout(effects);
                            }
                            ActionButton::Minimize => {
                                self.visible_windows.retain(|w| w != &window);
                                self.stashed_windows.push(window.clone());
                                let size = Size::new(PREVIEW_SIZE.0 * 2, PREVIEW_SIZE.1 * 2);
                                effects.push(WMEffect::SetWindowSize {
                                    window: window.clone(),
                                    size,
                                });
                                effects.push(WMEffect::UnmapWindow(window));
                                self.recompute_layout(effects);
                            }
                            ActionButton::Maximize => {
                                let others: Vec<_> = self
                                    .visible_windows
                                    .drain(..)
                                    .filter(|w| w != &window)
                                    .collect();
                                for other in others {
                                    effects.push(WMEffect::UnmapWindow(other.clone()));
                                    self.stashed_windows.push(other);
                                }
                                self.visible_windows = vec![window.clone()];
                                self.recompute_layout(effects);
                                effects.push(WMEffect::SetFocus(Some(window)));
                            }
                        }
                    } else if let Some(target) = hovered_window {
                        // Swap positions
                        let idx1 = self.visible_windows.iter().position(|w| w == &window);
                        let idx2 = self.visible_windows.iter().position(|w| w == &target);
                        if let (Some(i1), Some(i2)) = (idx1, idx2) {
                            self.visible_windows.swap(i1, i2);
                            self.recompute_layout(effects);
                        }
                    }

                    self.normal_state_under_cursor(effects);
                    EventResult::consumed(std::mem::take(effects))
                }
                WMState::StashOpened { mouse_area } => {
                    match mouse_area {
                        StashArea::Preview(win) => {
                            self.stashed_windows.retain(|w| w != &win);
                            self.visible_windows.push(win.clone());
                            self.recompute_layout(effects);
                            effects.push(WMEffect::SetFocus(Some(win)));
                            if self.stashed_windows.is_empty() {
                                self.normal_state_under_cursor(effects);
                            }
                        }
                        StashArea::PreviewClose(win) => {
                            self.stashed_windows.retain(|w| w != &win);
                            effects.push(WMEffect::CloseWindow(win));
                            if self.stashed_windows.is_empty() {
                                self.normal_state_under_cursor(effects);
                            }
                        }
                        StashArea::RestoreAll => {
                            let all = std::mem::take(&mut self.stashed_windows);
                            self.visible_windows.extend(all);
                            self.recompute_layout(effects);
                            self.normal_state_under_cursor(effects);
                        }
                        StashArea::CloseAll => {
                            let all = std::mem::take(&mut self.stashed_windows);
                            for w in all {
                                effects.push(WMEffect::CloseWindow(w));
                            }
                            self.normal_state_under_cursor(effects);
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
