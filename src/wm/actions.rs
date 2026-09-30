#[derive(Debug, Clone, PartialEq)]
pub enum WMAction {
    // Acts on the current window
    Close,
    Minimize,
    Maximize,

    // Layout commands
    SwapMaster,
    FocusMaster,

    // Stash commands
    ToggleStash,
    RestoreAll,
    CloseAll,

    // General
    Spawn(String),
    Quit,
}
