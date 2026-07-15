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

#[derive(Debug, Clone, Copy, PartialEq)]
/// Backed independent controller inputs
pub enum ControllerInput {
    South(bool),
    A(bool),
    East(bool),
    B(bool),
    C(bool),
    North(bool),
    X(bool),
    West(bool),
    Y(bool),
    Z(bool),
    LeftBumper(bool),
    RightBumper(bool),
    Select(bool),
    Start(bool),
    Mode(bool),
    LeftThumb(bool),
    RightThumb(bool),
    Up(bool),
    Down(bool),
    Left(bool),
    Right(bool),
    Menu(bool),
    Home(bool),
    RightJoyStick(f32, f32),
    LeftJoyStick(f32, f32),
    LeftTrigger(f32),
    LeftTriggerClick(bool),
    RightTrigger(f32),
    RightTriggerClick(bool),
    Wheel(f32, f32),
    Mouse(f32, f32),
    LeftMouse(bool),
    MiddleMouse(bool),
    RightMouse(bool),
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

/// Used to implement OS independent virtual controller
pub trait AbstractVirtualController {
    /// Send a virtual controller input
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()>;
}
