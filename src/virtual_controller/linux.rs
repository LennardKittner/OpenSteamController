use std::io;
use std::ops::Neg;

use crate::debug_println;
use crate::devices::{HapticEvent, HapticSide};
use crate::virtual_controller::{AbstractVirtualController, ControllerInput, ControllerOutput};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    RelativeAxisCode, UinputAbsSetup,
};

/// Xbox series x
const PRODUCT_ID: u16 = 0x0b12;
/// Xbox 360
const PRODUCT_ID_XINPUT: u16 = 0x028e;
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
) -> Option<(KeyCode, bool)> {
    Some(match (xinput_mode, input) {
        (_, ControllerInput::A(pressed)) => (KeyCode::BTN_SOUTH, pressed),
        (_, ControllerInput::B(pressed)) => (KeyCode::BTN_EAST, pressed),
        (false, ControllerInput::C(pressed)) => (KeyCode::BTN_C, pressed),
        // Mapping bug: the Linux kernel defines BTN_A/B/X/Y as aliases for
        // BTN_SOUTH/EAST/NORTH/WEST:
        //  X
        // Y B
        //  A
        // However, this is *not* the same as the Xbox layout:
        //  Y
        // X B
        //  A
        // Despite this, it would seem most default mappings assume the
        // Xbox layout *is* the same as the Linux layout, meaning we need to
        // deliberately reverse X and Y for as long as we pretend to be an
        // Xbox controller.
        (_, ControllerInput::X(pressed)) => (KeyCode::BTN_NORTH, pressed),
        (_, ControllerInput::Y(pressed)) => (KeyCode::BTN_WEST, pressed),
        (false, ControllerInput::Z(pressed)) => (KeyCode::BTN_Z, pressed),
        (_, ControllerInput::LeftBumper(pressed)) => (KeyCode::BTN_TL, pressed),
        (_, ControllerInput::RightBumper(pressed)) => (KeyCode::BTN_TR, pressed),
        (_, ControllerInput::Select(pressed)) => (KeyCode::BTN_SELECT, pressed),
        (_, ControllerInput::Start(pressed)) => (KeyCode::BTN_START, pressed),
        (false, ControllerInput::Mode(pressed)) => (KeyCode::BTN_MODE, pressed),
        (_, ControllerInput::LeftThumb(pressed)) => (KeyCode::BTN_THUMBL, pressed),
        (_, ControllerInput::RightThumb(pressed)) => (KeyCode::BTN_THUMBR, pressed),
        (false, ControllerInput::Menu(pressed)) => (KeyCode::BTN_MODE, pressed),
        (false, ControllerInput::Home(pressed)) => (KeyCode::BTN_MODE, pressed),
        (false, ControllerInput::Up(pressed)) => (KeyCode::BTN_DPAD_UP, pressed),
        (false, ControllerInput::Down(pressed)) => (KeyCode::BTN_DPAD_DOWN, pressed),
        (false, ControllerInput::Left(pressed)) => (KeyCode::BTN_DPAD_LEFT, pressed),
        (false, ControllerInput::Right(pressed)) => (KeyCode::BTN_DPAD_RIGHT, pressed),
        _ => return None,
    })
}

fn map_digital_mouse_input(input: ControllerInput) -> Option<(KeyCode, bool)> {
    #[allow(unreachable_code)]
    Some(match input {
        // TODO
        _ => return None,
    })
}

/// A virtual controller to send button inputs
pub struct VirtualController {
    xinput_mode: bool,
    controller: VirtualDevice,
    mouse: VirtualDevice,
    force_feedback_events: Vec<ControllerOutput>,
    left_trackpad_prev: Option<(f32, f32, f32)>,
    right_trackpad_prev: Option<(f32, f32, f32)>,
    mouse_feedback: f32,
}

/// Stick all the way in one direction
const STICK_MIN: i32 = -32768;
/// Stick all the way in the other direction
const STICK_MAX: i32 = 32767;
/// Trigger pressed down
const TRIGGER_MAX: i32 = 255;

// const FORCEFEEDBACK_MAX: u32 = 16;

const MOUSE_SPEED: f32 = 256.0;
const MOUSE_FEEDBACK_MAX: f32 = MOUSE_SPEED / 4.0;
const WHEEL_SPEED: f32 = -64.0;

const HAT_NONE: i32 = 0;
const HAT_LEFT: i32 = -1;
const HAT_RIGHT: i32 = 1;
const HAT_UP: i32 = -1;
const HAT_DOWN: i32 = 1;

impl VirtualController {
    /// create new virtual controller
    pub fn new(xinput_mode: bool) -> io::Result<VirtualController> {
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
        if !xinput_mode {
            controller_keys.insert(KeyCode::BTN_TL2);
            controller_keys.insert(KeyCode::BTN_TR2);
            controller_keys.insert(KeyCode::BTN_MODE);
            controller_keys.insert(KeyCode::BTN_DPAD_UP);
            controller_keys.insert(KeyCode::BTN_DPAD_DOWN);
            controller_keys.insert(KeyCode::BTN_DPAD_LEFT);
            controller_keys.insert(KeyCode::BTN_DPAD_RIGHT);
        }
        // let mut cont_ff = AttributeSet::<FFEffectCode>::new();
        // cont_ff.insert(FFEffectCode::FF_RUMBLE);
        // cont_ff.insert(FFEffectCode::FF_GAIN);

        let controller = VirtualDevice::builder()?
            .name(NAME)
            .input_id(InputId::new(
                BusType::BUS_USB,
                VENDOR_ID,
                if xinput_mode {
                    PRODUCT_ID_XINPUT
                } else {
                    PRODUCT_ID
                },
                0x0110,
            ))
            .with_keys(&controller_keys)?
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_HAT0X,
                AbsInfo::new(0, HAT_LEFT, HAT_RIGHT, 0, 0, 0),
            ))?
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_HAT0Y,
                AbsInfo::new(0, HAT_UP, HAT_DOWN, 0, 0, 0),
            ))?
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
            ))?
            // .with_ff(&cont_ff)?
            // .with_ff_effects_max(FORCEFEEDBACK_MAX)
            .build()?;

        let mut mouse_keys = AttributeSet::<KeyCode>::new();
        mouse_keys.insert(KeyCode::BTN_LEFT);
        mouse_keys.insert(KeyCode::BTN_MIDDLE);
        mouse_keys.insert(KeyCode::BTN_RIGHT);
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

        let force_feedback_events = Vec::new();
        let left_trackpad_prev: Option<(f32, f32, f32)> = None;
        let right_trackpad_prev: Option<(f32, f32, f32)> = None;
        let mouse_feedback: f32 = 0.0;
        Ok(Self {
            xinput_mode,
            controller,
            mouse,
            force_feedback_events,
            left_trackpad_prev,
            right_trackpad_prev,
            mouse_feedback,
        })
    }

    /// helper for digital inputs
    fn perform_digital_input(
        &mut self,
        input: ControllerInput,
        controller: &mut Vec<InputEvent>,
        mouse: &mut Vec<InputEvent>,
    ) {
        if let Some((input, pressed)) = map_digital_controller_input(self.xinput_mode, input) {
            controller.push(InputEvent::new(
                EventType::KEY.0,
                input.0,
                if pressed { 1 } else { 0 },
            ));
        } else if let Some((input, pressed)) = map_digital_mouse_input(input) {
            mouse.push(InputEvent::new(
                EventType::KEY.0,
                input.0,
                if pressed { 1 } else { 0 },
            ));
        } else {
            debug_println!("Ignoring input {input:?}");
        };
    }
}

impl AbstractVirtualController for VirtualController {
    fn send_input(&mut self, input: ControllerInput) -> anyhow::Result<()> {
        let mut cevents = Vec::new();
        let mut mevents = Vec::new();
        match input {
            ControllerInput::Left(pressed) => {
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_HAT0X.0,
                    if pressed { HAT_LEFT } else { HAT_NONE },
                ));
                if !self.xinput_mode {
                    self.perform_digital_input(input, &mut cevents, &mut mevents);
                }
            }
            ControllerInput::Right(pressed) => {
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_HAT0X.0,
                    if pressed { HAT_RIGHT } else { HAT_NONE },
                ));
                if !self.xinput_mode {
                    self.perform_digital_input(input, &mut cevents, &mut mevents);
                }
            }
            ControllerInput::Up(pressed) => {
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_HAT0Y.0,
                    if pressed { HAT_UP } else { HAT_NONE },
                ));
                if !self.xinput_mode {
                    self.perform_digital_input(input, &mut cevents, &mut mevents);
                }
            }
            ControllerInput::Down(pressed) => {
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_HAT0Y.0,
                    if pressed { HAT_DOWN } else { HAT_NONE },
                ));
                if !self.xinput_mode {
                    self.perform_digital_input(input, &mut cevents, &mut mevents);
                }
            }
            ControllerInput::LeftTrackpadTouch(_) => {
                self.left_trackpad_prev = None;
            }
            ControllerInput::RightTrackpadTouch(_) => {
                self.right_trackpad_prev = None;
                self.mouse_feedback = 0.0;
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

                    if x_out.abs() > 0 || y_out.abs() > 0 {
                        self.force_feedback_events
                            .push(ControllerOutput::Haptic(HapticEvent {
                                pad: HapticSide::Left,
                                duration: 0x000A,
                                interval: 0x0000,
                                count: 0x0001,
                                gain: 0xff,
                            }));
                    }

                    mevents.push(InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_HWHEEL_HI_RES.0,
                        x_out * 120,
                    ));
                    mevents.push(InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_WHEEL_HI_RES.0,
                        y_out * 120,
                    ));
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

                    let mouse_dist =
                        ((x_out as f32 * x_out as f32) + (y_out as f32 * y_out as f32)).sqrt();
                    self.mouse_feedback += mouse_dist;
                    if self.mouse_feedback >= MOUSE_FEEDBACK_MAX {
                        self.force_feedback_events
                            .push(ControllerOutput::Haptic(HapticEvent {
                                pad: HapticSide::Right,
                                duration: 0x000A,
                                interval: 0x0000,
                                count: 0x0001,
                                gain: 0xff,
                            }));
                        self.mouse_feedback %= MOUSE_FEEDBACK_MAX;
                    }

                    let y_out = y_out.neg();
                    mevents.push(InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_X.0,
                        x_out,
                    ));
                    mevents.push(InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_Y.0,
                        y_out,
                    ));
                } else {
                    self.right_trackpad_prev = Some((x, y, z));
                }
            }
            ControllerInput::RightTrackpadClick(pressed) => {
                self.force_feedback_events
                    .push(ControllerOutput::Haptic(HapticEvent {
                        pad: HapticSide::Right,
                        duration: 0x000A,
                        interval: 0x0000,
                        count: 0x0001,
                        gain: 0xff,
                    }));
                mevents.push(InputEvent::new(
                    EventType::KEY.0,
                    KeyCode::BTN_LEFT.0,
                    if pressed { 1 } else { 0 },
                ));
                self.mouse_feedback = 0.0;
                self.perform_digital_input(input, &mut cevents, &mut mevents);
            }
            ControllerInput::RightJoyStick(x, y) => {
                let y = y.neg();
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_RX.0,
                    if x.is_sign_positive() {
                        x * STICK_MAX as f32
                    } else {
                        x.neg() * STICK_MIN as f32
                    } as i32,
                ));
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_RY.0,
                    if y.is_sign_positive() {
                        y * STICK_MAX as f32
                    } else {
                        y.neg() * STICK_MIN as f32
                    } as i32,
                ));
            }
            ControllerInput::LeftJoyStick(x, y) => {
                let y = y.neg();
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_X.0,
                    if x.is_sign_positive() {
                        x * STICK_MAX as f32
                    } else {
                        x.neg() * STICK_MIN as f32
                    } as i32,
                ));
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_Y.0,
                    if y.is_sign_positive() {
                        y * STICK_MAX as f32
                    } else {
                        y.neg() * STICK_MIN as f32
                    } as i32,
                ));
            }
            ControllerInput::LeftTrigger(force) => {
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_Z.0,
                    if force.is_sign_positive() {
                        force * TRIGGER_MAX as f32
                    } else {
                        0f32
                    } as i32,
                ));
            }
            ControllerInput::RightTrigger(force) => {
                cevents.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_RZ.0,
                    if force.is_sign_positive() {
                        force * TRIGGER_MAX as f32
                    } else {
                        0f32
                    } as i32,
                ));
            }
            digital_input => {
                self.perform_digital_input(digital_input, &mut cevents, &mut mevents);
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
