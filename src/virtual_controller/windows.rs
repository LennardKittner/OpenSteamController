use std::{ops::Neg, time::Duration};

use vigem_client::{Client, Error, XButtons, XGamepad, Xbox360Wired};

use crate::virtual_controller::{AbstractVirtualController, ControllerInput};

/// A virtual controller to send button inputs
pub struct VirtualController {
    device: Xbox360Wired<Client>,
    state: XGamepad,
}

/// min value for stick inputs
const STICK_MIN: i16 = i16::MIN;
/// max value for stick inputs
const STICK_MAX: i16 = i16::MAX;
/// mac value for tirggers
const TRIGGER_MAX: u8 = u8::MAX;

/// calculate new state based on current state and a new input
fn update_digital_buttons(current_buttons: u16, input: ControllerInput) -> u16 {
    let bit_flag_helper = |set, flag: u16, state: u16| {
        if set {
            state | flag
        } else {
            state & !flag
        }
    };

    match input {
        ControllerInput::South(set) => bit_flag_helper(set, XButtons::A, current_buttons),
        ControllerInput::A(set) => bit_flag_helper(set, XButtons::A, current_buttons),
        ControllerInput::East(set) => bit_flag_helper(set, XButtons::B, current_buttons),
        ControllerInput::B(set) => bit_flag_helper(set, XButtons::B, current_buttons),
        ControllerInput::North(set) => bit_flag_helper(set, XButtons::Y, current_buttons),
        ControllerInput::X(set) => bit_flag_helper(set, XButtons::X, current_buttons),
        ControllerInput::West(set) => bit_flag_helper(set, XButtons::X, current_buttons),
        ControllerInput::Y(set) => bit_flag_helper(set, XButtons::Y, current_buttons),
        ControllerInput::LeftBumper(set) => bit_flag_helper(set, XButtons::LB, current_buttons),
        ControllerInput::RightBumper(set) => bit_flag_helper(set, XButtons::RB, current_buttons),
        ControllerInput::Select(set) => bit_flag_helper(set, XButtons::BACK, current_buttons),
        ControllerInput::Start(set) => bit_flag_helper(set, XButtons::START, current_buttons),
        ControllerInput::Mode(set) => bit_flag_helper(set, XButtons::GUIDE, current_buttons),
        ControllerInput::LeftThumb(set) => bit_flag_helper(set, XButtons::LTHUMB, current_buttons),
        ControllerInput::RightThumb(set) => bit_flag_helper(set, XButtons::RTHUMB, current_buttons),
        ControllerInput::Up(set) => bit_flag_helper(set, XButtons::UP, current_buttons),
        ControllerInput::Down(set) => bit_flag_helper(set, XButtons::DOWN, current_buttons),
        ControllerInput::Left(set) => bit_flag_helper(set, XButtons::LEFT, current_buttons),
        ControllerInput::Right(set) => bit_flag_helper(set, XButtons::RIGHT, current_buttons),
        ControllerInput::Menu(set) => bit_flag_helper(set, XButtons::GUIDE, current_buttons),
        ControllerInput::Home(set) => bit_flag_helper(set, XButtons::GUIDE, current_buttons),

        ControllerInput::Z(_) => current_buttons,
        ControllerInput::C(_) => current_buttons,
        input => {
            println!("Unknown or analog input {:?}", input);
            current_buttons
        }
    }
}

impl VirtualController {
    /// create new virtual controller
    pub fn new() -> Result<VirtualController, Error> {
        let client = vigem_client::Client::connect().unwrap();
        let id = vigem_client::TargetId::XBOX360_WIRED;
        let mut device = vigem_client::Xbox360Wired::new(client, id);
        std::thread::sleep(Duration::from_millis(2000));
        device.plugin().unwrap();
        std::thread::sleep(Duration::from_millis(2000));
        device.wait_ready().unwrap();
        Ok(Self {
            device,
            state: XGamepad::default(),
        })
    }
}

impl AbstractVirtualController for VirtualController {
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()> {
        match input {
            ControllerInput::Wheel(_, _) => {
                return Ok(());
            }
            ControllerInput::Mouse(_, _) => {
                return Ok(());
            }
            ControllerInput::RightJoyStick(x, y) => {
                self.state.thumb_rx = if x.is_sign_positive() {
                    x * STICK_MAX as f32
                } else {
                    x.neg() * STICK_MIN as f32
                } as i16;
                self.state.thumb_ry = if y.is_sign_positive() {
                    y * STICK_MAX as f32
                } else {
                    y.neg() * STICK_MIN as f32
                } as i16;
            }
            ControllerInput::LeftJoyStick(x, y) => {
                self.state.thumb_lx = if x.is_sign_positive() {
                    x * STICK_MAX as f32
                } else {
                    x.neg() * STICK_MIN as f32
                } as i16;
                self.state.thumb_ly = if y.is_sign_positive() {
                    y * STICK_MAX as f32
                } else {
                    y.neg() * STICK_MIN as f32
                } as i16;
            }
            ControllerInput::LeftTrigger(force) => {
                self.state.left_trigger = if force.is_sign_positive() {
                    force * TRIGGER_MAX as f32
                } else {
                    0f32
                } as u8;
            }
            ControllerInput::RightTrigger(force) => {
                self.state.right_trigger = if force.is_sign_positive() {
                    force * TRIGGER_MAX as f32
                } else {
                    0f32
                } as u8;
            }
            digital_input => {
                self.state.buttons = vigem_client::XButtons(update_digital_buttons(
                    self.state.buttons.raw,
                    digital_input,
                ))
            }
        }

        self.device.update(&self.state)?;
        Ok(())
    }
}
