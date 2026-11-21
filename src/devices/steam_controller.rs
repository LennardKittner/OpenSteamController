use std::ops::Range;

use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceEvent, DeviceOutput, DeviceState},
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

pub struct SteamController {
    state: DeviceState,
    bitmap: u32,
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
        Self { state, bitmap: 0 }
    }

    /// Builds a packet based on a given command and a payload
    /// Basic structure: feature report flag, command, size of payload, payload
    fn decorate_packet(command: u8, mut payload: Vec<u8>) -> Vec<u8> {
        let mut packet = vec![FEATURE_REPORT, command, payload.len() as u8];
        packet.append(&mut payload);
        packet.append(&mut vec![0x00; 64 - packet.len()]);
        packet
    }

    /// Converts the slice containing the bits of currently pressed buttons into corresponding
    /// events of press or release button and updates the previous bitmap for comparison
    /// TODO: since we have an upper limit for the number of buttons it may be beneficial to use
    /// fixed size arrays instead of vectors here
    fn handle_buttons(&mut self, response: [u8; 4]) -> Vec<DeviceEvent> {
        let button_bitmap = Button::from_bits_truncate(u32::from_le_bytes(response));
        let previous_bitmap = Button::from_bits_truncate(self.bitmap);
        self.bitmap = button_bitmap.bits();
        let changed_buttons = button_bitmap.symmetric_difference(previous_bitmap);

        if !changed_buttons.is_empty() {
            self.get_buttons(&changed_buttons, &button_bitmap)
        } else {
            vec![]
        }
    }

    /// Converts a bitmap of which buttons changed state and a bitmap of the current button states
    /// into a Vec of DeviceEvent filled with digital DeviceOutput, either pressed or
    /// released
    fn get_buttons(&self, changes: &Button, states: &Button) -> Vec<DeviceEvent> {
        changes
            .iter()
            .map(|button| {
                DeviceEvent::ButtonPressed(bitflags_match!(button, {
                    Button::A => DeviceOutput::A(states.contains(Button::A)),
                    Button::B => DeviceOutput::B(states.contains(Button::B)),
                    Button::X => DeviceOutput::X(states.contains(Button::X)),
                    Button::Y => DeviceOutput::Y(states.contains(Button::Y)),

                    Button::DpadUp => DeviceOutput::Up(states.contains(Button::DpadUp)),
                    Button::DpadDown => DeviceOutput::Down(states.contains(Button::DpadDown)),
                    Button::DpadLeft => DeviceOutput::Left(states.contains(Button::DpadLeft)),
                    Button::DpadRight => DeviceOutput::Right(states.contains(Button::DpadRight)),

                    Button::Start => DeviceOutput::Menu(states.contains(Button::Start)),
                    Button::Select => DeviceOutput::View(states.contains(Button::Select)),
                    Button::Steam => DeviceOutput::Steam(states.contains(Button::Steam)),
                    Button::Menu => DeviceOutput::QuickAccessMenu(states.contains(Button::Menu)),

                    Button::ThumbLeftTouch => DeviceOutput::LeftJoyStickTouch(states.contains(Button::ThumbLeftTouch)),
                    Button::ThumbRightTouch => DeviceOutput::RightJoyStickTouch(states.contains(Button::ThumbRightTouch)),

                    Button::L1 => DeviceOutput::L1(states.contains(Button::L1)),
                    Button::L2 => DeviceOutput::L2(states.contains(Button::L2)),
                    Button::ThumbLeft => DeviceOutput::L3(states.contains(Button::ThumbLeft)),
                    Button::L4 => DeviceOutput::L4(states.contains(Button::L4)),
                    Button::L5 => DeviceOutput::L5(states.contains(Button::L5)),

                    Button::GripLeft => DeviceOutput::LeftGripTouch(states.contains(Button::GripLeft)),

                    Button::R1 => DeviceOutput::R1(states.contains(Button::R1)),
                    Button::R2 => DeviceOutput::R2(states.contains(Button::R2)),
                    Button::ThumbRight => DeviceOutput::R3(states.contains(Button::ThumbRight)),
                    Button::R4 => DeviceOutput::R4(states.contains(Button::R4)),
                    Button::R5 => DeviceOutput::R5(states.contains(Button::R5)),

                    Button::GripRight => DeviceOutput::RightGripTouch(states.contains(Button::GripRight)),

                    Button::PadLeftClick => DeviceOutput::LeftTrackpadClick(states.contains(Button::PadLeftClick)),
                    Button::PadLeftTouch => DeviceOutput::LeftTrackpadTouch(states.contains(Button::PadLeftTouch)),

                    Button::PadRightClick => DeviceOutput::RightTrackpadClick(states.contains(Button::PadRightClick)),
                    Button::PadRightTouch => DeviceOutput::RightTrackpadTouch(states.contains(Button::PadRightTouch)),

                    _ => panic!("Undefined Button!"),
                }))
            })
            .collect::<Vec<DeviceEvent>>()
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
    fn handle_triggers(&mut self, response: [u8; 4]) -> Vec<DeviceEvent> {
        let left = Self::convert_analog(response[0..2].try_into().unwrap());
        let right = Self::convert_analog(response[2..4].try_into().unwrap());
        vec![
            DeviceEvent::ButtonPressed(DeviceOutput::LeftTrigger(left)),
            DeviceEvent::ButtonPressed(DeviceOutput::RightTrigger(right)),
        ]
    }

    /// Converts four bytes of data into controller input events for the left and right joystick
    fn handle_joysticks(&self, response: [u8; 8]) -> Vec<DeviceEvent> {
        let (left_x, left_y) = Self::convert_analog_2d(response[0..4].try_into().unwrap());
        let (right_x, right_y) = Self::convert_analog_2d(response[4..8].try_into().unwrap());
        vec![
            DeviceEvent::ButtonPressed(DeviceOutput::LeftJoyStick(left_x, left_y)),
            DeviceEvent::ButtonPressed(DeviceOutput::RightJoyStick(right_x, right_y)),
        ]
    }

    /// Converts four bytes of data into controller input events for the left and right trackpad
    fn handle_trackpads(&mut self, response: [u8; 12]) -> Vec<DeviceEvent> {
        let (left_x, left_y, left_force) =
            Self::convert_analog_3d(response[0..6].try_into().unwrap());
        let (right_x, right_y, right_force) =
            Self::convert_analog_3d(response[6..12].try_into().unwrap());
        vec![
            DeviceEvent::ButtonPressed(DeviceOutput::LeftTrackpad(left_x, left_y, left_force)),
            DeviceEvent::ButtonPressed(DeviceOutput::RightTrackpad(right_x, right_y, right_force)),
        ]
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
                events.append(
                    &mut self.handle_joysticks(response[JOYSTICK_LOCATION].try_into().unwrap()),
                );
                events.append(
                    &mut self.handle_trackpads(response[TRACKPAD_LOCATION].try_into().unwrap()),
                );
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

    fn active_refresh_state(&mut self) -> Result<Vec<DeviceOutput>, super::DeviceError> {
        self.get_device_state()
            .write_hid_report(&SteamController::get_disable_lizard_mode_packet())?;
        Ok(Vec::new())
    }
}
