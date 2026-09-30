pub mod events;
pub mod layout;
pub mod rebound;
pub mod render;

use crate::backend::shell::WindowElement;
use events::*;
use layout::*;
use rebound::Rebound;
use smithay::utils::{Logical, Point, Rectangle, Size};

#[derive(Debug, Clone, PartialEq)]
pub enum StashArea {
    Outside,
    Background,
    Preview(WindowElement),
    PreviewClose(WindowElement),
    CloseAll,
    RestoreAll,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WMState {
    Normal {
        active_window: Option<WindowElement>,
    },
    WindowSelected {
        window: WindowElement,
        hovered_window: Option<WindowElement>,
        hovered_button: Option<ActionButton>,
    },
    StashOpened {
        mouse_area: StashArea,
    },
}

#[derive(Debug)]
pub struct WMEngine {
    pub screen_size: Size<i32, Logical>,
    pub visible_windows: Vec<WindowElement>,
    pub stashed_windows: Vec<WindowElement>,
    pub state: WMState,
    rebound: Rebound,
    last_pointer_pos: Point<i32, Logical>,
}

impl WMEngine {
    pub fn new(screen_size: Size<i32, Logical>) -> Self {
        Self {
            screen_size,
            visible_windows: Vec::new(),
            stashed_windows: Vec::new(),
            state: WMState::Normal {
                active_window: None,
            },
            rebound: Rebound::new(),
            last_pointer_pos: Point::new(0, 0),
        }
    }

    pub fn handle_event(&mut self, event: WMEvent) -> EventResult {
        let mut effects = Vec::new();

        match event {
            WMEvent::ScreenResized(size) => {
                self.screen_size = size;
                self.recompute_layout(&mut effects);
                EventResult::forwarded(effects)
            }
            WMEvent::WindowCreated(window) => {
                self.visible_windows.push(window.clone());
                self.recompute_layout(&mut effects);
                if let WMState::Normal { .. } = self.state {
                    self.normal_state_under_cursor(&mut effects);
                }
                EventResult::forwarded(effects)
            }
            WMEvent::WindowDestroyed(window) => {
                self.visible_windows.retain(|w| w != &window);
                self.stashed_windows.retain(|w| w != &window);
                self.recompute_layout(&mut effects);
                if let WMState::WindowSelected { window: sel, .. } = &self.state {
                    if sel == &window {
                        self.normal_state_under_cursor(&mut effects);
                    }
                } else if let WMState::Normal { .. } = self.state {
                    self.normal_state_under_cursor(&mut effects);
                }
                EventResult::forwarded(effects)
            }
            WMEvent::PointerMoved { pos, time } => {
                let pixel_pos = pos.to_i32_round();
                self.last_pointer_pos = pixel_pos;
                self.handle_pointer_motion(pixel_pos, time, &mut effects)
            }
            WMEvent::PointerButton {
                button,
                pressed,
                pos,
                time,
            } => {
                let pixel_pos = pos.to_i32_round();
                self.last_pointer_pos = pixel_pos;
                self.handle_pointer_button(button, pressed, pixel_pos, time, &mut effects)
            }
            WMEvent::KeyEscape => {
                if let WMState::Normal { .. } = self.state {
                    EventResult::forwarded(effects)
                } else {
                    self.normal_state_under_cursor(&mut effects);
                    EventResult::consumed(effects)
                }
            }
        }
    }

    fn recompute_layout(&self, effects: &mut Vec<WMEffect>) {
        let tiles = master_slave_layout(self.screen_size, self.visible_windows.len(), 10);
        for (win, tile) in self.visible_windows.iter().zip(tiles.into_iter()) {
            effects.push(WMEffect::MapWindow {
                window: win.clone(),
                loc: tile.loc,
            });
            effects.push(WMEffect::SetWindowSize {
                window: win.clone(),
                size: tile.size,
            });
        }
    }

    fn handle_pointer_motion(
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

    fn handle_pointer_button(
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

    fn get_window_for_edge(&self, edge: Edge, y: i32) -> Option<WindowElement> {
        if self.visible_windows.is_empty() {
            return None;
        }
        if edge == Edge::Left || self.visible_windows.len() == 1 {
            return self.visible_windows.first().cloned();
        }
        // Right edge: check stack slice
        let tiles = master_slave_layout(self.screen_size, self.visible_windows.len(), 10);
        for (win, tile) in self
            .visible_windows
            .iter()
            .skip(1)
            .zip(tiles.into_iter().skip(1))
        {
            if y >= tile.loc.y && y <= tile.loc.y + tile.size.h {
                return Some(win.clone());
            }
        }
        self.visible_windows.last().cloned()
    }

    fn get_window_geometry(&self, window: &WindowElement) -> Option<Rectangle<i32, Logical>> {
        let idx = self.visible_windows.iter().position(|w| w == window)?;
        let tiles = master_slave_layout(self.screen_size, self.visible_windows.len(), 10);
        let tile = tiles.get(idx)?;
        Some(Rectangle::new(tile.loc, tile.size))
    }

    pub fn window_at(&self, pos: Point<i32, Logical>) -> Option<WindowElement> {
        let tiles = master_slave_layout(self.screen_size, self.visible_windows.len(), 10);
        for (win, tile) in self.visible_windows.iter().zip(tiles.into_iter()) {
            let rect = Rectangle::new(tile.loc, tile.size);
            if rect.contains(pos) {
                return Some(win.clone());
            }
        }
        None
    }
    pub fn normal_state_under_cursor(&mut self, effects: &mut Vec<WMEffect>) {
        let under = self.window_at(self.last_pointer_pos);
        self.rebound.reset();
        self.state = WMState::Normal {
            active_window: under.clone(),
        };
        effects.push(WMEffect::SetFocus(under));
    }
}
