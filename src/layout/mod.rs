mod lizard;
mod nintendo;
mod xbox;

use crate::devices::{DeviceButton, DeviceCommand, DeviceOutput, DeviceProperties};
use crate::virtual_controller::ControllerInput;
use lizard::LizardLayout;
use nintendo::NintendoLayout;
use xbox::XboxLayout;

/// Trackpad as mouse speed multiplier
const TRACKPAD_MOUSE_SPEED: f32 = 256.0;
/// Mouse movement threshold to trigger trackpad haptics
const TRACKPAD_MOUSE_FEEDBACK_MAX: f32 = TRACKPAD_MOUSE_SPEED / 4.0;
/// Trackpad as wheel speed multiplier
const TRACKPAD_WHEEL_SPEED: f32 = 32.0;
/// Joystick as mouse speed multiplier
const STICK_MOUSE_SPEED: f32 = 8.0;
/// Joystick as mouse deadzone
const STICK_MOUSE_DEADZONE: f32 = 0.025;
/// Joystick as wheel speed multiplier
const STICK_WHEEL_SPEED: f32 = 1.0;
/// Joystick as wheel deadzone
const STICK_WHEEL_DEADZONE: f32 = STICK_MOUSE_DEADZONE;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Events for or from the device
pub enum Layout {
    Xbox,
    Nintendo,
    Lizard,
}

pub enum MappedOutput {
    Device(DeviceCommand),
    Controller(ControllerInput),
}

#[derive(Debug, Clone)]
pub struct MouseHandler {
    trackpad_prev: Option<(f32, f32, f32)>,
    error: (f32, f32),
    distance: f32,
}

impl MouseHandler {
    pub fn new() -> MouseHandler {
        MouseHandler {
            trackpad_prev: None,
            error: (0.0, 0.0),
            distance: 0.0,
        }
    }

    pub fn trackpad(&mut self, (x, y, z): (f32, f32, f32)) -> Option<(i32, i32, bool)> {
        let prev = self.trackpad_prev;
        self.trackpad_prev = Some((x, y, z));
        prev.map(|(prev_x, prev_y, _prev_z)| {
            let x = (x - prev_x) * TRACKPAD_MOUSE_SPEED;
            let y = (y - prev_y) * TRACKPAD_MOUSE_SPEED;

            let x = x + self.error.0;
            let x_out = x as i32;
            self.error.0 = x - (x_out as f32);

            let y = y + self.error.1;
            let y_out = y as i32;
            self.error.1 = y - (y_out as f32);

            self.distance += ((x_out as f32 * x_out as f32) + (y_out as f32 * y_out as f32)).sqrt();

            if self.distance >= TRACKPAD_MOUSE_FEEDBACK_MAX {
                self.distance %= TRACKPAD_MOUSE_FEEDBACK_MAX;
                (x_out, y_out, true)
            } else {
                (x_out, y_out, false)
            }
        })
    }

    pub fn reset_trackpad(&mut self) {
        self.trackpad_prev = None;
        self.error = (0.0, 0.0);
        self.distance = 0.0;
    }

    pub fn joystick(&mut self, (x, y): (f32, f32)) -> Option<(i32, i32)> {
        let x2 = x * x;
        let y2 = y * y;
        if (x2 + y2) < STICK_MOUSE_DEADZONE {
            return None;
        }

        let x = x2 * x.signum() * STICK_MOUSE_SPEED;
        let y = y2 * y.signum() * STICK_MOUSE_SPEED;

        let x = x + self.error.0;
        let x_out = x as i32;
        self.error.0 = x - (x_out as f32);

        let y = y + self.error.1;
        let y_out = y as i32;
        self.error.1 = y - (y_out as f32);

        Some((x_out, y_out))
    }
}

#[derive(Debug, Clone)]
pub struct WheelHandler {
    trackpad_prev: Option<(f32, f32, f32)>,
    error: (f32, f32),
}

impl WheelHandler {
    pub fn new() -> WheelHandler {
        WheelHandler {
            trackpad_prev: None,
            error: (0.0, 0.0),
        }
    }

    pub fn smooth_trackpad(&mut self, (x, y, z): (f32, f32, f32)) -> Option<(f32, f32)> {
        let prev = self.trackpad_prev;
        self.trackpad_prev = Some((x, y, z));
        prev.map(|(prev_x, prev_y, _prev_z)| {
            (
                (x - prev_x) * -TRACKPAD_WHEEL_SPEED,
                (y - prev_y) * -TRACKPAD_WHEEL_SPEED,
            )
        })
    }

    pub fn notched_trackpad(&mut self, (x, y, z): (f32, f32, f32)) -> Option<(i32, i32, bool)> {
        let prev = self.trackpad_prev;
        self.trackpad_prev = Some((x, y, z));
        prev.map(|(prev_x, prev_y, _prev_z)| {
            let x = (x - prev_x) * -TRACKPAD_WHEEL_SPEED;
            let y = (y - prev_y) * -TRACKPAD_WHEEL_SPEED;

            let x = x + self.error.0;
            let x_out = x as i32;
            self.error.0 = x - (x_out as f32);

            let y = y + self.error.1;
            let y_out = y as i32;
            self.error.1 = y - (y_out as f32);

            (x_out, y_out, x_out != 0 || y_out != 0)
        })
    }

    pub fn reset_trackpad(&mut self) {
        self.trackpad_prev = None;
        self.error = (0.0, 0.0);
    }

    pub fn joystick(&self, (x, y): (f32, f32)) -> Option<(f32, f32)> {
        let x2 = x * x;
        let y2 = y * y;
        if (x2 + y2) < STICK_WHEEL_DEADZONE {
            return None;
        }

        let x = x2 * x.signum() * STICK_WHEEL_SPEED;
        let y = y2 * y.signum() * STICK_WHEEL_SPEED;

        Some((x, y))
    }
}

#[derive(Debug, Clone)]
pub struct LayoutState {
    pub buttons_prev: DeviceButton,
    pub mouse: MouseHandler,
    pub wheel: WheelHandler,
}

impl LayoutState {
    pub fn new() -> LayoutState {
        LayoutState {
            buttons_prev: DeviceButton::from_bits_truncate(0),
            mouse: MouseHandler::new(),
            wheel: WheelHandler::new(),
        }
    }

    pub fn update_bitmap(&mut self, input: DeviceOutput) {
        match input {
            DeviceOutput::A(p) => self.buttons_prev.set(DeviceButton::A, p),
            DeviceOutput::B(p) => self.buttons_prev.set(DeviceButton::B, p),
            DeviceOutput::X(p) => self.buttons_prev.set(DeviceButton::X, p),
            DeviceOutput::Y(p) => self.buttons_prev.set(DeviceButton::Y, p),

            DeviceOutput::Up(p) => self.buttons_prev.set(DeviceButton::Up, p),
            DeviceOutput::Down(p) => self.buttons_prev.set(DeviceButton::Down, p),
            DeviceOutput::Left(p) => self.buttons_prev.set(DeviceButton::Left, p),
            DeviceOutput::Right(p) => self.buttons_prev.set(DeviceButton::Right, p),

            DeviceOutput::Menu(p) => self.buttons_prev.set(DeviceButton::Menu, p),
            DeviceOutput::View(p) => self.buttons_prev.set(DeviceButton::View, p),
            DeviceOutput::Steam(p) => self.buttons_prev.set(DeviceButton::Steam, p),
            DeviceOutput::QuickAccessMenu(p) => {
                self.buttons_prev.set(DeviceButton::QuickAccessMenu, p)
            }

            DeviceOutput::LeftJoyStick(_, _) => {}
            DeviceOutput::LeftJoyStickTouch(p) => {
                self.buttons_prev.set(DeviceButton::LeftJoyStickTouch, p)
            }

            DeviceOutput::RightJoyStick(_, _) => {}
            DeviceOutput::RightJoyStickTouch(p) => {
                self.buttons_prev.set(DeviceButton::RightJoyStickTouch, p)
            }

            DeviceOutput::L1(p) => self.buttons_prev.set(DeviceButton::L1, p),
            DeviceOutput::L2(p) => self.buttons_prev.set(DeviceButton::L2, p),
            DeviceOutput::L3(p) => self.buttons_prev.set(DeviceButton::L3, p),
            DeviceOutput::L4(p) => self.buttons_prev.set(DeviceButton::L4, p),
            DeviceOutput::L5(p) => self.buttons_prev.set(DeviceButton::L5, p),

            DeviceOutput::LeftTrigger(_) => {}
            DeviceOutput::LeftGripTouch(p) => self.buttons_prev.set(DeviceButton::LeftGripTouch, p),

            DeviceOutput::R1(p) => self.buttons_prev.set(DeviceButton::R1, p),
            DeviceOutput::R2(p) => self.buttons_prev.set(DeviceButton::R2, p),
            DeviceOutput::R3(p) => self.buttons_prev.set(DeviceButton::R3, p),
            DeviceOutput::R4(p) => self.buttons_prev.set(DeviceButton::R4, p),
            DeviceOutput::R5(p) => self.buttons_prev.set(DeviceButton::R5, p),

            DeviceOutput::RightTrigger(_) => {}
            DeviceOutput::RightGripTouch(p) => {
                self.buttons_prev.set(DeviceButton::RightGripTouch, p)
            }

            DeviceOutput::LeftTrackpad(_, _, _) => {}
            DeviceOutput::LeftTrackpadClick(p) => {
                self.buttons_prev.set(DeviceButton::LeftTrackpadClick, p)
            }
            DeviceOutput::LeftTrackpadTouch(p) => {
                self.buttons_prev.set(DeviceButton::LeftTrackpadTouch, p)
            }

            DeviceOutput::RightTrackpad(_, _, _) => {}
            DeviceOutput::RightTrackpadClick(p) => {
                self.buttons_prev.set(DeviceButton::RightTrackpadClick, p)
            }
            DeviceOutput::RightTrackpadTouch(p) => {
                self.buttons_prev.set(DeviceButton::RightTrackpadTouch, p)
            }
        }
    }
}

pub trait ControllerLayout {
    fn map_device_output(
        state: &mut LayoutState,
        input: DeviceOutput,
        device_properties: &DeviceProperties,
    ) -> Vec<MappedOutput>;
}

fn map_steam_layout(
    state: &LayoutState,
    input: DeviceOutput,
    device_properties: &DeviceProperties,
) -> Option<Vec<MappedOutput>> {
    match input {
        DeviceOutput::Steam(_) => return None,
        _ => {
            if !state.buttons_prev.contains(DeviceButton::Steam) {
                return None;
            }
        }
    };

    match input {
        DeviceOutput::A(p) => {
            if p {
                Some(vec![MappedOutput::Device(DeviceCommand::SwitchLayout(
                    match device_properties.layout {
                        Layout::Xbox => Layout::Nintendo,
                        Layout::Nintendo => Layout::Xbox,
                        Layout::Lizard => device_properties.controller_layout,
                    },
                ))])
            } else {
                None
            }
        }
        DeviceOutput::B(p) => {
            if p {
                Some(vec![MappedOutput::Device(DeviceCommand::SwitchLayout(
                    match device_properties.layout {
                        Layout::Lizard => device_properties.controller_layout,
                        _ => Layout::Lizard,
                    },
                ))])
            } else {
                None
            }
        }
        DeviceOutput::Y(p) => {
            if p {
                Some(vec![MappedOutput::Device(DeviceCommand::TurnOff)])
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn map_device_output(
    state: &mut LayoutState,
    input: DeviceOutput,
    device_properties: &DeviceProperties,
) -> Vec<MappedOutput> {
    if let Some(steam_bindings) = map_steam_layout(state, input, device_properties) {
        state.update_bitmap(input);
        return steam_bindings;
    }

    let result = match device_properties.layout {
        Layout::Xbox => XboxLayout::map_device_output(state, input, device_properties),
        Layout::Nintendo => NintendoLayout::map_device_output(state, input, device_properties),
        Layout::Lizard => LizardLayout::map_device_output(state, input, device_properties),
    };
    state.update_bitmap(input);
    result
}
