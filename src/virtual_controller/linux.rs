use std::{io, ops::Neg, str::FromStr};

use crate::debug_println;
use crate::virtual_controller::{AbstractVirtualController, ControllerInput, ControllerOutput};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    RelativeAxisCode, UinputAbsSetup,
};

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum EmulatedDevice {
    Xbox360,
    #[default]
    XboxElite,
    DualShock4,
    DualSenseEdge,
}

impl FromStr for EmulatedDevice {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "x360" => Ok(Self::Xbox360),
            "xbox360" => Ok(Self::Xbox360),

            "xbe" => Ok(Self::XboxElite),
            "xboxelite" => Ok(Self::XboxElite),

            "ds4" => Ok(Self::DualShock4),
            "dualshock4" => Ok(Self::DualShock4),

            "dse" => Ok(Self::DualSenseEdge),
            "dualsenseedge" => Ok(Self::DualSenseEdge),

            _ => return Err("Unknown device name, expected 'x360', 'xbox360', 'xbe', 'xboxelite', 'ds4', 'dualshock4', 'dse', or 'dualsenseedge'"),
        }
    }
}

const VENDOR_ID_MICROSOFT: u16 = 0x045e;
const PRODUCT_ID_XBOX_360: u16 = 0x028e;
const PRODUCT_ID_XBOX_ONE_ELITE: u16 = 0x02e3;
const PRODUCT_NAME_XBOX_360: &str = "Xbox 360 Controller";
const PRODUCT_NAME_XBOX_ONE_ELITE: &str = "Xbox One Elite Controller";

const VENDOR_ID_SONY: u16 = 0x054c;
const PRODUCT_ID_DUALSHOCK_4: u16 = 0x09cc;
const PRODUCT_ID_DUALSENSE_EDGE: u16 = 0x0df2;
const PRODUCT_NAME_DUALSHOCK_4: &str = "PS4 Controller";
const PRODUCT_NAME_DUALSENSE_EDGE: &str = "DualSense Edge Controller";

fn emulated_device_info(device: EmulatedDevice) -> (u16, u16, &'static str) {
    match device {
        EmulatedDevice::Xbox360 => (
            VENDOR_ID_MICROSOFT,
            PRODUCT_ID_XBOX_360,
            PRODUCT_NAME_XBOX_360,
        ),
        EmulatedDevice::XboxElite => (
            VENDOR_ID_MICROSOFT,
            PRODUCT_ID_XBOX_ONE_ELITE,
            PRODUCT_NAME_XBOX_ONE_ELITE,
        ),
        EmulatedDevice::DualShock4 => (
            VENDOR_ID_SONY,
            PRODUCT_ID_DUALSHOCK_4,
            PRODUCT_NAME_DUALSHOCK_4,
        ),
        EmulatedDevice::DualSenseEdge => (
            VENDOR_ID_SONY,
            PRODUCT_ID_DUALSENSE_EDGE,
            PRODUCT_NAME_DUALSENSE_EDGE,
        ),
    }
}

/// Name of the virtual mouse
const MOUSE_NAME: &str = "Steam Controller virtual mouse";

/// Stick all the way in one direction
const STICK_MIN: i32 = -32768;
/// Stick all the way in the other direction
const STICK_MAX: i32 = 32767;

fn map_stick(val: f32) -> i32 {
    if val.is_sign_positive() {
        (val * (STICK_MAX as f32)) as i32
    } else {
        (val.neg() * (STICK_MIN as f32)) as i32
    }
}

/// Trigger pressed down
const TRIGGER_MAX: i32 = 255;

fn map_trigger(force: f32) -> i32 {
    if force.is_sign_positive() {
        (force * (TRIGGER_MAX as f32)) as i32
    } else {
        0i32
    }
}

/// When using REL_(H)WHEEL_HI_RES, 120 is a complete line
const SMOOTH_WHEEL_MULTIPLIER: i32 = 120;

const HAT_NONE: i32 = 0;
const HAT_LEFT: i32 = -1;
const HAT_RIGHT: i32 = 1;
const HAT_UP: i32 = -1;
const HAT_DOWN: i32 = 1;

// Missing from evdev-0.13.2
const BTN_GRIPL: u16 = 0x224;
const BTN_GRIPR: u16 = 0x225;
const BTN_GRIPL2: u16 = 0x226;
const BTN_GRIPR2: u16 = 0x227;

fn map_absolute_controller_input(
    device: EmulatedDevice,
    input: ControllerInput,
) -> Option<(AbsoluteAxisCode, i32)> {
    Some(match (device, input) {
        (_, ControllerInput::Up(pressed)) => (
            AbsoluteAxisCode::ABS_HAT0Y,
            if pressed { HAT_UP } else { HAT_NONE },
        ),
        (_, ControllerInput::Down(pressed)) => (
            AbsoluteAxisCode::ABS_HAT0Y,
            if pressed { HAT_DOWN } else { HAT_NONE },
        ),
        (_, ControllerInput::Left(pressed)) => (
            AbsoluteAxisCode::ABS_HAT0X,
            if pressed { HAT_LEFT } else { HAT_NONE },
        ),
        (_, ControllerInput::Right(pressed)) => (
            AbsoluteAxisCode::ABS_HAT0X,
            if pressed { HAT_RIGHT } else { HAT_NONE },
        ),

        (EmulatedDevice::DualShock4, ControllerInput::LeftTrigger(force)) => {
            (AbsoluteAxisCode::ABS_RX, map_trigger(force))
        }
        (EmulatedDevice::DualShock4, ControllerInput::RightTrigger(force)) => {
            (AbsoluteAxisCode::ABS_RY, map_trigger(force))
        }

        (_, ControllerInput::LeftTrigger(force)) => (AbsoluteAxisCode::ABS_Z, map_trigger(force)),
        (_, ControllerInput::RightTrigger(force)) => (AbsoluteAxisCode::ABS_RZ, map_trigger(force)),

        _ => return None,
    })
}

fn map_absolute_controller_input_2d(
    device: EmulatedDevice,
    input: ControllerInput,
) -> Option<((AbsoluteAxisCode, i32), (AbsoluteAxisCode, i32))> {
    Some(match (device, input) {
        (_, ControllerInput::LeftJoyStick(x, y)) => (
            (AbsoluteAxisCode::ABS_X, map_stick(x)),
            (AbsoluteAxisCode::ABS_Y, map_stick(y.neg())),
        ),
        (EmulatedDevice::DualShock4, ControllerInput::RightJoyStick(x, y)) => (
            (AbsoluteAxisCode::ABS_Z, map_stick(x)),
            (AbsoluteAxisCode::ABS_RZ, map_stick(y.neg())),
        ),
        (_, ControllerInput::RightJoyStick(x, y)) => (
            (AbsoluteAxisCode::ABS_RX, map_stick(x)),
            (AbsoluteAxisCode::ABS_RY, map_stick(y.neg())),
        ),

        _ => return None,
    })
}

/// map generic controller input the the uinput specific events
/// returns the controller input event and a bool specifying whether the input is pressed or
/// released
fn map_digital_controller_input(
    device: EmulatedDevice,
    input: ControllerInput,
) -> Option<(KeyCode, bool)> {
    Some(match device {
        EmulatedDevice::Xbox360 => match input {
            ControllerInput::A(pressed) => (KeyCode::BTN_SOUTH, pressed),
            ControllerInput::B(pressed) => (KeyCode::BTN_EAST, pressed),
            ControllerInput::X(pressed) => (KeyCode::BTN_NORTH, pressed),
            ControllerInput::Y(pressed) => (KeyCode::BTN_WEST, pressed),

            ControllerInput::Start(pressed) => (KeyCode::BTN_START, pressed),
            ControllerInput::Select(pressed) => (KeyCode::BTN_SELECT, pressed),

            ControllerInput::LeftBumper(pressed) => (KeyCode::BTN_TL, pressed),
            ControllerInput::LeftThumb(pressed) => (KeyCode::BTN_THUMBL, pressed),

            ControllerInput::RightBumper(pressed) => (KeyCode::BTN_TR, pressed),
            ControllerInput::RightThumb(pressed) => (KeyCode::BTN_THUMBR, pressed),

            _ => return None,
        },

        EmulatedDevice::XboxElite => match input {
            ControllerInput::A(pressed) => (KeyCode::BTN_SOUTH, pressed),
            ControllerInput::B(pressed) => (KeyCode::BTN_EAST, pressed),
            ControllerInput::X(pressed) => (KeyCode::BTN_NORTH, pressed),
            ControllerInput::Y(pressed) => (KeyCode::BTN_WEST, pressed),

            ControllerInput::Up(pressed) => (KeyCode::BTN_DPAD_UP, pressed),
            ControllerInput::Down(pressed) => (KeyCode::BTN_DPAD_DOWN, pressed),
            ControllerInput::Left(pressed) => (KeyCode::BTN_DPAD_LEFT, pressed),
            ControllerInput::Right(pressed) => (KeyCode::BTN_DPAD_RIGHT, pressed),

            ControllerInput::Start(pressed) => (KeyCode::BTN_START, pressed),
            ControllerInput::Select(pressed) => (KeyCode::BTN_SELECT, pressed),
            ControllerInput::Mode(pressed) => (KeyCode::BTN_MODE, pressed),

            ControllerInput::LeftBumper(pressed) => (KeyCode::BTN_TL, pressed),
            ControllerInput::LeftTriggerClick(pressed) => (KeyCode::BTN_TL2, pressed),
            ControllerInput::LeftThumb(pressed) => (KeyCode::BTN_THUMBL, pressed),

            ControllerInput::RightBumper(pressed) => (KeyCode::BTN_TR, pressed),
            ControllerInput::RightTriggerClick(pressed) => (KeyCode::BTN_TR2, pressed),
            ControllerInput::RightThumb(pressed) => (KeyCode::BTN_THUMBR, pressed),

            ControllerInput::LeftGrip1(pressed) => (KeyCode(BTN_GRIPL), pressed),
            ControllerInput::LeftGrip2(pressed) => (KeyCode(BTN_GRIPL2), pressed),
            ControllerInput::RightGrip1(pressed) => (KeyCode(BTN_GRIPR), pressed),
            ControllerInput::RightGrip2(pressed) => (KeyCode(BTN_GRIPR2), pressed),

            _ => return None,
        },

        EmulatedDevice::DualShock4 => match input {
            ControllerInput::A(pressed) => (KeyCode::BTN_EAST, pressed),
            ControllerInput::B(pressed) => (KeyCode::BTN_NORTH, pressed),
            ControllerInput::X(pressed) => (KeyCode::BTN_SOUTH, pressed),
            ControllerInput::Y(pressed) => (KeyCode::BTN_WEST, pressed),

            ControllerInput::Up(pressed) => (KeyCode::BTN_DPAD_UP, pressed),
            ControllerInput::Down(pressed) => (KeyCode::BTN_DPAD_DOWN, pressed),
            ControllerInput::Left(pressed) => (KeyCode::BTN_DPAD_LEFT, pressed),
            ControllerInput::Right(pressed) => (KeyCode::BTN_DPAD_RIGHT, pressed),

            ControllerInput::Start(pressed) => (KeyCode::BTN_START, pressed),
            ControllerInput::Select(pressed) => (KeyCode::BTN_SELECT, pressed),
            ControllerInput::Mode(pressed) => (KeyCode::BTN_THUMBR, pressed),

            ControllerInput::LeftBumper(pressed) => (KeyCode::BTN_TL, pressed),
            ControllerInput::LeftTriggerClick(pressed) => (KeyCode::BTN_TL2, pressed),
            ControllerInput::LeftThumb(pressed) => (KeyCode::BTN_MODE, pressed),

            ControllerInput::RightBumper(pressed) => (KeyCode::BTN_TR, pressed),
            ControllerInput::RightTriggerClick(pressed) => (KeyCode::BTN_TR2, pressed),
            ControllerInput::RightThumb(pressed) => (KeyCode::BTN_THUMBL, pressed),

            _ => return None,
        },

        EmulatedDevice::DualSenseEdge => match input {
            ControllerInput::A(pressed) => (KeyCode::BTN_SOUTH, pressed),
            ControllerInput::B(pressed) => (KeyCode::BTN_EAST, pressed),
            ControllerInput::X(pressed) => (KeyCode::BTN_WEST, pressed),
            ControllerInput::Y(pressed) => (KeyCode::BTN_NORTH, pressed),

            ControllerInput::Up(pressed) => (KeyCode::BTN_DPAD_UP, pressed),
            ControllerInput::Down(pressed) => (KeyCode::BTN_DPAD_DOWN, pressed),
            ControllerInput::Left(pressed) => (KeyCode::BTN_DPAD_LEFT, pressed),
            ControllerInput::Right(pressed) => (KeyCode::BTN_DPAD_RIGHT, pressed),

            ControllerInput::Start(pressed) => (KeyCode::BTN_START, pressed),
            ControllerInput::Select(pressed) => (KeyCode::BTN_SELECT, pressed),
            ControllerInput::Mode(pressed) => (KeyCode::BTN_MODE, pressed),

            ControllerInput::LeftBumper(pressed) => (KeyCode::BTN_TL, pressed),
            ControllerInput::LeftTriggerClick(pressed) => (KeyCode::BTN_TL2, pressed),
            ControllerInput::LeftThumb(pressed) => (KeyCode::BTN_THUMBL, pressed),

            ControllerInput::RightBumper(pressed) => (KeyCode::BTN_TR, pressed),
            ControllerInput::RightTriggerClick(pressed) => (KeyCode::BTN_TR2, pressed),
            ControllerInput::RightThumb(pressed) => (KeyCode::BTN_THUMBR, pressed),

            ControllerInput::LeftGrip1(pressed) => (KeyCode(BTN_GRIPL), pressed),
            ControllerInput::LeftGrip2(pressed) => (KeyCode(BTN_GRIPL2), pressed),
            ControllerInput::RightGrip1(pressed) => (KeyCode(BTN_GRIPR), pressed),
            ControllerInput::RightGrip2(pressed) => (KeyCode(BTN_GRIPR2), pressed),

            _ => return None,
        },
    })
}

fn map_digital_mouse_input(input: ControllerInput) -> Option<(KeyCode, bool)> {
    Some(match input {
        ControllerInput::LeftMouse(pressed) => (KeyCode::BTN_LEFT, pressed),
        ControllerInput::MiddleMouse(pressed) => (KeyCode::BTN_MIDDLE, pressed),
        ControllerInput::RightMouse(pressed) => (KeyCode::BTN_RIGHT, pressed),

        ControllerInput::EnterKey(pressed) => (KeyCode::KEY_ENTER, pressed),
        ControllerInput::EscKey(pressed) => (KeyCode::KEY_ESC, pressed),
        ControllerInput::CtrlKey(pressed) => (KeyCode::KEY_LEFTCTRL, pressed),
        ControllerInput::ShiftKey(pressed) => (KeyCode::KEY_LEFTSHIFT, pressed),
        ControllerInput::AltKey(pressed) => (KeyCode::KEY_LEFTALT, pressed),
        ControllerInput::MetaKey(pressed) => (KeyCode::KEY_LEFTMETA, pressed),

        ControllerInput::UpKey(pressed) => (KeyCode::KEY_UP, pressed),
        ControllerInput::DownKey(pressed) => (KeyCode::KEY_DOWN, pressed),
        ControllerInput::LeftKey(pressed) => (KeyCode::KEY_LEFT, pressed),
        ControllerInput::RightKey(pressed) => (KeyCode::KEY_RIGHT, pressed),

        _ => return None,
    })
}

/// A virtual controller to send button inputs
pub struct VirtualController {
    emulation: EmulatedDevice,
    controller: VirtualDevice,
    mouse: VirtualDevice,
    force_feedback_events: Vec<ControllerOutput>,
    smooth_wheel_error: (f32, f32),
}

impl VirtualController {
    /// create new virtual controller
    pub fn new(emulation: EmulatedDevice) -> io::Result<VirtualController> {
        let (vid, pid, name) = emulated_device_info(emulation);

        let mut controller_keys = AttributeSet::<KeyCode>::new();
        controller_keys.insert(KeyCode::BTN_NORTH);
        controller_keys.insert(KeyCode::BTN_EAST);
        controller_keys.insert(KeyCode::BTN_SOUTH);
        controller_keys.insert(KeyCode::BTN_WEST);
        controller_keys.insert(KeyCode::BTN_TL);
        controller_keys.insert(KeyCode::BTN_TR);
        controller_keys.insert(KeyCode::BTN_SELECT);
        controller_keys.insert(KeyCode::BTN_START);
        controller_keys.insert(KeyCode::BTN_THUMBL);
        controller_keys.insert(KeyCode::BTN_THUMBR);
        match emulation {
            EmulatedDevice::Xbox360 => {}
            _ => {
                controller_keys.insert(KeyCode::BTN_DPAD_UP);
                controller_keys.insert(KeyCode::BTN_DPAD_DOWN);
                controller_keys.insert(KeyCode::BTN_DPAD_LEFT);
                controller_keys.insert(KeyCode::BTN_DPAD_RIGHT);

                controller_keys.insert(KeyCode::BTN_MODE);

                controller_keys.insert(KeyCode::BTN_TL2);
                controller_keys.insert(KeyCode::BTN_TR2);
            }
        }
        match emulation {
            EmulatedDevice::XboxElite | EmulatedDevice::DualSenseEdge => {
                controller_keys.insert(KeyCode(BTN_GRIPL));
                controller_keys.insert(KeyCode(BTN_GRIPL2));
                controller_keys.insert(KeyCode(BTN_GRIPR));
                controller_keys.insert(KeyCode(BTN_GRIPR2));
            }
            _ => {}
        }
        // let mut cont_ff = AttributeSet::<FFEffectCode>::new();
        // cont_ff.insert(FFEffectCode::FF_RUMBLE);
        // cont_ff.insert(FFEffectCode::FF_GAIN);

        let controller = VirtualDevice::builder()?
            .name(name)
            .input_id(InputId::new(BusType::BUS_USB, vid, pid, 0x0110))
            .with_keys(&controller_keys)?
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_HAT0X,
                AbsInfo::new(0, HAT_LEFT, HAT_RIGHT, 0, 0, 0),
            ))?
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_HAT0Y,
                AbsInfo::new(0, HAT_UP, HAT_DOWN, 0, 0, 0),
            ))?;
        let controller = match emulation {
            EmulatedDevice::DualShock4 => controller
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_X,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_Y,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_Z,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_RZ,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_RX,
                    AbsInfo::new(0, 0, TRIGGER_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_RY,
                    AbsInfo::new(0, 0, TRIGGER_MAX, 0, 0, 0),
                ))?,
            _ => controller
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_X,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_Y,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_RX,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_RY,
                    AbsInfo::new(0, STICK_MIN, STICK_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_Z,
                    AbsInfo::new(0, 0, TRIGGER_MAX, 0, 0, 0),
                ))?
                .with_absolute_axis(&UinputAbsSetup::new(
                    AbsoluteAxisCode::ABS_RZ,
                    AbsInfo::new(0, 0, TRIGGER_MAX, 0, 0, 0),
                ))?,
        };
        let controller = controller
            // .with_ff(&cont_ff)?
            // .with_ff_effects_max(FORCEFEEDBACK_MAX)
            .build()?;

        let mut mouse_keys = AttributeSet::<KeyCode>::new();
        mouse_keys.insert(KeyCode::BTN_LEFT);
        mouse_keys.insert(KeyCode::BTN_MIDDLE);
        mouse_keys.insert(KeyCode::BTN_RIGHT);
        mouse_keys.insert(KeyCode::KEY_ENTER);
        mouse_keys.insert(KeyCode::KEY_ESC);
        mouse_keys.insert(KeyCode::KEY_LEFTCTRL);
        mouse_keys.insert(KeyCode::KEY_LEFTSHIFT);
        mouse_keys.insert(KeyCode::KEY_LEFTALT);
        mouse_keys.insert(KeyCode::KEY_LEFTMETA);
        mouse_keys.insert(KeyCode::KEY_UP);
        mouse_keys.insert(KeyCode::KEY_DOWN);
        mouse_keys.insert(KeyCode::KEY_LEFT);
        mouse_keys.insert(KeyCode::KEY_RIGHT);
        let mut mouse_axes = AttributeSet::<RelativeAxisCode>::new();
        mouse_axes.insert(RelativeAxisCode::REL_X);
        mouse_axes.insert(RelativeAxisCode::REL_Y);
        mouse_axes.insert(RelativeAxisCode::REL_HWHEEL_HI_RES);
        mouse_axes.insert(RelativeAxisCode::REL_WHEEL_HI_RES);
        let mouse = VirtualDevice::builder()?
            .name(MOUSE_NAME)
            .with_keys(&mouse_keys)?
            .with_relative_axes(&mouse_axes)?
            .build()?;

        Ok(Self {
            emulation,
            controller,
            mouse,
            force_feedback_events: vec![],
            smooth_wheel_error: (0.0, 0.0),
        })
    }
}

impl AbstractVirtualController for VirtualController {
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()> {
        let mut cevents = Vec::new();
        let mut mevents = Vec::new();
        match input {
            ControllerInput::Up(_) => {
                let (axis, value) = map_absolute_controller_input(self.emulation, input).unwrap();
                cevents.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value));
                if let Some((input, pressed)) = map_digital_controller_input(self.emulation, input)
                {
                    cevents.push(InputEvent::new(EventType::KEY.0, input.0, pressed as i32));
                }
            }
            ControllerInput::Down(_) => {
                let (axis, value) = map_absolute_controller_input(self.emulation, input).unwrap();
                cevents.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value));
                if let Some((input, pressed)) = map_digital_controller_input(self.emulation, input)
                {
                    cevents.push(InputEvent::new(EventType::KEY.0, input.0, pressed as i32));
                }
            }
            ControllerInput::Left(_) => {
                let (axis, value) = map_absolute_controller_input(self.emulation, input).unwrap();
                cevents.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value));
                if let Some((input, pressed)) = map_digital_controller_input(self.emulation, input)
                {
                    cevents.push(InputEvent::new(EventType::KEY.0, input.0, pressed as i32));
                }
            }
            ControllerInput::Right(_) => {
                let (axis, value) = map_absolute_controller_input(self.emulation, input).unwrap();
                cevents.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value));
                if let Some((input, pressed)) = map_digital_controller_input(self.emulation, input)
                {
                    cevents.push(InputEvent::new(EventType::KEY.0, input.0, pressed as i32));
                }
            }

            ControllerInput::Wheel(x, y) => {
                mevents.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_HWHEEL_HI_RES.0,
                    x * SMOOTH_WHEEL_MULTIPLIER,
                ));
                mevents.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_WHEEL_HI_RES.0,
                    y * SMOOTH_WHEEL_MULTIPLIER,
                ));
            }
            ControllerInput::SmoothWheel(x, y) => {
                let x = x * (SMOOTH_WHEEL_MULTIPLIER as f32);
                let y = y * (SMOOTH_WHEEL_MULTIPLIER as f32);

                let x = x + self.smooth_wheel_error.0;
                let x_out = x as i32;
                self.smooth_wheel_error.0 = x - (x_out as f32);

                let y = y + self.smooth_wheel_error.1;
                let y_out = y as i32;
                self.smooth_wheel_error.1 = y - (y_out as f32);

                mevents.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_HWHEEL_HI_RES.0,
                    x_out,
                ));
                mevents.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_WHEEL_HI_RES.0,
                    y_out,
                ));
            }
            ControllerInput::Mouse(x, y) => {
                mevents.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_X.0,
                    x,
                ));
                mevents.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_Y.0,
                    -y,
                ));
            }

            _ => {
                if let Some((axis, value)) = map_absolute_controller_input(self.emulation, input) {
                    cevents.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value));
                } else if let Some(((x_axis, x_value), (y_axis, y_value))) =
                    map_absolute_controller_input_2d(self.emulation, input)
                {
                    cevents.push(InputEvent::new(EventType::ABSOLUTE.0, x_axis.0, x_value));
                    cevents.push(InputEvent::new(EventType::ABSOLUTE.0, y_axis.0, y_value));
                } else if let Some((input, pressed)) =
                    map_digital_controller_input(self.emulation, input)
                {
                    cevents.push(InputEvent::new(EventType::KEY.0, input.0, pressed as i32));
                } else if let Some((input, pressed)) = map_digital_mouse_input(input) {
                    mevents.push(InputEvent::new(EventType::KEY.0, input.0, pressed as i32));
                } else {
                    debug_println!("Ignoring input {input:?}");
                };
            }
        }
        self.controller.emit(&cevents)?;
        self.mouse.emit(&mevents)?;
        Ok(())
    }

    fn recv_output(&mut self) -> anyhow::Result<Vec<ControllerOutput>> {
        Ok(self.force_feedback_events.drain(..).collect())
    }
}
