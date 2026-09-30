pub mod events;
pub mod keyboard;
pub mod layout;
pub mod mouse;
pub mod rebound;
pub mod render;

use crate::backend::shell::WindowElement;
use events::*;
use layout::*;
use rebound::Rebound;
use smithay::utils::{Logical, Point, Rectangle, Size};
use tracing::info;

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
            WMEvent::KeyPress { modifiers, keysym } => {
                self.handle_keypress(modifiers, keysym, &mut effects)
            }
            WMEvent::KeyTap(keysym) => self.handle_tap(keysym, &mut effects),
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
