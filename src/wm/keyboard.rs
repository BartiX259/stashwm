use smithay::input::keyboard::{Keysym, ModifiersState};

// use super::events::*;
// use super::layout::*;
// use crate::wm::StashArea;
use crate::{
    protocol::{BackendEffect, EventResult},
    wm::{WMEngine, WMState},
};
// use crate::wm::WMState;
// use smithay::utils::{Logical, Point, Size};

impl WMEngine {
    pub fn handle_keypress(
        &mut self,
        modifiers: ModifiersState,
        keysym: Keysym,
        effects: &mut Vec<BackendEffect>,
    ) -> EventResult {
        tracing::info!("press {:?} + {:?}", modifiers, keysym);
        match &self.state {
            WMState::Normal { active_window } => {
                if modifiers.logo && keysym == Keysym::Return {
                    self.spawn_command("alacritty".to_string(), effects);
                } else if modifiers.logo && keysym == Keysym::z {
                    if let Some(window) = active_window {
                        self.close_window(window.clone(), effects);
                    }
                } else {
                    return EventResult::empty_forwarded();
                }
            }
            _ => (),
        }
        EventResult::consumed(std::mem::take(effects))
    }
    pub fn handle_tap(&mut self, keysym: Keysym, effects: &mut Vec<BackendEffect>) -> EventResult {
        tracing::info!("tap {:?}", keysym);
        if keysym == Keysym::Super_L {
            self.toggle_stash(effects);
        } else {
            return EventResult::empty_forwarded();
        }
        EventResult::consumed(std::mem::take(effects))
    }
}
