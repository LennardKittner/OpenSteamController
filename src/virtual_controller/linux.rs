use std::ops::Neg;

use crate::debug_println;
use crate::virtual_controller::{AbstractVirtualController, ControllerInput};
use uinput::event::Controller;
use uinput::event::{absolute, controller, relative};
use uinput::{Device, Result};

/// Xbox series x
const PRODUCT_ID: u16 = 0x0b12;
/// Microsoft
const VENDOR_ID: u16 = 0x045e;
/// Name of the controller
const NAME: &str = "Xbox Controller";

/// Name of the virtual mouse
const MOUSE_NAME: &str = "Steam Controller virtual mouse";

/// map generic controller input the the uinput specific events
/// returns the controller input event and a bool specifying whether the input is pressed or
/// released
///
/// Panics if the input is analog
fn map_digital_controller_input(
    xinput_mode: bool,
    input: ControllerInput,
) -> Option<(uinput::event::Controller, bool)> {
    Some(match (xinput_mode, input) {
        (false, ControllerInput::South(pressed)) => {
            (Controller::GamePad(controller::GamePad::South), pressed)
        }
        (_, ControllerInput::A(pressed)) => (Controller::GamePad(controller::GamePad::A), pressed),
        (false, ControllerInput::East(pressed)) => {
            (Controller::GamePad(controller::GamePad::East), pressed)
        }
        (_, ControllerInput::B(pressed)) => (Controller::GamePad(controller::GamePad::B), pressed),
        (_, ControllerInput::C(pressed)) => (Controller::GamePad(controller::GamePad::C), pressed),
        (false, ControllerInput::North(pressed)) => {
            (Controller::GamePad(controller::GamePad::North), pressed)
        }
        (_, ControllerInput::X(pressed)) => (Controller::GamePad(controller::GamePad::X), pressed),
        (false, ControllerInput::West(pressed)) => {
            (Controller::GamePad(controller::GamePad::West), pressed)
        }
        (_, ControllerInput::Y(pressed)) => (Controller::GamePad(controller::GamePad::Y), pressed),
        (_, ControllerInput::Z(pressed)) => (Controller::GamePad(controller::GamePad::Z), pressed),
        (_, ControllerInput::LeftBumper(pressed)) => {
            (Controller::GamePad(controller::GamePad::TL), pressed)
        }
        (_, ControllerInput::RightBumper(pressed)) => {
            (Controller::GamePad(controller::GamePad::TR), pressed)
        }
        (_, ControllerInput::Select(pressed)) => {
            (Controller::GamePad(controller::GamePad::Select), pressed)
        }
        (_, ControllerInput::Start(pressed)) => {
            (Controller::GamePad(controller::GamePad::Start), pressed)
        }
        (false, ControllerInput::Mode(pressed)) => {
            (Controller::GamePad(controller::GamePad::Mode), pressed)
        }
        (_, ControllerInput::LeftThumb(pressed)) => {
            (Controller::GamePad(controller::GamePad::ThumbL), pressed)
        }
        (_, ControllerInput::RightThumb(pressed)) => {
            (Controller::GamePad(controller::GamePad::ThumbR), pressed)
        }
        (false, ControllerInput::Menu(pressed)) => {
            (Controller::GamePad(controller::GamePad::Mode), pressed)
        }
        (false, ControllerInput::Home(pressed)) => {
            (Controller::GamePad(controller::GamePad::Mode), pressed)
        }
        (false, ControllerInput::Up(pressed)) => (Controller::DPad(controller::DPad::Up), pressed),
        (false, ControllerInput::Down(pressed)) => {
            (Controller::DPad(controller::DPad::Down), pressed)
        }
        (false, ControllerInput::Left(pressed)) => {
            (Controller::DPad(controller::DPad::Left), pressed)
        }
        (false, ControllerInput::Right(pressed)) => {
            (Controller::DPad(controller::DPad::Right), pressed)
        }
        _ => return None,
    })
}

fn map_digital_mouse_input(input: ControllerInput) -> Option<(uinput::event::Controller, bool)> {
    Some(match input {
        ControllerInput::RightTrackpadClick(pressed) => {
            (Controller::Mouse(controller::Mouse::Left), pressed)
        }
        // ControllerInput::LeftTrackpadClick(pressed) => (Controller::Mouse(controller::Mouse::Middle), pressed),
        _ => return None,
    })
}

/// A virtual controller to send button inputs
pub struct VirtualController {
    xinput_mode: bool,
    controller: Device,
    mouse: Device,
    left_trackpad_prev: Option<(f32, f32, f32)>,
    right_trackpad_prev: Option<(f32, f32, f32)>,
}

/// Stick all the way in one direction
const STICK_MIN: i32 = -32768;
/// Stick all the way in the other direction
const STICK_MAX: i32 = 32767;
/// Trigger pressed down
const TRIGGER_MAX: i32 = 255;

const MOUSE_SPEED: f32 = 256.0;
const WHEEL_SPEED: f32 = -64.0;

const HAT_NONE: i32 = 0;
const HAT_LEFT: i32 = -1;
const HAT_RIGHT: i32 = 1;
const HAT_UP: i32 = -1;
const HAT_DOWN: i32 = 1;

impl VirtualController {
    /// create new virtual controller
    pub fn new(xinput_mode: bool) -> Result<VirtualController> {
        let mut controller = uinput::default()?
            .name(NAME)?
            .vendor(VENDOR_ID)
            .product(PRODUCT_ID)
            .event(uinput::event::Controller::GamePad(controller::GamePad::A))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::B))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::X))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::Y))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::TL))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::TR))?
            .event(uinput::event::Absolute::Hat(absolute::Hat::X0))?
            .min(HAT_LEFT)
            .max(HAT_RIGHT)
            .fuzz(0)
            .flat(0)
            .event(uinput::event::Absolute::Hat(absolute::Hat::Y0))?
            .min(HAT_UP)
            .max(HAT_DOWN)
            .fuzz(0)
            .flat(0)
            .event(uinput::event::Controller::GamePad(
                controller::GamePad::Select,
            ))?
            .event(uinput::event::Controller::GamePad(
                controller::GamePad::Start,
            ))?
            .event(uinput::event::Controller::GamePad(
                controller::GamePad::ThumbL,
            ))?
            .event(uinput::event::Controller::GamePad(
                controller::GamePad::ThumbR,
            ))?
            // Left stick
            .event(uinput::event::Absolute::Position(absolute::Position::X))?
            .min(STICK_MIN)
            .max(STICK_MAX)
            .fuzz(0)
            .flat(0)
            .event(uinput::event::Absolute::Position(absolute::Position::Y))?
            .min(STICK_MIN)
            .max(STICK_MAX)
            .fuzz(0)
            .flat(0)
            // Right stick
            .event(uinput::event::Absolute::Position(absolute::Position::RX))?
            .min(STICK_MIN)
            .max(STICK_MAX)
            .fuzz(0)
            .flat(0)
            .event(uinput::event::Absolute::Position(absolute::Position::RY))?
            .min(STICK_MIN)
            .max(STICK_MAX)
            .fuzz(0)
            .flat(0)
            // Triggers
            .event(uinput::event::Absolute::Position(absolute::Position::Z))?
            .min(0)
            .max(TRIGGER_MAX)
            .fuzz(0)
            .flat(0)
            .event(uinput::event::Absolute::Position(absolute::Position::RZ))?
            .min(0)
            .max(TRIGGER_MAX)
            .fuzz(0)
            .flat(0);
        if !xinput_mode {
            controller = controller
                .event(uinput::event::Controller::GamePad(controller::GamePad::TR2))?
                .event(uinput::event::Controller::GamePad(controller::GamePad::TL2))?
                .event(uinput::event::Controller::GamePad(
                    controller::GamePad::Mode,
                ))?
                .event(uinput::event::Controller::DPad(controller::DPad::Up))?
                .event(uinput::event::Controller::DPad(controller::DPad::Down))?
                .event(uinput::event::Controller::DPad(controller::DPad::Left))?
                .event(uinput::event::Controller::DPad(controller::DPad::Right))?;
        }
        let controller = controller.create()?;
        let mouse = uinput::default()?
            .name(MOUSE_NAME)?
            .event(uinput::event::Relative::Position(relative::Position::X))?
            .event(uinput::event::Relative::Position(relative::Position::Y))?
            .event(uinput::event::Relative::Wheel(relative::Wheel::Horizontal))?
            .event(uinput::event::Relative::Wheel(relative::Wheel::Vertical))?
            .event(uinput::event::Controller::Mouse(controller::Mouse::Left))?
            .event(uinput::event::Controller::Mouse(controller::Mouse::Middle))?
            .event(uinput::event::Controller::Mouse(controller::Mouse::Right))?
            .create()?;
        let left_trackpad_prev: Option<(f32, f32, f32)> = None;
        let right_trackpad_prev: Option<(f32, f32, f32)> = None;
        Ok(Self {
            xinput_mode,
            controller,
            mouse,
            left_trackpad_prev,
            right_trackpad_prev,
        })
    }

    /// helper for digital inputs
    fn perform_digital_input(&mut self, input: ControllerInput) -> anyhow::Result<()> {
        if let Some((input, pressed)) = map_digital_controller_input(self.xinput_mode, input) {
            if pressed {
                self.controller.press(&input)?;
            } else {
                self.controller.release(&input)?;
            }
        } else if let Some((input, pressed)) = map_digital_mouse_input(input) {
            if pressed {
                self.mouse.press(&input)?;
            } else {
                self.mouse.release(&input)?;
            }
        } else {
            debug_println!("Ignoring input {input:?}");
            return Ok(());
        };
        Ok(())
    }
}

impl AbstractVirtualController for VirtualController {
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()> {
        match input {
            ControllerInput::Left(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::X0,
                    if pressed { HAT_LEFT } else { HAT_NONE },
                )?;
                if !self.xinput_mode {
                    self.perform_digital_input(input)?;
                }
            }
            ControllerInput::Right(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::X0,
                    if pressed { HAT_RIGHT } else { HAT_NONE },
                )?;
                if !self.xinput_mode {
                    self.perform_digital_input(input)?;
                }
            }
            ControllerInput::Up(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::Y0,
                    if pressed { HAT_UP } else { HAT_NONE },
                )?;
                if !self.xinput_mode {
                    self.perform_digital_input(input)?;
                }
            }
            ControllerInput::Down(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::Y0,
                    if pressed { HAT_DOWN } else { HAT_NONE },
                )?;
                if !self.xinput_mode {
                    self.perform_digital_input(input)?;
                }
            }
            ControllerInput::LeftTrackpadTouch(_) => {
                self.left_trackpad_prev = None;
            }
            ControllerInput::RightTrackpadTouch(_) => {
                self.right_trackpad_prev = None;
            }
            ControllerInput::LeftTrackpad(x, y, z) => {
                if let Some((prev_x, prev_y, prev_z)) = self.left_trackpad_prev {
                    let x_diff = x - prev_x;
                    let y_diff = y - prev_y;
                    let _z_diff = z - prev_z;

                    let x_out = (x_diff * WHEEL_SPEED) as i32;
                    let y_out = (y_diff * WHEEL_SPEED) as i32;

                    // Accumulate fractional output error
                    let x_err = x_diff - (x_out as f32 / WHEEL_SPEED);
                    let y_err = y_diff - (y_out as f32 / WHEEL_SPEED);
                    self.left_trackpad_prev = Some((x - x_err, y - y_err, z));

                    self.mouse
                        .position(&uinput::event::relative::Wheel::Horizontal, x_out)?;
                    self.mouse
                        .position(&uinput::event::relative::Wheel::Vertical, y_out)?;
                } else {
                    self.left_trackpad_prev = Some((x, y, z));
                }
            }
            ControllerInput::RightTrackpad(x, y, z) => {
                if let Some((prev_x, prev_y, prev_z)) = self.right_trackpad_prev {
                    let x_diff = x - prev_x;
                    let y_diff = y - prev_y;
                    let _z_diff = z - prev_z;

                    let x_out = (x_diff * MOUSE_SPEED) as i32;
                    let y_out = (y_diff * MOUSE_SPEED) as i32;

                    // Accumulate fractional output error
                    let x_err = x_diff - (x_out as f32 / MOUSE_SPEED);
                    let y_err = y_diff - (y_out as f32 / MOUSE_SPEED);
                    self.right_trackpad_prev = Some((x - x_err, y - y_err, z));

                    self.mouse
                        .position(&uinput::event::relative::Position::X, x_out)?;
                    self.mouse
                        .position(&uinput::event::relative::Position::Y, y_out.neg())?;
                } else {
                    self.right_trackpad_prev = Some((x, y, z));
                }
            }
            ControllerInput::RightJoyStick(x, y) => {
                let y = y.neg();
                self.controller.position(
                    &uinput::event::absolute::Position::RX,
                    if x.is_sign_positive() {
                        x * STICK_MAX as f32
                    } else {
                        x.neg() * STICK_MIN as f32
                    } as i32,
                )?;
                self.controller.position(
                    &uinput::event::absolute::Position::RY,
                    if y.is_sign_positive() {
                        y * STICK_MAX as f32
                    } else {
                        y.neg() * STICK_MIN as f32
                    } as i32,
                )?;
            }
            ControllerInput::LeftJoyStick(x, y) => {
                let y = y.neg();
                self.controller.position(
                    &uinput::event::absolute::Position::X,
                    if x.is_sign_positive() {
                        x * STICK_MAX as f32
                    } else {
                        x.neg() * STICK_MIN as f32
                    } as i32,
                )?;
                self.controller.position(
                    &uinput::event::absolute::Position::Y,
                    if y.is_sign_positive() {
                        y * STICK_MAX as f32
                    } else {
                        y.neg() * STICK_MIN as f32
                    } as i32,
                )?;
            }
            ControllerInput::LeftTrigger(force) => {
                self.controller.position(
                    &uinput::event::absolute::Position::Z,
                    if force.is_sign_positive() {
                        force * TRIGGER_MAX as f32
                    } else {
                        0f32
                    } as i32,
                )?;
            }
            ControllerInput::RightTrigger(force) => {
                self.controller.position(
                    &uinput::event::absolute::Position::RZ,
                    if force.is_sign_positive() {
                        force * TRIGGER_MAX as f32
                    } else {
                        0f32
                    } as i32,
                )?;
            }
            digital_input => self.perform_digital_input(digital_input)?,
        }
        self.controller.synchronize()?;
        self.mouse.synchronize()?;
        Ok(())
    }
}
