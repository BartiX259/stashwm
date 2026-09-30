use crate::backend::shell::WindowElement;
use smithay::utils::{Logical, Point, Size};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionButton {
    Minimize,
    Maximize,
    Close,
}

pub enum WMEvent {
    ScreenResized(Size<i32, Logical>),
    WindowCreated(WindowElement),
    WindowDestroyed(WindowElement),
    PointerMoved {
        pos: Point<f64, Logical>,
        time: Instant,
    },
    PointerButton {
        button: u32,
        pressed: bool,
        pos: Point<f64, Logical>,
        time: Instant,
    },
    KeyEscape,
}

#[derive(Debug)]
pub enum WMEffect {
    MapWindow {
        window: WindowElement,
        loc: Point<i32, Logical>,
    },
    UnmapWindow(WindowElement),
    SetWindowSize {
        window: WindowElement,
        size: Size<i32, Logical>,
    },
    SetFocus(Option<WindowElement>),
    CloseWindow(WindowElement),
}

pub struct EventResult {
    pub effects: Vec<WMEffect>,
    pub consumed: bool,
}

impl EventResult {
    pub fn consumed(effects: Vec<WMEffect>) -> Self {
        Self {
            effects,
            consumed: true,
        }
    }
    pub fn forwarded(effects: Vec<WMEffect>) -> Self {
        Self {
            effects,
            consumed: false,
        }
    }
    pub fn empty_consumed() -> Self {
        Self {
            effects: Vec::new(),
            consumed: true,
        }
    }
    pub fn empty_forwarded() -> Self {
        Self {
            effects: Vec::new(),
            consumed: false,
        }
    }
}
