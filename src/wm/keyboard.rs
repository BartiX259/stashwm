use smithay::input::keyboard::{Keysym, ModifiersState};

// use super::events::*;
// use super::layout::*;
// use crate::wm::StashArea;
use crate::wm::{
    WMEngine, WMState,
    events::{EventResult, WMEffect},
};
// use crate::wm::WMState;
// use smithay::utils::{Logical, Point, Size};

impl WMEngine {
    pub fn handle_keypress(
        &mut self,
        modifiers: ModifiersState,
        keysym: Keysym,
        effects: &mut Vec<WMEffect>,
    ) -> EventResult {
        tracing::info!("press {:?} + {:?}", modifiers, keysym);
        match &self.state {
            WMState::Normal { active_window } => {
                if modifiers.logo && keysym == Keysym::Return {
                    effects.push(WMEffect::Spawn("alacritty".to_string()));
                } else if modifiers.logo && keysym == Keysym::z {
                    if let Some(window) = active_window {
                        self.visible_windows.retain(|w| w != window);
                        effects.push(WMEffect::CloseWindow(window.clone()));
                        self.recompute_layout(effects);
                        self.normal_state_under_cursor(effects);
                    }
                } else {
                    return EventResult::empty_forwarded();
                }
            }
            _ => (),
        }
        EventResult::consumed(std::mem::take(effects))
    }
    pub fn handle_tap(&mut self, keysym: Keysym, effects: &mut Vec<WMEffect>) -> EventResult {
        tracing::info!("tap {:?}", keysym);
        match &self.state {
            WMState::Normal { active_window } => {
                if keysym == Keysym::Super_L {
                    effects.push(WMEffect::Spawn("alacritty".to_string()));
                } else {
                    return EventResult::empty_forwarded();
                }
            }
            _ => (),
        }
        EventResult::consumed(std::mem::take(effects))
    }
}
