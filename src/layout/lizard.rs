use crate::devices::{DeviceCommand, DeviceOutput, DeviceProperties, HapticEvent, HapticSide};
use crate::layout::{ControllerLayout, LayoutState, MappedOutput};
use crate::virtual_controller::ControllerInput;

pub struct LizardLayout {}

impl ControllerLayout for LizardLayout {
    fn map_device_output(
        state: &mut LayoutState,
        input: DeviceOutput,
        device_properties: &DeviceProperties,
    ) -> Vec<MappedOutput> {
        match input {
            DeviceOutput::A(p) => vec![MappedOutput::Controller(ControllerInput::EnterKey(p))],
            DeviceOutput::B(p) => vec![MappedOutput::Controller(ControllerInput::EscKey(p))],
            DeviceOutput::X(_) => vec![],
            DeviceOutput::Y(_) => vec![],

            DeviceOutput::Up(p) => vec![MappedOutput::Controller(ControllerInput::UpKey(p))],
            DeviceOutput::Down(p) => vec![MappedOutput::Controller(ControllerInput::DownKey(p))],
            DeviceOutput::Left(p) => vec![MappedOutput::Controller(ControllerInput::LeftKey(p))],
            DeviceOutput::Right(p) => vec![MappedOutput::Controller(ControllerInput::RightKey(p))],

            DeviceOutput::Menu(_) => vec![],
            DeviceOutput::View(_) => vec![],
            DeviceOutput::Steam(_) => vec![],
            DeviceOutput::QuickAccessMenu(p) => {
                if !p {
                    vec![MappedOutput::Device(DeviceCommand::SwitchLayout(
                        device_properties.controller_layout,
                    ))]
                } else {
                    vec![]
                }
            }

            DeviceOutput::LeftJoyStick(x, y) => state.wheel.joystick((x, y)).map_or_else(
                || vec![],
                |(x, y)| vec![MappedOutput::Controller(ControllerInput::SmoothWheel(x, y))],
            ),
            DeviceOutput::LeftJoyStickTouch(_) => vec![],

            DeviceOutput::RightJoyStick(x, y) => state.mouse.joystick((x, y)).map_or_else(
                || vec![],
                |(x, y)| vec![MappedOutput::Controller(ControllerInput::Mouse(x, y))],
            ),
            DeviceOutput::RightJoyStickTouch(_) => vec![],

            DeviceOutput::L1(_) => vec![],
            DeviceOutput::L2(p) => vec![MappedOutput::Controller(ControllerInput::RightMouse(p))],
            DeviceOutput::L3(_) => vec![],
            DeviceOutput::L4(_) => vec![],
            DeviceOutput::L5(_) => vec![],

            DeviceOutput::LeftTrigger(_) => vec![],
            DeviceOutput::LeftGripTouch(_) => vec![],

            DeviceOutput::R1(_) => vec![],
            DeviceOutput::R2(p) => vec![MappedOutput::Controller(ControllerInput::LeftMouse(p))],
            DeviceOutput::R3(p) => vec![MappedOutput::Controller(ControllerInput::MiddleMouse(p))],
            DeviceOutput::R4(_) => vec![],
            DeviceOutput::R5(_) => vec![],

            DeviceOutput::RightTrigger(_) => vec![],
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
