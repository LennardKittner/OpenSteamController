#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use open_steam_controller::debug_println;
use open_steam_controller::devices::{Controller, DeviceCommand};
use open_steam_controller::layout::{self, LayoutState, MappedOutput};
use open_steam_controller::multi_threading::{self, ControllerReceiver};
use open_steam_controller::virtual_controller::{ControllerOutput, EmulatedDevice};

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
        .about(env!("CARGO_PKG_DESCRIPTION"))
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
                            // Windows always uses xinput mode
                            controller_handler(true, device, device_rx, local_exit);
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

/// handles packets from a controller and is responsible for generating virtual controller inputs
/// the exit atomic is used for termination
fn controller_handler(
    emulated: EmulatedDevice,
    mut device: Controller,
    mut device_rx: ControllerReceiver,
    exit: Arc<AtomicBool>,
) {
    use open_steam_controller::virtual_controller::{AbstractVirtualController, VirtualController};
    let mut virt_controller: Option<VirtualController> = None;
    let mut state = LayoutState::new();
    // Run loop
    let mut run_counter = 0;
    loop {
        match if run_counter % 300 == 0 && device.device_properties().connected == Some(true) {
            device.active_refresh_state()
        } else {
            device.passive_refresh_state()
        } {
            Ok(device_outputs) => {
                // Let the virtual controllers connection state mirror the real
                // controller
                if device.device_properties().connected == Some(false) && virt_controller.is_some()
                {
                    virt_controller = None;
                } else if device.device_properties().connected == Some(true)
                    && virt_controller.is_none()
                {
                    match VirtualController::new(emulated) {
                        Ok(vc) => {
                            virt_controller = Some(vc);
                            state = LayoutState::new();
                        }
                        Err(e) => eprintln!("Failed to create virtual controller {e}"),
                    }
                }

                let props = device.device_properties();
                if let Some(virt_controller) = &mut virt_controller {
                    for device_output in device_outputs {
                        for layout_output in
                            layout::map_device_output(&mut state, device_output, &props)
                        {
                            match layout_output {
                                MappedOutput::Controller(controller_input) => {
                                    if let Err(e) = virt_controller.send_input(controller_input) {
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

        if let Some(virt_controller) = &mut virt_controller {
            match virt_controller.recv_output() {
                Ok(events) => {
                    for event in events {
                        match event {
                            ControllerOutput::Rumble(event) => {
                                if let Err(e) = device.try_apply(DeviceCommand::Rumble(event)) {
                                    debug_println!("{e}");
                                }
                            }
                            ControllerOutput::Haptic(event) => {
                                if let Err(e) = device.try_apply(DeviceCommand::Haptic(event)) {
                                    debug_println!("{e}");
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    debug_println!("Error getting controller output: {e}");
                }
            }
        }

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
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .action(ArgAction::SetTrue)
                .required(false)
                .help("Use verbose output"),
        )
        .arg(
            Arg::new("monochrome_icons")
                .long("monochrome-icons")
                .action(ArgAction::SetTrue)
                .required(false)
                .help("Use the symbolic (monochrome) variants of the system tray icons"),
        )
        .arg(
            Arg::new("emulate")
                .long("emulate")
                .short('e')
                .value_name("CONTROLLER")
                .value_parser(clap::value_parser!(EmulatedDevice))
                .num_args(1)
                .action(ArgAction::Set)
                .required(false)
                .help("Which device should the virtual controller emulate"),
        )
        .get_matches();

    VERBOSE.set(matches.get_flag("verbose")).unwrap();
    let emulated_device = matches.get_one("emulate").map(|e| *e).unwrap_or_default();
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
                        controller_handler(emulated_device, device, device_rx, local_exit);
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
