#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use open_steam_controller::debug_println;
use open_steam_controller::devices::{
    Controller, DeviceButton, DeviceCommand, DeviceLayout, DeviceOutput, DeviceProperties,
};
use open_steam_controller::multi_threading::{self, ControllerReceiver};
use open_steam_controller::virtual_controller::ControllerInput;

#[cfg(target_os = "linux")]
mod status_tray;

#[cfg(not(target_os = "linux"))]
mod status_tray_not_linux;

#[cfg(target_os = "windows")]
mod tray_battery_icon_state;

#[cfg(not(target_os = "linux"))]
fn main() {
    use clap::ArgAction;
    use open_steam_controller::devices::{count_compatible_devices, DeviceCommand};
    use open_steam_controller::multi_threading::ControllerSender;
    use std::sync::mpsc;
    use std::thread::JoinHandle;

    use crate::status_tray_not_linux::TrayApp;
    use open_steam_controller::devices::connect_compatible_devices;
    use open_steam_controller::devices::DeviceProperties;
    use open_steam_controller::VERBOSE;
    use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};

    let event_loop: EventLoop<Vec<DeviceProperties>> =
        EventLoop::with_user_event().build().unwrap();
    let proxy: EventLoopProxy<Vec<DeviceProperties>> = event_loop.create_proxy();
    event_loop.set_control_flow(ControlFlow::Wait);

    use std::time::Duration;

    use clap::{Arg, Command};

    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .disable_version_flag(false)
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A tray application for monitoring the new Steam Controller")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .action(ArgAction::SetTrue)
                .required(false)
                .help("Use verbose output "),
        )
        .get_matches();

    VERBOSE.set(matches.get_flag("verbose")).unwrap();

    let (tx, rx) = mpsc::channel::<(u32, DeviceCommand)>();

    std::thread::spawn(move || {
        loop {
            let exit = Arc::new(AtomicBool::new(false));

            let devices = loop {
                match connect_compatible_devices() {
                    Ok(d) => break d,
                    Err(e) => {
                        let _ = proxy.send_event(Vec::new());
                        eprintln!("Connecting failed with error: {e}");
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            };

            let device_interface_count = count_compatible_devices().unwrap_or(u32::MAX);

            let mut controller_threads = devices
                .into_iter()
                .map(|device| {
                    let (device_tx, device_rx) = multi_threading::create_controller_channel();
                    let local_exit = exit.clone();
                    (
                        std::thread::spawn(move || {
                            controller_handler(device, device_rx, local_exit);
                        }),
                        device_tx,
                    )
                })
                .collect::<Vec<(JoinHandle<()>, ControllerSender)>>();

            // Run loop
            loop {
                // this behaves like a iter_timeout
                let first = rx.recv_timeout(Duration::from_millis(500));
                for (device_id, command) in first.into_iter().chain(rx.try_iter()) {
                    controller_threads[device_id as usize]
                        .1
                        .send_command(command);
                }

                let mut states = controller_threads
                    .iter()
                    .map(|d| d.1.get_latest_properties())
                    .enumerate()
                    .filter_map(|(device_id, p)| {
                        if device_id == 0 || p.connected == Some(true) {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<DeviceProperties>>();
                // in case there are more than one we can discard controller 0 if not connected
                if states.len() != 1 {
                    states.retain(|s| s.connected == Some(true));
                }

                let _ = proxy.send_event(states);
                // if a new puck or controller is connected or disconnected
                if count_compatible_devices().unwrap_or(u32::MAX) != device_interface_count
                    || controller_threads.iter().any(|t| t.0.is_finished())
                {
                    exit.store(true, std::sync::atomic::Ordering::Relaxed);
                    controller_threads.drain(..).for_each(|t| {
                        t.0.join().unwrap();
                    });
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });

    event_loop.run_app(&mut TrayApp::new(tx)).unwrap();
}

/// Trackpad as mouse speed multiplier
const TRACKPAD_MOUSE_SPEED: f32 = 256.0;
/// Trackpad as wheel speed multiplier
const TRACKPAD_WHEEL_SPEED: f32 = -64.0;
/// Joystick as mouse speed multiplier
const STICK_MOUSE_SPEED: f32 = 8.0;
/// Joystick as mouse deadzone
const STICK_MOUSE_DEADZONE: f32 = 0.025;
/// Joystick as wheel speed multiplier
const STICK_WHEEL_SPEED: f32 = 1.0;

enum MappedOutput {
    Device(DeviceCommand),
    Controller(ControllerInput),
}

#[derive(Debug, Clone)]
struct LayoutState {
    pub buttons_prev: DeviceButton,
    pub left_trackpad_prev: Option<(f32, f32, f32)>,
    pub right_trackpad_prev: Option<(f32, f32, f32)>,
}

trait ControllerLayout {
    fn map_device_output(
        input: DeviceOutput,
        state: &mut LayoutState,
        device_properties: &DeviceProperties,
    ) -> Vec<MappedOutput>;
}

struct XboxLayout {}
struct NintendoLayout {}
struct LizardLayout {}

impl ControllerLayout for XboxLayout {
    fn map_device_output(
        input: DeviceOutput,
        state: &mut LayoutState,
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
                        DeviceLayout::Lizard,
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
                let prev = state.left_trackpad_prev;
                state.left_trackpad_prev = Some((x, y, z));
                if let Some((prev_x, prev_y, _prev_z)) = prev {
                    let x = x - prev_x;
                    let y = y - prev_y;
                    vec![MappedOutput::Controller(ControllerInput::Wheel(
                        x * TRACKPAD_WHEEL_SPEED,
                        y * TRACKPAD_WHEEL_SPEED,
                    ))]
                } else {
                    state.left_trackpad_prev = Some((x, y, z));
                    vec![]
                }
            }
            DeviceOutput::LeftTrackpadClick(_) => vec![],
            DeviceOutput::LeftTrackpadTouch(_) => {
                state.left_trackpad_prev = None;
                vec![]
            }

            DeviceOutput::RightTrackpad(x, y, z) => {
                let prev = state.right_trackpad_prev;
                state.right_trackpad_prev = Some((x, y, z));
                if let Some((prev_x, prev_y, _prev_z)) = prev {
                    let x = x - prev_x;
                    let y = y - prev_y;
                    vec![MappedOutput::Controller(ControllerInput::Mouse(
                        x * TRACKPAD_MOUSE_SPEED,
                        y * -TRACKPAD_MOUSE_SPEED,
                    ))]
                } else {
                    state.right_trackpad_prev = Some((x, y, z));
                    vec![]
                }
            }
            DeviceOutput::RightTrackpadClick(p) => {
                vec![MappedOutput::Controller(ControllerInput::LeftMouse(p))]
            }
            DeviceOutput::RightTrackpadTouch(_) => {
                state.right_trackpad_prev = None;
                vec![]
            }
        }
    }
}

impl ControllerLayout for NintendoLayout {
    fn map_device_output(
        input: DeviceOutput,
        state: &mut LayoutState,
        _device_properties: &DeviceProperties,
    ) -> Vec<MappedOutput> {
        match input {
            DeviceOutput::A(p) => vec![MappedOutput::Controller(ControllerInput::B(p))],
            DeviceOutput::B(p) => vec![MappedOutput::Controller(ControllerInput::A(p))],
            DeviceOutput::X(p) => vec![MappedOutput::Controller(ControllerInput::Y(p))],
            DeviceOutput::Y(p) => vec![MappedOutput::Controller(ControllerInput::X(p))],

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
                        DeviceLayout::Lizard,
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
                let prev = state.left_trackpad_prev;
                state.left_trackpad_prev = Some((x, y, z));
                if let Some((prev_x, prev_y, _prev_z)) = prev {
                    let x = x - prev_x;
                    let y = y - prev_y;
                    vec![MappedOutput::Controller(ControllerInput::Wheel(
                        x * TRACKPAD_WHEEL_SPEED,
                        y * TRACKPAD_WHEEL_SPEED,
                    ))]
                } else {
                    state.left_trackpad_prev = Some((x, y, z));
                    vec![]
                }
            }
            DeviceOutput::LeftTrackpadClick(_) => vec![],
            DeviceOutput::LeftTrackpadTouch(_) => {
                state.left_trackpad_prev = None;
                vec![]
            }

            DeviceOutput::RightTrackpad(x, y, z) => {
                let prev = state.right_trackpad_prev;
                state.right_trackpad_prev = Some((x, y, z));
                if let Some((prev_x, prev_y, _prev_z)) = prev {
                    let x = x - prev_x;
                    let y = y - prev_y;
                    vec![MappedOutput::Controller(ControllerInput::Mouse(
                        x * TRACKPAD_MOUSE_SPEED,
                        y * -TRACKPAD_MOUSE_SPEED,
                    ))]
                } else {
                    state.right_trackpad_prev = Some((x, y, z));
                    vec![]
                }
            }
            DeviceOutput::RightTrackpadClick(p) => {
                vec![MappedOutput::Controller(ControllerInput::LeftMouse(p))]
            }
            DeviceOutput::RightTrackpadTouch(_) => {
                state.right_trackpad_prev = None;
                vec![]
            }
        }
    }
}

impl ControllerLayout for LizardLayout {
    fn map_device_output(
        input: DeviceOutput,
        state: &mut LayoutState,
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

            DeviceOutput::LeftJoyStick(x, y) => {
                let x2 = x * x;
                let y2 = y * y;
                if (x2 + y2) > STICK_MOUSE_DEADZONE {
                    vec![MappedOutput::Controller(ControllerInput::Wheel(
                        x2 * x.signum() * STICK_WHEEL_SPEED,
                        y2 * y.signum() * STICK_WHEEL_SPEED,
                    ))]
                } else {
                    vec![]
                }
            }
            DeviceOutput::LeftJoyStickTouch(_) => vec![],

            DeviceOutput::RightJoyStick(x, y) => {
                let x2 = x * x;
                let y2 = y * y;
                if (x2 + y2) > STICK_MOUSE_DEADZONE {
                    vec![MappedOutput::Controller(ControllerInput::Mouse(
                        x2 * x.signum() * STICK_MOUSE_SPEED,
                        y2 * y.signum() * -STICK_MOUSE_SPEED,
                    ))]
                } else {
                    vec![]
                }
            }
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
                let prev = state.left_trackpad_prev;
                state.left_trackpad_prev = Some((x, y, z));
                if let Some((prev_x, prev_y, _prev_z)) = prev {
                    let x = x - prev_x;
                    let y = y - prev_y;
                    vec![MappedOutput::Controller(ControllerInput::Wheel(
                        x * TRACKPAD_WHEEL_SPEED,
                        y * TRACKPAD_WHEEL_SPEED,
                    ))]
                } else {
                    vec![]
                }
            }
            DeviceOutput::LeftTrackpadClick(_) => vec![],
            DeviceOutput::LeftTrackpadTouch(_) => {
                state.left_trackpad_prev = None;
                vec![]
            }

            DeviceOutput::RightTrackpad(x, y, z) => {
                let prev = state.right_trackpad_prev;
                state.right_trackpad_prev = Some((x, y, z));
                if let Some((prev_x, prev_y, _prev_z)) = prev {
                    let x = x - prev_x;
                    let y = y - prev_y;
                    vec![MappedOutput::Controller(ControllerInput::Mouse(
                        x * TRACKPAD_MOUSE_SPEED,
                        y * -TRACKPAD_MOUSE_SPEED,
                    ))]
                } else {
                    vec![]
                }
            }
            DeviceOutput::RightTrackpadClick(p) => {
                vec![MappedOutput::Controller(ControllerInput::LeftMouse(p))]
            }
            DeviceOutput::RightTrackpadTouch(_) => {
                state.right_trackpad_prev = None;
                vec![]
            }
        }
    }
}

fn mode_layout_button_map(
    input: DeviceOutput,
    state: &LayoutState,
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
                        DeviceLayout::Xbox => DeviceLayout::Nintendo,
                        DeviceLayout::Nintendo => DeviceLayout::Xbox,
                        DeviceLayout::Lizard => device_properties.controller_layout,
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
                        DeviceLayout::Lizard => device_properties.controller_layout,
                        _ => DeviceLayout::Lizard,
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

fn event_map(
    input: DeviceOutput,
    state: &mut LayoutState,
    device_properties: &DeviceProperties,
) -> Vec<MappedOutput> {
    if let Some(mode_bindings) = mode_layout_button_map(input, state, device_properties) {
        return mode_bindings;
    }

    match device_properties.layout {
        DeviceLayout::Xbox => XboxLayout::map_device_output(input, state, device_properties),
        DeviceLayout::Nintendo => {
            NintendoLayout::map_device_output(input, state, device_properties)
        }
        DeviceLayout::Lizard => LizardLayout::map_device_output(input, state, device_properties),
    }
}

fn update_bitmap(bitmap: &mut DeviceButton, input: DeviceOutput) {
    match input {
        DeviceOutput::A(p) => bitmap.set(DeviceButton::A, p),
        DeviceOutput::B(p) => bitmap.set(DeviceButton::B, p),
        DeviceOutput::X(p) => bitmap.set(DeviceButton::X, p),
        DeviceOutput::Y(p) => bitmap.set(DeviceButton::Y, p),

        DeviceOutput::Up(p) => bitmap.set(DeviceButton::Up, p),
        DeviceOutput::Down(p) => bitmap.set(DeviceButton::Down, p),
        DeviceOutput::Left(p) => bitmap.set(DeviceButton::Left, p),
        DeviceOutput::Right(p) => bitmap.set(DeviceButton::Right, p),

        DeviceOutput::Menu(p) => bitmap.set(DeviceButton::Menu, p),
        DeviceOutput::View(p) => bitmap.set(DeviceButton::View, p),
        DeviceOutput::Steam(p) => bitmap.set(DeviceButton::Steam, p),
        DeviceOutput::QuickAccessMenu(p) => bitmap.set(DeviceButton::QuickAccessMenu, p),

        DeviceOutput::LeftJoyStick(_, _) => {}
        DeviceOutput::LeftJoyStickTouch(p) => bitmap.set(DeviceButton::LeftJoyStickTouch, p),

        DeviceOutput::RightJoyStick(_, _) => {}
        DeviceOutput::RightJoyStickTouch(p) => bitmap.set(DeviceButton::RightJoyStickTouch, p),

        DeviceOutput::L1(p) => bitmap.set(DeviceButton::L1, p),
        DeviceOutput::L2(p) => bitmap.set(DeviceButton::L2, p),
        DeviceOutput::L3(p) => bitmap.set(DeviceButton::L3, p),
        DeviceOutput::L4(p) => bitmap.set(DeviceButton::L4, p),
        DeviceOutput::L5(p) => bitmap.set(DeviceButton::L5, p),

        DeviceOutput::LeftTrigger(_) => {}
        DeviceOutput::LeftGripTouch(p) => bitmap.set(DeviceButton::LeftGripTouch, p),

        DeviceOutput::R1(p) => bitmap.set(DeviceButton::R1, p),
        DeviceOutput::R2(p) => bitmap.set(DeviceButton::R2, p),
        DeviceOutput::R3(p) => bitmap.set(DeviceButton::R3, p),
        DeviceOutput::R4(p) => bitmap.set(DeviceButton::R4, p),
        DeviceOutput::R5(p) => bitmap.set(DeviceButton::R5, p),

        DeviceOutput::RightTrigger(_) => {}
        DeviceOutput::RightGripTouch(p) => bitmap.set(DeviceButton::RightGripTouch, p),

        DeviceOutput::LeftTrackpad(_, _, _) => {}
        DeviceOutput::LeftTrackpadClick(p) => bitmap.set(DeviceButton::LeftTrackpadClick, p),
        DeviceOutput::LeftTrackpadTouch(p) => bitmap.set(DeviceButton::LeftTrackpadTouch, p),

        DeviceOutput::RightTrackpad(_, _, _) => {}
        DeviceOutput::RightTrackpadClick(p) => bitmap.set(DeviceButton::RightTrackpadClick, p),
        DeviceOutput::RightTrackpadTouch(p) => bitmap.set(DeviceButton::RightTrackpadTouch, p),
    }
}

/// handles packets from a controller and is responsible for generating virtual controller inputs
/// the exit atomic is used for termination
fn controller_handler(
    mut device: Controller,
    mut device_rx: ControllerReceiver,
    exit: Arc<AtomicBool>,
) {
    use open_steam_controller::virtual_controller::{AbstractVirtualController, VirtualController};
    let mut virt_controller: Option<VirtualController> = None;
    let mut state = LayoutState {
        buttons_prev: DeviceButton::from_bits_truncate(0),
        left_trackpad_prev: None,
        right_trackpad_prev: None,
    };
    // Run loop
    let mut run_counter = 0;
    loop {
        match if run_counter % 300 == 0 && device.device_properties().connected == Some(true) {
            device.active_refresh_state()
        } else {
            device.passive_refresh_state()
        } {
            Ok(input_events) => {
                // Let the virtual controllers connection state mirror the real
                // controller
                if device.device_properties().connected == Some(false) && virt_controller.is_some()
                {
                    virt_controller = None;
                } else if device.device_properties().connected == Some(true)
                    && virt_controller.is_none()
                {
                    match VirtualController::new() {
                        Ok(vc) => {
                            virt_controller = Some(vc);
                            state = LayoutState {
                                buttons_prev: DeviceButton::from_bits_truncate(0),
                                left_trackpad_prev: None,
                                right_trackpad_prev: None,
                            }
                        }
                        Err(e) => eprintln!("Failed to create virtual controller {e}"),
                    }
                }

                let props = device.device_properties();
                if let Some(virt_controller) = &mut virt_controller {
                    for input_event in input_events {
                        for event in event_map(input_event, &mut state, &props) {
                            match event {
                                MappedOutput::Controller(button_event) => {
                                    if let Err(e) = virt_controller.send_input(button_event) {
                                        debug_println!("{e}");
                                    }
                                }
                                MappedOutput::Device(command) => {
                                    if let Err(e) = device.try_apply(command) {
                                        debug_println!("{e}");
                                    }
                                }
                            }
                        }
                        update_bitmap(&mut state.buttons_prev, input_event);
                    }
                }
            }
            Err(error) => {
                let mut props = device.device_properties();
                props.connected = None;
                device_rx.try_update_state(&props);
                eprintln!("{error}");
                return;
            }
        };
        for command in device_rx.receive_commands() {
            let _ = device.try_apply(command);
        }
        device_rx.try_update_state(&device.device_properties());
        run_counter += 1;
        if exit.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    use clap::ArgAction;
    use clap::{Arg, Command};
    use open_steam_controller::devices::connect_compatible_devices;
    use open_steam_controller::devices::count_compatible_devices;
    use open_steam_controller::devices::DeviceProperties;
    use open_steam_controller::multi_threading::ControllerSender;
    use open_steam_controller::VERBOSE;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use status_tray::{StatusTray, TrayHandler};

    if let Ok(name) = std::env::current_exe() {
        if let Some(name) = name.to_str() {
            if let Ok(askpass) = std::env::var("SUDO_ASKPASS") {
                if name == askpass {
                    open_steam_controller::act_as_askpass_handler();
                }
            }
        }
    }
    open_steam_controller::prompt_user_for_udev_rule();
    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .disable_version_flag(false)
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A tray application for monitoring the new Steam Controller.")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .action(ArgAction::SetTrue)
                .required(false)
                .help("Use verbose output "),
        )
        .arg(
            Arg::new("monochrome_icons")
                .long("monochrome-icons")
                .action(ArgAction::SetTrue)
                .required(false)
                .help("Use the symbolic (monochrome) variants of the system tray icons"),
        )
        .get_matches();

    VERBOSE.set(matches.get_flag("verbose")).unwrap();
    let monochrome_icons = matches.get_flag("monochrome_icons");

    let (tx, rx) = mpsc::channel();
    let tray_handler = TrayHandler::new(StatusTray::new(tx, monochrome_icons));
    loop {
        let exit = Arc::new(AtomicBool::new(false));

        let devices = loop {
            match connect_compatible_devices() {
                Ok(d) => break d,
                Err(e) => {
                    tray_handler.clear_state();
                    eprintln!("Connecting failed with error: {e}");
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        };

        let device_interface_count = count_compatible_devices().unwrap_or(u32::MAX);

        let mut controller_threads = devices
            .into_iter()
            .map(|device| {
                let (device_tx, device_rx) = multi_threading::create_controller_channel();
                let local_exit = exit.clone();
                (
                    std::thread::spawn(move || {
                        controller_handler(device, device_rx, local_exit);
                    }),
                    device_tx,
                )
            })
            .collect::<Vec<(JoinHandle<()>, ControllerSender)>>();

        // Run loop
        loop {
            // this behaves like a iter_timeout
            let first = rx.recv_timeout(Duration::from_millis(500));
            for (device_id, command) in first.into_iter().chain(rx.try_iter()) {
                controller_threads[device_id as usize]
                    .1
                    .send_command(command);
            }

            let mut states = controller_threads
                .iter()
                .map(|d| d.1.get_latest_properties())
                .enumerate()
                .filter_map(|(device_id, p)| {
                    if device_id == 0 || p.connected == Some(true) {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect::<Vec<DeviceProperties>>();
            // in case there are more than one we can discard controller 0 if not connected
            if states.len() != 1 {
                states.retain(|s| s.connected == Some(true));
            }

            tray_handler.update(&states);
            // if a new puck or controller is connected or disconnected
            if count_compatible_devices().unwrap_or(u32::MAX) != device_interface_count
                || controller_threads.iter().any(|t| t.0.is_finished())
            {
                exit.store(true, std::sync::atomic::Ordering::Relaxed);
                controller_threads.drain(..).for_each(|t| {
                    t.0.join().unwrap();
                });
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
