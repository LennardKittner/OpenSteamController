use std::ops::Range;

use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceCommand, DeviceEvent, DeviceState},
    virtual_controller::ControllerInput,
};
use bitflags::{bitflags, bitflags_match};

pub const VENDOR_IDS: [u16; 1] = [0x28de];
pub const PRODUCT_IDS: [u16; 3] = [
    0x1302, // connection via cable
    0x1303, // connection via Bluetooth
    0x1304, // connection via puck
];

/// Flag to start a feature report
const FEATURE_REPORT: u8 = 0x01;
/// Command to set a setting
const SET_SETTING_CMD: u8 = 0x87;
/// Response prefix for button event
const RESPONSE_INPUT_EVENT: [u8; 2] = [0x45, 0x42];
/// Response prefix for status event
const RESPONSE_STATUS_EVENT: u8 = 0x43;
/// Response prefix for connection event
const RESPONSE_CONNECTION_EVENT: u8 = 0x79;
/// Range in the response containing the button bits
const BUTTON_LOCATION: Range<usize> = 2..6;
/// Range in the response containing the triggers
const TRIGGER_LOCATION: Range<usize> = 6..10;
/// Range in the response containing the left yoystick
const JOYSTICK_LOCATION: Range<usize> = 10..18;
/// Range in the response containing the trackpads (x y pressure)
const TRACKPAD_LOCATION: Range<usize> = 18..30;
/// Max value of an analog input
const ANALOG_MAX: f32 = 0b01111111_11111111 as f32;

/// Trackpad as mouse speed multiplier
const TRACKPAD_MOUSE_SPEED: f32 = 256.0;
/// Trackpad as wheel speed multiplier
const TRACKPAD_WHEEL_SPEED: f32 = -64.0;
/// Joystick as mouse speed multiplier
const STICK_MOUSE_SPEED: f32 = 8.0;
/// Joystick as wheel speed multiplier
const STICK_WHEEL_SPEED: f32 = 1.0;

pub struct SteamController {
    state: DeviceState,
    left_trackpad_prev: Option<(f32, f32, f32)>,
    right_trackpad_prev: Option<(f32, f32, f32)>,
    alt_mode: bool,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Button: u32 {
        const A                 = 1 <<  0;
        const B                 = 1 <<  1;
        const X                 = 1 <<  2;
        const Y                 = 1 <<  3;
        const Menu              = 1 <<  4;
        const ThumbRight        = 1 <<  5;
        const Start             = 1 <<  6;
        const R4                = 1 <<  7;
        const R5                = 1 <<  8;
        const R1                = 1 <<  9;
        const DpadDown          = 1 << 10;
        const DpadRight         = 1 << 11;
        const DpadLeft          = 1 << 12;
        const DpadUp            = 1 << 13;
        const Select            = 1 << 14;
        const ThumbLeft         = 1 << 15;
        const Steam             = 1 << 16;
        const L4                = 1 << 17;
        const L5                = 1 << 18;
        const L1                = 1 << 19;
        const ThumbRightTouch   = 1 << 20;
        const PadRightTouch     = 1 << 21;
        const PadRightClick     = 1 << 22;
        const R2                = 1 << 23;
        const ThumbLeftTouch    = 1 << 24;
        const PadLeftTouch      = 1 << 25;
        const PadLeftClick      = 1 << 26;
        const L2                = 1 << 27;
        const GripRight         = 1 << 28;
        const GripLeft          = 1 << 29;
    }
}

impl SteamController {
    /// Returns the complete packet to disable "lizard mode"
    /// Has to be sent frequently to keep it disabled
    pub fn get_disable_lizard_mode_packet() -> Vec<u8> {
        Self::decorate_packet(SET_SETTING_CMD, vec![0x09, 0x00, 0x00])
    }

    /// Initially disables the "lizard mode" and constructs the controller
    pub fn new_from_state(state: DeviceState) -> Self {
        let left_trackpad_prev: Option<(f32, f32, f32)> = None;
        let right_trackpad_prev: Option<(f32, f32, f32)> = None;
        let alt_mode: bool = false;
        Self { state, left_trackpad_prev, right_trackpad_prev, alt_mode }
    }

    /// Builds a packet based on a given command and a payload
    /// Basic structure: feature report flag, command, size of payload, payload
    fn decorate_packet(command: u8, mut payload: Vec<u8>) -> Vec<u8> {
        let mut packet = vec![FEATURE_REPORT, command, payload.len() as u8];
        packet.append(&mut payload);
        packet.append(&mut vec![0x00; 64 - packet.len()]);
        packet
    }

    fn wrap_controller_input_into_device_event(input: &[ControllerInput]) -> Vec<DeviceEvent> {
        input
            .iter()
            .map(|event| DeviceEvent::ButtonPressed(*event))
            .collect()
    }

    /// parse button combinations to generate special events
    /// TODO: maybe use a set instead of a vec for faster lookup
    fn parse_button_combinations(
        &mut self,
        all_buttons: &[ControllerInput],
        changes: &[ControllerInput],
    ) -> Vec<DeviceEvent> {
        if (all_buttons.contains(&ControllerInput::Y(true))
            && changes.contains(&ControllerInput::Home(true)))
            || (changes.contains(&ControllerInput::Y(true))
                && all_buttons.contains(&ControllerInput::Home(true)))
        {
            vec![DeviceEvent::Command(DeviceCommand::TurnOff)]
        } else if (all_buttons.contains(&ControllerInput::A(true))
            && changes.contains(&ControllerInput::Home(true)))
            || (changes.contains(&ControllerInput::A(true))
                && all_buttons.contains(&ControllerInput::Home(true)))
        {
            vec![
                DeviceEvent::ButtonPressed(ControllerInput::A(false)),
                DeviceEvent::ButtonPressed(ControllerInput::B(false)),
                DeviceEvent::Command(DeviceCommand::NintendoLayoutToggle),
            ]
        } else {
            vec![]
        }
    }

    /// Converts the slice containing the bits of currently pressed buttons into corresponding
    /// events of press or release button and updates the previous bitmap for comparison
    /// TODO: since we have an upper limit for the number of buttons it may be beneficial to use
    /// fixed size arrays instead of vectors here
    fn handle_buttons(&mut self, response: [u8; 4]) -> Vec<DeviceEvent> {
        let button_bitmap = Button::from_bits_truncate(u32::from_le_bytes(response));
        let previous_bitmap = Button::from_bits_truncate(
            self.get_device_state()
                .device_properties
                .previous_button_bitmap as u32,
        );
        let changed_buttons = button_bitmap.symmetric_difference(previous_bitmap);

        if !changed_buttons.is_empty() {
            let changed_buttons = self.get_buttons(&changed_buttons, &button_bitmap);
            let mut result = Self::wrap_controller_input_into_device_event(&changed_buttons);
            result.push(DeviceEvent::UpdateBitmap(button_bitmap.bits() as u64));
            let previous_changed_buttons = self.get_buttons(&button_bitmap, &previous_bitmap);
            let mut events_from_combination = self.parse_button_combinations(
                &previous_changed_buttons,
                &changed_buttons,
            );
            result.append(&mut events_from_combination);

            result
        } else {
            vec![]
        }
    }

    /// Converts a bitmap of which buttons changed state and a bitmap of the current button states
    /// into a Vec of ControllerInput filled with digital ControllerInput, either pressed or
    /// released
    fn get_buttons(&mut self, changes: &Button, states: &Button) -> Vec<ControllerInput> {
        changes
            .iter()
            .map(|button| {
                // the `as u8 as f32` convert bools to 0.0 or 1.0
                if self.alt_mode {
                    bitflags_match!(button, {
                        Button::A =>                ControllerInput::EnterKey(states.contains(Button::A)),
                        Button::B =>                ControllerInput::EscKey(states.contains(Button::B)),
                        Button::X =>                ControllerInput::Ignore, // TODO
                        Button::Y =>                ControllerInput::Ignore, // TODO
                        Button::ThumbRight =>       ControllerInput::MiddleMouse(states.contains(Button::ThumbRight)), // TODO
                        Button::Select =>           ControllerInput::Ignore, // TODO
                        Button::R4 =>               ControllerInput::Ignore, // TODO
                        Button::R5 =>               ControllerInput::Ignore, // TODO
                        Button::R1 =>               ControllerInput::Ignore, // TODO
                        Button::DpadDown =>         ControllerInput::DownKey(states.contains(Button::DpadDown)),
                        Button::DpadRight =>        ControllerInput::RightKey(states.contains(Button::DpadRight)),
                        Button::DpadLeft =>         ControllerInput::LeftKey(states.contains(Button::DpadLeft)),
                        Button::DpadUp =>           ControllerInput::UpKey(states.contains(Button::DpadUp)),
                        Button::Start =>            ControllerInput::Ignore, // TODO
                        Button::ThumbLeft =>        ControllerInput::Ignore, // TODO
                        Button::Steam =>            ControllerInput::Ignore, // TODO
                        Button::L4 =>               ControllerInput::Ignore, // TODO
                        Button::L5 =>               ControllerInput::Ignore, // TODO
                        Button::L1 =>               ControllerInput::Ignore, // TODO
                        Button::ThumbRightTouch =>  ControllerInput::Ignore, // TODO
                        Button::PadRightClick =>    ControllerInput::LeftMouse(states.contains(Button::PadRightClick)),
                        Button::R2 =>               ControllerInput::LeftMouse(states.contains(Button::R2)),
                        Button::ThumbLeftTouch =>   ControllerInput::Ignore, //TODO
                        Button::PadLeftClick =>     ControllerInput::Ignore, // TODO
                        Button::L2 =>               ControllerInput::RightMouse(states.contains(Button::L2)),
                        Button::GripRight =>        ControllerInput::Ignore, // TODO
                        Button::GripLeft =>         ControllerInput::Ignore,   // TODO
                        Button::Menu => {
                            self.alt_mode = states.contains(Button::Menu);
                            ControllerInput::Ignore
                        },
                        Button::PadRightTouch => {
                            self.right_trackpad_prev = None;
                            ControllerInput::Ignore
                        },
                        Button::PadLeftTouch => {
                            self.left_trackpad_prev = None;
                            ControllerInput::Ignore
                        },
                        _ => panic!("Undefined Button!"),
                    })
                } else {
                    bitflags_match!(button, {
                        Button::A =>                ControllerInput::A(states.contains(Button::A)),
                        Button::B =>                ControllerInput::B(states.contains(Button::B)),
                        Button::X =>                ControllerInput::X(states.contains(Button::X)),
                        Button::Y =>                ControllerInput::Y(states.contains(Button::Y)),
                        Button::ThumbRight =>       ControllerInput::RightThumb(states.contains(Button::ThumbRight)),
                        Button::Select =>           ControllerInput::Select(states.contains(Button::Select)),
                        Button::R4 =>               ControllerInput::RightBumper(states.contains(Button::R4)), // TODO
                        Button::R5 =>               ControllerInput::RightTrigger(states.contains(Button::R5) as u8 as f32), // TODO
                        Button::R1 =>               ControllerInput::RightBumper(states.contains(Button::R1)),
                        Button::DpadDown =>         ControllerInput::Down(states.contains(Button::DpadDown)),
                        Button::DpadRight =>        ControllerInput::Right(states.contains(Button::DpadRight)),
                        Button::DpadLeft =>         ControllerInput::Left(states.contains(Button::DpadLeft)),
                        Button::DpadUp =>           ControllerInput::Up(states.contains(Button::DpadUp)),
                        Button::Start =>            ControllerInput::Start(states.contains(Button::Start)),
                        Button::ThumbLeft =>        ControllerInput::LeftThumb(states.contains(Button::ThumbLeft)),
                        Button::Steam =>            ControllerInput::Home(states.contains(Button::Steam)),
                        Button::L4 =>               ControllerInput::LeftBumper(states.contains(Button::L4)), // TODO
                        Button::L5 =>               ControllerInput::LeftTrigger(states.contains(Button::L5) as u8 as f32), // TODO
                        Button::L1 =>               ControllerInput::LeftBumper(states.contains(Button::L1)), // TODO
                        Button::ThumbRightTouch =>  ControllerInput::Ignore, // TODO
                        Button::PadRightClick =>    ControllerInput::LeftMouse(states.contains(Button::PadRightClick)), // TODO
                        Button::R2 =>               ControllerInput::RightTriggerClick(states.contains(Button::R2)),
                        Button::ThumbLeftTouch =>   ControllerInput::Ignore, //TODO
                        Button::PadLeftClick =>     ControllerInput::Ignore, // TODO
                        Button::L2 =>               ControllerInput::LeftTriggerClick(states.contains(Button::L2)),
                        Button::GripRight =>        ControllerInput::Ignore, // TODO
                        Button::GripLeft =>         ControllerInput::Ignore,   // TODO
                        Button::Menu => {
                            self.alt_mode = states.contains(Button::Menu);
                            ControllerInput::Ignore
                        },
                        Button::PadRightTouch => {
                            self.right_trackpad_prev = None;
                            ControllerInput::Ignore
                        },
                        Button::PadLeftTouch => {
                            self.left_trackpad_prev = None;
                            ControllerInput::Ignore
                        },
                        _ => panic!("Undefined Button!"),
                    })
                }
            })
            .collect::<Vec<ControllerInput>>()
    }

    /// Converts two bytes of data into a value between -1.0 and 1.0
    fn convert_analog(input: [u8; 2]) -> f32 {
        let bits = i16::from_le_bytes(input) as f32;
        bits / ANALOG_MAX
    }

    /// Converts four bytes of data into two axes between -1.0 and 1.0
    fn convert_analog_2d(input: [u8; 4]) -> (f32, f32) {
        let x = Self::convert_analog(input[0..2].try_into().unwrap());
        let y = Self::convert_analog(input[2..4].try_into().unwrap());
        (x, y)
    }

    /// Converts six bytes of data into two three between -1.0 and 1.0
    fn convert_analog_3d(input: [u8; 6]) -> (f32, f32, f32) {
        let x = Self::convert_analog(input[0..2].try_into().unwrap());
        let y = Self::convert_analog(input[2..4].try_into().unwrap());
        let z = Self::convert_analog(input[4..6].try_into().unwrap());
        (x, y, z)
    }

    /// Converts four bytes of data into controller input events for the left and right trigger
    fn handle_triggers(&self, response: [u8; 4]) -> Vec<DeviceEvent> {
        let left = Self::convert_analog(response[0..2].try_into().unwrap());
        let right = Self::convert_analog(response[2..4].try_into().unwrap());
        if self.alt_mode {
            vec![]
        } else {
            Self::wrap_controller_input_into_device_event(&[
                ControllerInput::LeftTrigger(left),
                ControllerInput::RightTrigger(right),
            ])
        }
    }

    /// Converts four bytes of data into controller input events for the left and right joystick
    fn handle_joysticks(&self, response: [u8; 8]) -> Vec<DeviceEvent> {
        let (left_x, left_y) = Self::convert_analog_2d(response[0..4].try_into().unwrap());
        let (right_x, right_y) = Self::convert_analog_2d(response[4..8].try_into().unwrap());
        if self.alt_mode {
            let mut events = vec![];
            // TODO: Check if the sticks are even being touched
            if ((left_x * left_x) + (left_y * left_y)) > 0.05 {
                events.push(ControllerInput::Wheel(left_x * STICK_WHEEL_SPEED, left_y * STICK_WHEEL_SPEED));
            }
            if ((right_x * right_x) + (right_y * right_y)) > 0.05 {
                events.push(ControllerInput::Mouse(right_x * STICK_MOUSE_SPEED, -right_y * STICK_MOUSE_SPEED));
            }
            Self::wrap_controller_input_into_device_event(&events[..])
        } else {
            Self::wrap_controller_input_into_device_event(&[
                ControllerInput::LeftJoyStick(left_x, left_y),
                ControllerInput::RightJoyStick(right_x, right_y),
            ])
        }
    }

    /// Converts four bytes of data into controller input events for the left and right trackpad
    fn handle_trackpads(&mut self, response: [u8; 12]) -> Vec<DeviceEvent> {
        let (left_x, left_y, left_force) =
            Self::convert_analog_3d(response[0..6].try_into().unwrap());
        let (right_x, right_y, right_force) =
            Self::convert_analog_3d(response[6..12].try_into().unwrap());
        let mut events = vec![];

        if let Some((prev_x, prev_y, prev_force)) = self.left_trackpad_prev {
            let x_diff = left_x - prev_x;
            let y_diff = left_y - prev_y;
            let _force_diff = left_force - prev_force;

            self.left_trackpad_prev = Some((left_x, left_y, left_force));

            events.push(ControllerInput::Wheel(x_diff * TRACKPAD_WHEEL_SPEED, y_diff * TRACKPAD_WHEEL_SPEED));
        } else {
            self.left_trackpad_prev = Some((left_x, left_y, left_force));
        }

        if let Some((prev_x, prev_y, prev_force)) = self.right_trackpad_prev {
            let x_diff = right_x - prev_x;
            let y_diff = right_y - prev_y;
            let _force_diff = right_force - prev_force;

            self.right_trackpad_prev = Some((right_x, right_y, right_force));

            events.push(ControllerInput::Mouse(x_diff * TRACKPAD_MOUSE_SPEED, -y_diff * TRACKPAD_MOUSE_SPEED));
        } else {
            self.right_trackpad_prev = Some((right_x, right_y, right_force));
        }

        Self::wrap_controller_input_into_device_event(&events[..])
    }

    fn handle_status(response: &[u8; 16]) -> Vec<DeviceEvent> {
        // implicit connection event, because receiving data == controller is connected
        let connected_event = DeviceEvent::WirelessConnected(true);
        let charge_event = DeviceEvent::Charging(match response[1] {
            1 => ChargingStatus::NotCharging,
            // 2 means charging with puck, is it worth it do differentiate between the two?
            3 | 2 => ChargingStatus::Charging,
            4 => ChargingStatus::FullyCharged,
            x => {
                debug_println!("Unknown charging status encountered: {x}");
                ChargingStatus::ChargeError
            }
        });
        //  2 battery? values 92-96, sudden jump to 100?
        let battery_event = DeviceEvent::BatteryLevel(response[2]);
        //  3 unknown; values 1-256
        //  4 unknown; values 15 and 16
        //  5 unknown; values 4-184
        //  6 unknown; static 16
        //  7 unknown; values 0-252
        //  8 unknown values 0-19
        //  9 unknown; values 0-245
        // 10 charging? values 0-1, at least one time 0 while connected
        let _charging1 = response[10];
        // 11 unknown; values 0-212
        // 12 charging? values 0-1
        let _charging2 = response[12];
        // 13 unknown; values 56-248
        // 14 unknown; values 98-104
        // 15.. unused? only zeroes
        vec![charge_event, battery_event, connected_event]
    }

    fn handle_connection(response: u8) -> Option<DeviceEvent> {
        match response {
            1 => Some(DeviceEvent::WirelessConnected(false)),
            // use status packets to determine whether a controller is connected to avoid detecting
            // the puck itself as a controller
            2 => None, // Some(DeviceEvent::WirelessConnected(true)),
            e => {
                debug_println!("Unknown connection event: {e}");
                None
            }
        }
    }
}

impl Device for SteamController {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_event_from_device_response(&mut self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        let mut events = vec![];
        match response[0] {
            RESPONSE_STATUS_EVENT => {
                events.append(&mut SteamController::handle_status(
                    &response[0..16].try_into().unwrap(),
                ));
            }
            respone if RESPONSE_INPUT_EVENT.contains(&respone) => {
                events.append(
                    &mut self.handle_buttons(response[BUTTON_LOCATION].try_into().unwrap()),
                );
                events.append(&mut self.handle_triggers(
                    // 34-35 rotate left down
                    // 36-37 rotate bottom down
                    // 42-43 rotate right down
                    // 44-45 rotate left
                    response[TRIGGER_LOCATION].try_into().unwrap(),
                ));
                events.append(&mut self.handle_joysticks(
                    response[JOYSTICK_LOCATION].try_into().unwrap(),
                ));
                events.append(&mut self.handle_trackpads(
                    response[TRACKPAD_LOCATION].try_into().unwrap(),
                ));
            }
            RESPONSE_CONNECTION_EVENT => {
                if let Some(event) = SteamController::handle_connection(response[1]) {
                    events.push(event);
                }
            }
            _ => {
                debug_println!("{:?}", &response);
            }
        }

        if events.is_empty() {
            None
        } else {
            Some(events)
        }
    }

    fn get_device_state(&self) -> &DeviceState {
        &self.state
    }

    fn get_device_state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    fn turn_off(&mut self) -> Result<(), super::DeviceError> {
        let buffer = {
            let mut buf = [0u8; 64];
            buf[0] = 0x01;
            buf[1] = 0x9f;
            buf[2] = 0x04;
            buf[3] = 0x6f; // o
            buf[4] = 0x66; // f
            buf[5] = 0x66; // f
            buf[6] = 0x21; // !

            buf
        };
        self.get_device_state().write_hid_report(&buffer)?;
        Ok(())
    }

    fn allow_passive_refresh(&mut self) -> bool {
        true
    }

    fn active_refresh_state(&mut self) -> Result<Vec<ControllerInput>, super::DeviceError> {
        self.get_device_state()
            .write_hid_report(&SteamController::get_disable_lizard_mode_packet())?;
        Ok(Vec::new())
    }
}
