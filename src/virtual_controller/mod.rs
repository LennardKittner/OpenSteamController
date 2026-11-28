#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub use linux::VirtualController;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::VirtualController;

use crate::devices::{HapticEvent, RumbleEvent};

#[derive(Debug, Clone, Copy, PartialEq)]
/// Backed independent controller inputs
pub enum ControllerInput {
    /// A (Steam, Xbox) / B (Nintendo) / Cross (Sony) / BTN_A/BTN_SOUTH (Linux)
    A(bool),
    /// B (Steam, Xbox) / A (Nintendo) / Circle (Sony) / BTN_B/BTN_EAST (Linux)
    B(bool),
    /// BTN_C (Linux)
    C(bool),
    /// X (Steam, Xbox) / Y (Nintendo) / Square (Sony) / BTN_Y/BTN_WEST (Linux) / BTN_X/BTN_NORTH (Linux-Xbox)
    X(bool),
    /// Y (Steam, Xbox) / X (Nintendo) / Triangle (Sony) / BTN_X/BTN_NORTH (Linux) / BTN_Y/BTN_WEST (Linux-Xbox)
    Y(bool),
    /// BTN_Z (Linux) / Capture (Switch-Linux)
    Z(bool),

    /// D-pad up / Hat 0 Y -1 (XInput)
    Up(bool),
    /// D-pad down / Hat 0 Y +1 (XInput)
    Down(bool),
    /// D-pad left / Hat 0 X -1 (XInput)
    Left(bool),
    /// D-pad right / Hat 0 X +1 (Xinput)
    Right(bool),

    /// Start / Menu (Steam, Xbox) / + (Switch) / Options (Sony)
    Start(bool),
    /// Select / View (Steam, Xbox) / - (Switch) / Share (Sony)
    Select(bool),
    /// Guide (XInput, KDE) / Steam (Steam) / Xbox (Xbox) / Home (Switch) / PS (Sony) / Mode (Linux)
    Mode(bool),
    /// Misc (KDE) / Quick Access Menu (Steam) / Share (Xbox)
    Menu(bool),

    LeftJoyStick(f32, f32),
    RightJoyStick(f32, f32),

    /// L2 (analog) / Abs Z (Linux)
    LeftTrigger(f32),
    /// R2 (analog) / Abs RZ (Linux)
    RightTrigger(f32),

    /// L1 (Steam, Sony) / LB (Xbox) / TL (Linux)
    LeftBumper(bool),
    /// L2 (Steam, Sony) / LT (Xbox) / TL2 (Linux)
    LeftTriggerClick(bool),
    /// L3 (Steam, Sony) / LS (Xbox) / ThumbL (Linux)
    LeftThumb(bool),
    /// L4 (Steam) / P3 (Xbox) / LB (Sony) / GripL (Linux) / TRIGGER_HAPPY3 (Linux-XBE) / TRIGGER_HAPPY3 (Linux-DSE)
    LeftGrip1(bool),
    /// L5 (Steam) / P4 (Xbox) / FN1 (Sony) / GripL2 (Linux) / TRIGGER_HAPPY4 (Linux-XBE) / TRIGGER_HAPPY1 (Linux-DSE)
    LeftGrip2(bool),

    /// R1 (Steam, Sony) / RB (Xbox) / TR (Linux)
    RightBumper(bool),
    /// R2 (Steam, Sony) / RT (Xbox) / TR2 (Linux)
    RightTriggerClick(bool),
    /// R3 (Steam, Sony) / RS (Xbox) / ThumbR (Linux)
    RightThumb(bool),
    /// R4 (Steam) / P1 (Xbox) / RB (Sony) / GripR (Linux) / TRIGGER_HAPPY1 (Linux-XBE) / TRIGGER_HAPPY4 (Linux-DSE)
    RightGrip1(bool),
    /// R5 (Steam) / P2 (Xbox) / FN2 (Sony / GripR2 (Linux) / TRIGGER_HAPPY2 (Linux-XBE) / TRIGGER_HAPPY2 (Linux-DSE)
    RightGrip2(bool),

    // Mouse
    /// Interger line scroll
    Wheel(i32, i32),
    /// Fractional line scroll
    SmoothWheel(f32, f32),
    Mouse(i32, i32),
    LeftMouse(bool),
    MiddleMouse(bool),
    RightMouse(bool),

    // Keyboard
    EnterKey(bool),
    EscKey(bool),
    CtrlKey(bool),
    ShiftKey(bool),
    AltKey(bool),
    MetaKey(bool),
    UpKey(bool),
    DownKey(bool),
    LeftKey(bool),
    RightKey(bool),

    Ignore,
}

#[derive(Debug, Clone, Copy)]
pub enum ControllerOutput {
    Rumble(RumbleEvent),
    Haptic(HapticEvent),
}

/// Used to implement OS independent virtual controller
pub trait AbstractVirtualController {
    /// Send a virtual controller input
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()>;
    /// Receive virtual controller output
    fn recv_output(&mut self) -> anyhow::Result<Vec<ControllerOutput>>;
}
