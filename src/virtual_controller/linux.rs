use std::ops::Neg;

use crate::debug_println;
use crate::virtual_controller::{AbstractVirtualController, ControllerInput};
use uinput::event::{Controller, Keyboard};
use uinput::event::{absolute, controller, keyboard, relative};
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
    input: ControllerInput,
) -> Option<(uinput::event::Controller, bool)> {
    Some(match input {
        ControllerInput::South(pressed) => {
            (Controller::GamePad(controller::GamePad::South), pressed)
        }
        ControllerInput::A(pressed) => (Controller::GamePad(controller::GamePad::A), pressed),
        ControllerInput::East(pressed) => (Controller::GamePad(controller::GamePad::East), pressed),
        ControllerInput::B(pressed) => (Controller::GamePad(controller::GamePad::B), pressed),
        ControllerInput::C(pressed) => (Controller::GamePad(controller::GamePad::C), pressed),
        ControllerInput::North(pressed) => {
            (Controller::GamePad(controller::GamePad::North), pressed)
        }
        ControllerInput::X(pressed) => (Controller::GamePad(controller::GamePad::X), pressed),
        ControllerInput::West(pressed) => (Controller::GamePad(controller::GamePad::West), pressed),
        ControllerInput::Y(pressed) => (Controller::GamePad(controller::GamePad::Y), pressed),
        ControllerInput::Z(pressed) => (Controller::GamePad(controller::GamePad::Z), pressed),
        ControllerInput::LeftBumper(pressed) => {
            (Controller::GamePad(controller::GamePad::TL), pressed)
        }
        ControllerInput::RightBumper(pressed) => {
            (Controller::GamePad(controller::GamePad::TR), pressed)
        }
        ControllerInput::Select(pressed) => {
            (Controller::GamePad(controller::GamePad::Select), pressed)
        }
        ControllerInput::Start(pressed) => {
            (Controller::GamePad(controller::GamePad::Start), pressed)
        }
        ControllerInput::Mode(pressed) => (Controller::GamePad(controller::GamePad::Mode), pressed),
        ControllerInput::LeftThumb(pressed) => {
            (Controller::GamePad(controller::GamePad::ThumbL), pressed)
        }
        ControllerInput::RightThumb(pressed) => {
            (Controller::GamePad(controller::GamePad::ThumbR), pressed)
        }
        ControllerInput::Menu(pressed) => (Controller::GamePad(controller::GamePad::Mode), pressed),
        ControllerInput::Home(pressed) => (Controller::GamePad(controller::GamePad::Mode), pressed),
        ControllerInput::Up(pressed) => (Controller::DPad(controller::DPad::Up), pressed),
        ControllerInput::Down(pressed) => (Controller::DPad(controller::DPad::Down), pressed),
        ControllerInput::Left(pressed) => (Controller::DPad(controller::DPad::Left), pressed),
        ControllerInput::Right(pressed) => (Controller::DPad(controller::DPad::Right), pressed),
        _ => return None,
    })
}

fn map_digital_mouse_input(input: ControllerInput) -> Option<(uinput::event::Controller, bool)> {
    Some(match input {
        ControllerInput::LeftMouse(pressed) => {
            (Controller::Mouse(controller::Mouse::Left), pressed)
        }
        ControllerInput::MiddleMouse(pressed) => {
            (Controller::Mouse(controller::Mouse::Middle), pressed)
        }
        ControllerInput::RightMouse(pressed) => {
            (Controller::Mouse(controller::Mouse::Right), pressed)
        }
        _ => return None,
    })
}

fn map_digital_keyboard_input(input: ControllerInput) -> Option<(uinput::event::Keyboard, bool)> {
    Some(match input {
        ControllerInput::EnterKey(pressed) => (Keyboard::Key(keyboard::Key::Enter), pressed),
        ControllerInput::EscKey(pressed) => (Keyboard::Function(keyboard::Function::Esc), pressed),
        ControllerInput::CtrlKey(pressed) => (Keyboard::Key(keyboard::Key::LeftControl), pressed),
        ControllerInput::ShiftKey(pressed) => (Keyboard::Key(keyboard::Key::LeftShift), pressed),
        ControllerInput::AltKey(pressed) => (Keyboard::Key(keyboard::Key::LeftAlt), pressed),
        ControllerInput::MetaKey(pressed) => (Keyboard::Key(keyboard::Key::LeftMeta), pressed),
        ControllerInput::UpKey(pressed) => (Keyboard::Key(keyboard::Key::Up), pressed),
        ControllerInput::DownKey(pressed) => (Keyboard::Key(keyboard::Key::Down), pressed),
        ControllerInput::LeftKey(pressed) => (Keyboard::Key(keyboard::Key::Left), pressed),
        ControllerInput::RightKey(pressed) => (Keyboard::Key(keyboard::Key::Right), pressed),
        _ => return None,
    })
}

/// A virtual controller to send button inputs
pub struct VirtualController {
    controller: Device,
    mouse: Device,
    mouse_error: (f32, f32),
    wheel_error: (f32, f32),
}

/// Stick all the way in one direction
const STICK_MIN: i32 = -32768;
/// Stick all the way in the other direction
const STICK_MAX: i32 = 32767;
/// Trigger pressed down
const TRIGGER_MAX: i32 = 255;

const HAT_NONE: i32 = 0;
const HAT_LEFT: i32 = -1;
const HAT_RIGHT: i32 = 1;
const HAT_UP: i32 = -1;
const HAT_DOWN: i32 = 1;

impl VirtualController {
    /// create new virtual controller
    pub fn new() -> Result<VirtualController> {
        let controller = uinput::default()?
            .name(NAME)?
            .vendor(VENDOR_ID)
            .product(PRODUCT_ID)
            .event(uinput::event::Controller::GamePad(controller::GamePad::A))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::B))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::X))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::Y))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::TL))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::TR))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::TR2))?
            .event(uinput::event::Controller::GamePad(controller::GamePad::TL2))?
            .event(uinput::event::Controller::GamePad(
                controller::GamePad::Mode,
            ))?
            .event(uinput::event::Controller::DPad(controller::DPad::Up))?
            .event(uinput::event::Controller::DPad(controller::DPad::Down))?
            .event(uinput::event::Controller::DPad(controller::DPad::Left))?
            .event(uinput::event::Controller::DPad(controller::DPad::Right))?
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
            .flat(0)
            .create()?;
        let mouse = uinput::default()?
            .name(MOUSE_NAME)?
            .event(uinput::event::Relative::Wheel(relative::Wheel::Horizontal))?
            .event(uinput::event::Relative::Wheel(relative::Wheel::Vertical))?
            .event(uinput::event::Relative::Position(relative::Position::X))?
            .event(uinput::event::Relative::Position(relative::Position::Y))?
            .event(uinput::event::Controller::Mouse(controller::Mouse::Left))?
            .event(uinput::event::Controller::Mouse(controller::Mouse::Middle))?
            .event(uinput::event::Controller::Mouse(controller::Mouse::Right))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::Enter))?
            .event(uinput::event::Keyboard::Function(keyboard::Function::Esc))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::LeftControl))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::LeftShift))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::LeftAlt))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::LeftMeta))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::Up))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::Down))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::Left))?
            .event(uinput::event::Keyboard::Key(keyboard::Key::Right))?
            .create()?;
        let mouse_error: (f32, f32) = (0.0, 0.0);
        let wheel_error: (f32, f32) = (0.0, 0.0);
        Ok(Self {
            controller,
            mouse,
            mouse_error,
            wheel_error,
        })
    }

    /// helper for digital inputs
    fn perform_digital_input(&mut self, input: ControllerInput) -> anyhow::Result<()> {
        if let Some((input, pressed)) = map_digital_controller_input(input) {
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
        } else if let Some((input, pressed)) = map_digital_keyboard_input(input) {
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
                self.perform_digital_input(input)?;
            }
            ControllerInput::Right(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::X0,
                    if pressed { HAT_RIGHT } else { HAT_NONE },
                )?;
                self.perform_digital_input(input)?;
            }
            ControllerInput::Up(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::Y0,
                    if pressed { HAT_UP } else { HAT_NONE },
                )?;
                self.perform_digital_input(input)?;
            }
            ControllerInput::Down(pressed) => {
                self.controller.position(
                    &uinput::event::absolute::Hat::Y0,
                    if pressed { HAT_DOWN } else { HAT_NONE },
                )?;
                self.perform_digital_input(input)?;
            }
            ControllerInput::Wheel(x, y) => {
                let x = x + self.wheel_error.0;
                let x_out = x as i32;
                self.wheel_error.0 = x - (x_out as f32);

                let y = y + self.wheel_error.1;
                let y_out = y as i32;
                self.wheel_error.1 = y - (y_out as f32);

                self.mouse
                    .position(&uinput::event::relative::Wheel::Horizontal, x_out)?;
                self.mouse
                    .position(&uinput::event::relative::Wheel::Vertical, y_out)?;
            }
            ControllerInput::Mouse(x, y) => {
                let x = x + self.mouse_error.0;
                let x_out = x as i32;
                self.mouse_error.0 = x - (x_out as f32);

                let y = y + self.mouse_error.1;
                let y_out = y as i32;
                self.mouse_error.1 = y - (y_out as f32);

                self.mouse
                    .position(&uinput::event::relative::Position::X, x_out)?;
                self.mouse
                    .position(&uinput::event::relative::Position::Y, y_out)?;
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
