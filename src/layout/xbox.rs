use crate::devices::{DeviceCommand, DeviceOutput, DeviceProperties, HapticEvent, HapticSide};
use crate::layout::{ControllerLayout, Layout, LayoutState, MappedOutput};
use crate::virtual_controller::ControllerInput;

pub struct XboxLayout {}

impl ControllerLayout for XboxLayout {
    fn map_device_output(
        state: &mut LayoutState,
        input: DeviceOutput,
        _device_properties: &DeviceProperties,
    ) -> Vec<MappedOutput> {
        match input {
            DeviceOutput::A(p) => vec![MappedOutput::Controller(ControllerInput::A(p))],
            DeviceOutput::B(p) => vec![MappedOutput::Controller(ControllerInput::B(p))],
            DeviceOutput::X(p) => vec![MappedOutput::Controller(ControllerInput::X(p))],
            DeviceOutput::Y(p) => vec![MappedOutput::Controller(ControllerInput::Y(p))],

            DeviceOutput::Up(p) => vec![MappedOutput::Controller(ControllerInput::Up(p))],
            DeviceOutput::Down(p) => vec![MappedOutput::Controller(ControllerInput::Down(p))],
            DeviceOutput::Left(p) => vec![MappedOutput::Controller(ControllerInput::Left(p))],
            DeviceOutput::Right(p) => vec![MappedOutput::Controller(ControllerInput::Right(p))],

            DeviceOutput::Menu(p) => vec![MappedOutput::Controller(ControllerInput::Start(p))],
            DeviceOutput::View(p) => vec![MappedOutput::Controller(ControllerInput::Select(p))],
            DeviceOutput::Steam(p) => vec![MappedOutput::Controller(ControllerInput::Mode(p))],
            DeviceOutput::QuickAccessMenu(p) => {
                if p {
                    vec![MappedOutput::Device(DeviceCommand::SwitchLayout(
                        Layout::Lizard,
                    ))]
                } else {
                    vec![]
                }
            }

            DeviceOutput::LeftJoyStick(x, y) => vec![MappedOutput::Controller(
                ControllerInput::LeftJoyStick(x, y),
            )],
            DeviceOutput::LeftJoyStickTouch(_) => vec![],

            DeviceOutput::RightJoyStick(x, y) => vec![MappedOutput::Controller(
                ControllerInput::RightJoyStick(x, y),
            )],
            DeviceOutput::RightJoyStickTouch(_) => vec![],

            DeviceOutput::L1(p) => vec![MappedOutput::Controller(ControllerInput::LeftBumper(p))],
            DeviceOutput::L2(p) => vec![MappedOutput::Controller(
                ControllerInput::LeftTriggerClick(p),
            )],
            DeviceOutput::L3(p) => vec![MappedOutput::Controller(ControllerInput::LeftThumb(p))],
            DeviceOutput::L4(p) => vec![MappedOutput::Controller(ControllerInput::LeftGrip1(p))],
            DeviceOutput::L5(p) => vec![MappedOutput::Controller(ControllerInput::LeftGrip2(p))],

            DeviceOutput::LeftTrigger(f) => {
                vec![MappedOutput::Controller(ControllerInput::LeftTrigger(f))]
            }
            DeviceOutput::LeftGripTouch(_) => vec![],

            DeviceOutput::R1(p) => vec![MappedOutput::Controller(ControllerInput::RightBumper(p))],
            DeviceOutput::R2(p) => vec![MappedOutput::Controller(
                ControllerInput::RightTriggerClick(p),
            )],
            DeviceOutput::R3(p) => vec![MappedOutput::Controller(ControllerInput::RightThumb(p))],
            DeviceOutput::R4(p) => vec![MappedOutput::Controller(ControllerInput::RightGrip1(p))],
            DeviceOutput::R5(p) => vec![MappedOutput::Controller(ControllerInput::RightGrip2(p))],

            DeviceOutput::RightTrigger(f) => {
                vec![MappedOutput::Controller(ControllerInput::RightTrigger(f))]
            }
            DeviceOutput::RightGripTouch(_) => vec![],

            DeviceOutput::LeftTrackpad(x, y, z) => {
                state.wheel.notched_trackpad((x, y, z)).map_or_else(
                    || vec![],
                    |(x, y, h)| {
                        let mut result =
                            vec![MappedOutput::Controller(ControllerInput::Wheel(x, y))];
                        if h {
                            result.push(MappedOutput::Device(DeviceCommand::Haptic(HapticEvent {
                                pad: HapticSide::Left,
                                duration: 0x000A,
                                interval: 0x0000,
                                count: 0x0001,
                                gain: 0xff,
                            })));
                        }
                        result
                    },
                )
            }
            DeviceOutput::LeftTrackpadClick(_) => vec![],
            DeviceOutput::LeftTrackpadTouch(_) => {
                state.wheel.reset_trackpad();
                vec![]
            }

            DeviceOutput::RightTrackpad(x, y, z) => state.mouse.trackpad((x, y, z)).map_or_else(
                || vec![],
                |(x, y, h)| {
                    let mut result = vec![MappedOutput::Controller(ControllerInput::Mouse(x, y))];
                    if h {
                        result.push(MappedOutput::Device(DeviceCommand::Haptic(HapticEvent {
                            pad: HapticSide::Right,
                            duration: 0x000A,
                            interval: 0x0000,
                            count: 0x0001,
                            gain: 0xff,
                        })));
                    }
                    result
                },
            ),
            DeviceOutput::RightTrackpadClick(p) => {
                state.mouse.reset_trackpad();
                vec![
                    MappedOutput::Controller(ControllerInput::LeftMouse(p)),
                    MappedOutput::Device(DeviceCommand::Haptic(HapticEvent {
                        pad: HapticSide::Right,
                        duration: 0x000A,
                        interval: 0x0000,
                        count: 0x0001,
                        gain: 0xff,
                    })),
                ]
            }
            DeviceOutput::RightTrackpadTouch(_) => {
                state.mouse.reset_trackpad();
                vec![]
            }
        }
    }
}
