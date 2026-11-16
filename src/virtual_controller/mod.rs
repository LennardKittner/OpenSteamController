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
    /// A (Steam, Xbox) / B (Nintendo) / Cross (Sony) / South (Linux)
    A(bool),
    /// B (Steam, Xbox) / A (Nintendo) / Circle (Sony) / East (Linux)
    B(bool),
    C(bool),
    /// X (Steam, Xbox) / Y (Nintendo) / Square (Sony) / West (Linux)
    X(bool),
    /// Y (Steam, Xbox) / X (Nintendo) / Triangle (Sony) / North (Linux)
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
    RightTrigger(f32),
    LeftTrackpad(f32, f32, f32),
    LeftTrackpadTouch(bool),
    LeftTrackpadClick(bool),
    RightTrackpad(f32, f32, f32),
    RightTrackpadTouch(bool),
    RightTrackpadClick(bool),
    Ignore,
}

/// Used to implement OS independent virtual controller
pub trait AbstractVirtualController {
    /// Send a virtual controller input
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()>;
}
