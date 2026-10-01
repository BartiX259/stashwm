pub mod actions;
pub mod keyboard;
pub mod layout;
pub mod mouse;
pub mod rebound;
pub mod render;

use crate::protocol::*;
use crate::wm::actions::WMAction;
use crate::{backend::shell::WindowElement, wm::rebound::Edge};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionButton {
    Minimize,
    Maximize,
    Close,
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

    pub fn handle_event(&mut self, event: BackendEvent) -> EventResult {
        let mut effects = Vec::new();

        match event {
            BackendEvent::ScreenResized(size) => {
                self.screen_size = size;
                self.recompute_layout(&mut effects);
                EventResult::forwarded(effects)
            }
            BackendEvent::WindowCreated(window) => {
                self.visible_windows.push(window.clone());
                self.recompute_layout(&mut effects);
                if let WMState::Normal { .. } = self.state {
                    self.move_cursor_to_window(window, &mut effects);
                }
                EventResult::forwarded(effects)
            }
            BackendEvent::WindowDestroyed(window) => {
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
            BackendEvent::PointerMoved { pos, time } => {
                let pixel_pos = pos.to_i32_round();
                self.last_pointer_pos = pixel_pos;
                self.handle_pointer_motion(pixel_pos, time, &mut effects)
            }
            BackendEvent::PointerButton {
                button,
                pressed,
                pos,
                time,
            } => {
                let pixel_pos = pos.to_i32_round();
                self.last_pointer_pos = pixel_pos;
                self.handle_pointer_button(button, pressed, pixel_pos, time, &mut effects)
            }
            BackendEvent::KeyPress { modifiers, keysym } => {
                self.handle_keypress(modifiers, keysym, &mut effects)
            }
            BackendEvent::KeyTap(keysym) => self.handle_tap(keysym, &mut effects),
        }
    }

    pub fn execute_action(&mut self, action: WMAction, effects: &mut Vec<BackendEffect>) {
        match action {
            WMAction::Close => {
                if let Some(window) = self.get_current_window() {
                    self.close_window(window, effects);
                }
            }
            WMAction::Minimize => {
                if let Some(window) = self.get_current_window() {
                    self.minimize_window(window, effects);
                }
            }
            WMAction::Maximize => {
                if let Some(window) = self.get_current_window() {
                    self.maximize_window(window, effects);
                }
            }
            WMAction::SwapMaster => todo!(),
            WMAction::FocusMaster => todo!(),
            WMAction::ToggleStash => self.toggle_stash(effects),
            WMAction::RestoreAll => self.restore_all_stashed(effects),
            WMAction::CloseAll => self.close_all_stashed(effects),
            WMAction::Spawn(command) => self.spawn_command(command, effects),
            WMAction::Quit => todo!(),
        }
    }

    // Helpers
    pub fn close_window(&mut self, window: WindowElement, effects: &mut Vec<BackendEffect>) {
        self.visible_windows.retain(|w| w != &window);
        effects.push(BackendEffect::CloseWindow(window));
        self.recompute_layout(effects);
        self.normal_state_under_cursor(effects);
    }
    pub fn minimize_window(&mut self, window: WindowElement, effects: &mut Vec<BackendEffect>) {
        self.visible_windows.retain(|w| w != &window);
        self.stashed_windows.push(window.clone());
        let size = Size::new(PREVIEW_SIZE.0 * 2, PREVIEW_SIZE.1 * 2);
        effects.push(BackendEffect::SetWindowSize {
            window: window.clone(),
            size,
        });
        effects.push(BackendEffect::UnmapWindow(window));
        self.recompute_layout(effects);
        self.normal_state_under_cursor(effects);
    }
    pub fn maximize_window(&mut self, window: WindowElement, effects: &mut Vec<BackendEffect>) {
        let others: Vec<_> = self
            .visible_windows
            .drain(..)
            .filter(|w| w != &window)
            .collect();
        for other in others {
            effects.push(BackendEffect::UnmapWindow(other.clone()));
            self.stashed_windows.push(other);
        }
        self.visible_windows = vec![window.clone()];
        self.recompute_layout(effects);
        effects.push(BackendEffect::SetFocus(Some(window)));
    }
    pub fn swap_windows(
        &mut self,
        w1: WindowElement,
        w2: WindowElement,
        effects: &mut Vec<BackendEffect>,
    ) {
        let idx1 = self.visible_windows.iter().position(|w| w == &w1);
        let idx2 = self.visible_windows.iter().position(|w| w == &w2);
        if let (Some(i1), Some(i2)) = (idx1, idx2) {
            self.visible_windows.swap(i1, i2);
            self.recompute_layout(effects);
        }
    }
    pub fn restore_stashed(&mut self, window: WindowElement, effects: &mut Vec<BackendEffect>) {
        self.stashed_windows.retain(|w| w != &window);
        self.visible_windows.push(window.clone());
        self.recompute_layout(effects);
        effects.push(BackendEffect::SetFocus(Some(window)));
        if self.stashed_windows.is_empty() {
            self.normal_state_under_cursor(effects);
        }
    }
    pub fn close_stashed(&mut self, window: WindowElement, effects: &mut Vec<BackendEffect>) {
        self.stashed_windows.retain(|w| w != &window);
        effects.push(BackendEffect::CloseWindow(window));
        if self.stashed_windows.is_empty() {
            self.normal_state_under_cursor(effects);
        }
    }
    pub fn restore_all_stashed(&mut self, effects: &mut Vec<BackendEffect>) {
        let all = std::mem::take(&mut self.stashed_windows);
        self.visible_windows.extend(all);
        self.recompute_layout(effects);
        self.normal_state_under_cursor(effects);
    }
    pub fn close_all_stashed(&mut self, effects: &mut Vec<BackendEffect>) {
        let all = std::mem::take(&mut self.stashed_windows);
        for w in all {
            effects.push(BackendEffect::CloseWindow(w));
        }
        self.normal_state_under_cursor(effects);
    }
    pub fn toggle_stash(&mut self, effects: &mut Vec<BackendEffect>) {
        if let WMState::StashOpened { .. } = self.state {
            self.normal_state_under_cursor(effects);
        } else {
            self.state = WMState::StashOpened {
                mouse_area: StashArea::Outside,
            };
            effects.push(BackendEffect::SetFocus(None));
        }
    }
    pub fn select_window(&mut self, window: WindowElement, effects: &mut Vec<BackendEffect>) {
        self.state = WMState::WindowSelected {
            window,
            hovered_window: None,
            hovered_button: None,
        };
        effects.push(BackendEffect::SetFocus(None));
    }
    pub fn spawn_command(&mut self, command: String, effects: &mut Vec<BackendEffect>) {
        effects.push(BackendEffect::Spawn(command));
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
    pub fn normal_state_under_cursor(&mut self, effects: &mut Vec<BackendEffect>) {
        let under = self.window_at(self.last_pointer_pos);
        self.rebound.reset();
        self.state = WMState::Normal {
            active_window: under.clone(),
        };
        effects.push(BackendEffect::SetFocus(under));
    }
    pub fn move_cursor_to_window(
        &mut self,
        window: WindowElement,
        effects: &mut Vec<BackendEffect>,
    ) {
        if let Some(rect) = self.get_window_geometry(&window) {
            let center = Point::new(rect.loc.x + rect.size.w / 2, rect.loc.y + rect.size.h / 2);
            self.last_pointer_pos = center;
            self.rebound.reset();
            effects.push(BackendEffect::SetPointerLocation(center));
        }
    }

    fn get_current_window(&self) -> Option<WindowElement> {
        match &self.state {
            WMState::Normal { active_window } => active_window.clone(),
            WMState::WindowSelected { window, .. } => Some(window.clone()),
            WMState::StashOpened { .. } => None,
        }
    }

    fn recompute_layout(&self, effects: &mut Vec<BackendEffect>) {
        let tiles = master_slave_layout(self.screen_size, self.visible_windows.len(), 10);
        for (win, tile) in self.visible_windows.iter().zip(tiles.into_iter()) {
            effects.push(BackendEffect::MapWindow {
                window: win.clone(),
                loc: tile.loc,
            });
            effects.push(BackendEffect::SetWindowSize {
                window: win.clone(),
                size: tile.size,
            });
        }
    }

    fn get_window_for_edge(&self, edge: Edge, y: i32) -> Option<WindowElement> {
        if self.visible_windows.is_empty() {
            return None;
        }
        if edge == Edge::Left || self.visible_windows.len() == 1 {
            return self.visible_windows.first().cloned();
        }
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
}
