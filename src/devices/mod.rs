pub mod steam_controller;

use crate::{
    debug_println, devices::steam_controller::SteamController, virtual_controller::ControllerInput,
};
use hidapi::{HidApi, HidDevice, HidError};
use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    time::Duration,
};
use thistermination::TerminationFull;

const PASSIVE_REFRESH_TIME_OUT: Duration = Duration::from_secs(2);

pub fn format_int_value(value: u8, suffix: &str) -> String {
    if value == 0 && suffix == "min" {
        "never".to_string()
    } else {
        format!("{}{}", value, suffix)
    }
}

pub type DeviceBox = Box<dyn Device + Send>;
type DeviceFactory = fn(DeviceState) -> DeviceBox;

struct DeviceEntry {
    vendor_ids: &'static [u16],
    product_ids: &'static [u16],
    factory: DeviceFactory,
}

const DEVICE_REGISTER: &[DeviceEntry] = &[DeviceEntry {
    vendor_ids: &steam_controller::VENDOR_IDS,
    product_ids: &steam_controller::PRODUCT_IDS,
    factory: |s| Box::new(SteamController::new_from_state(s)),
}];

const RESPONSE_BUFFER_SIZE: usize = 256;
pub const RESPONSE_DELAY: Duration = Duration::from_millis(50);

pub enum Controller {
    Hid(DeviceBox),
}

impl Controller {
    pub fn device_properties(&self) -> DeviceProperties {
        match self {
            Controller::Hid(device) => device.get_device_state().device_properties.clone(),
        }
    }

    pub fn active_refresh_state(&mut self) -> Result<Vec<ControllerInput>, DeviceError> {
        match self {
            Controller::Hid(device) => device.active_refresh_state(),
        }
    }

    pub fn passive_refresh_state(&mut self) -> Result<Vec<ControllerInput>, DeviceError> {
        match self {
            Controller::Hid(device) => device.passive_refresh_state(),
        }
    }

    pub fn allow_passive_refresh(&mut self) -> bool {
        match self {
            Controller::Hid(device) => device.allow_passive_refresh(),
        }
    }

    pub fn try_apply(&mut self, command: DeviceCommand) -> Result<(), DeviceError> {
        match self {
            Controller::Hid(device) => device.try_apply(command),
        }
    }
}

pub fn connect_compatible_devices() -> Result<Vec<Controller>, DeviceError> {
    match connect_hid_devices() {
        Ok(devices) => Ok(devices.into_iter().map(Controller::Hid).collect()),
        Err(error) => Err(error),
    }
}

/// Count the number of interfaces belonging to supported devices
pub fn count_compatible_devices() -> Result<u32, DeviceError> {
    let all_product_ids: Vec<u16> = DEVICE_REGISTER
        .iter()
        .flat_map(|e| e.product_ids.iter().copied())
        .collect();
    let all_vendor_ids: Vec<u16> = DEVICE_REGISTER
        .iter()
        .flat_map(|e| e.vendor_ids.iter().copied())
        .collect();

    let hid_api = HidApi::new()?;
    let mut device_count = 0;
    for device in hid_api.device_list() {
        if all_product_ids.contains(&device.product_id())
            && all_vendor_ids.contains(&device.vendor_id())
        {
            device_count += 1;
        }
    }
    Ok(device_count)
}

fn connect_hid_devices() -> Result<Vec<DeviceBox>, DeviceError> {
    let all_product_ids: Vec<u16> = DEVICE_REGISTER
        .iter()
        .flat_map(|e| e.product_ids.iter().copied())
        .collect();
    let all_vendor_ids: Vec<u16> = DEVICE_REGISTER
        .iter()
        .flat_map(|e| e.vendor_ids.iter().copied())
        .collect();
    let states = DeviceState::new(&all_product_ids, &all_vendor_ids).unwrap_or_default();
    debug_println!("Found device selecting handler");

    // On Linux and MacOS we can just take the first
    #[cfg(not(target_os = "windows"))]
    {
        let mut devices: Vec<DeviceBox> = states
            .into_iter()
            // every 3rd device is a new controller
            // 0-2 is first
            // 3-5 is second...
            .step_by(3)
            .filter_map(|state| {
                eprintln!(
                    "Connecting to {}",
                    state
                        .device_properties
                        .device_name
                        .clone()
                        .unwrap_or("???".to_string())
                );
                DEVICE_REGISTER
                    .iter()
                    .find(|e| {
                        e.vendor_ids.contains(&state.device_properties.vendor_id)
                            && e.product_ids.contains(&state.device_properties.product_id)
                    })
                    .map(|entry| (entry.factory)(state))
            })
            .collect();

        if devices.is_empty() {
            Err(DeviceError::NoDeviceFound())
        } else {
            devices.iter_mut().enumerate().for_each(|(i, p)| {
                p.get_device_state_mut().device_properties.controller_id = i as u64
            });
            Ok(devices)
        }
    }
    // On Windows we have to check which interface can be used
    #[cfg(target_os = "windows")]
    {
        let mut devices = Vec::new();
        for (i, state) in states.into_iter().enumerate() {
            eprintln!(
                "Try to connect to {}",
                state
                    .device_properties
                    .device_name
                    .clone()
                    .unwrap_or("???".to_string())
            );
            let entry = DEVICE_REGISTER
                .iter()
                .find(|e| {
                    e.vendor_ids.contains(&state.device_properties.vendor_id)
                        && e.product_ids.contains(&state.device_properties.product_id)
                })
                .ok_or(DeviceError::NoDeviceFound())?;

            let mut test_device = (entry.factory)(state);

            let mut buff = [0u8; 64];
            let bytes_read = test_device
                .get_device_state_mut()
                .hid_device
                .read_timeout(&mut buff, 500);
            debug_println!("reading {i} {:?} {:?}", bytes_read, &buff);

            if let Ok(_b) = bytes_read {
                devices.push(test_device);
            }
        }
        if devices.is_empty() {
            Err(DeviceError::NoDeviceFound())
        } else {
            devices.iter_mut().enumerate().for_each(|(i, p)| {
                p.get_device_state_mut().device_properties.controller_id = i as u64
            });
            Ok(devices)
        }
    }
}

#[derive(Debug)]
pub struct DeviceState {
    pub hid_device: HidDevice,
    pub device_properties: DeviceProperties,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProperties {
    pub product_id: u16,
    pub vendor_id: u16,
    pub device_name: Option<String>,
    pub battery_level: Option<u8>,
    pub charging: Option<ChargingStatus>,
    pub connected: Option<bool>,
    pub previous_button_bitmap: u64,
    pub nintendo_layout: bool,
    pub controller_id: u64,
}

impl Display for DeviceProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_with_readonly_info(25))
    }
}

impl DeviceState {
    pub fn new(product_ids: &[u16], vendor_ids: &[u16]) -> Result<Vec<Self>, DeviceError> {
        let hid_api = HidApi::new()?;
        let mut potential_devices = HashSet::new();
        let mut error = Ok(());
        debug_println!(
            "Devices: {:?}",
            hid_api
                .device_list()
                .by_ref()
                .map(|d| { (d.vendor_id(), d.product_id(), d.product_string()) })
                .collect::<Vec<(u16, u16, Option<&str>)>>()
        );
        let device_candidates: Vec<(HidDevice, u16, u16)> = hid_api
            .device_list()
            .filter_map(|info| {
                if product_ids.contains(&info.product_id())
                    && vendor_ids.contains(&info.vendor_id())
                {
                    debug_println!(
                        "Selecting: {:x}:{:x} {:?}",
                        info.vendor_id(),
                        info.product_id(),
                        info.product_string()
                    );
                    match info.open_device(&hid_api) {
                        Ok(device) => Some((device, info.product_id(), info.vendor_id())),
                        Err(e) => {
                            debug_println!(
                                "Failed to open: {:x}:{:x} {:?}: {:?}",
                                info.vendor_id(),
                                info.product_id(),
                                info.product_string(),
                                e
                            );
                            error = Err(e);
                            None
                        }
                    }
                } else {
                    if let Some(name) = info.product_string() {
                        if name.contains("Steam Controller") {
                            potential_devices.insert((
                                info.vendor_id(),
                                info.product_id(),
                                info.product_string(),
                            ));
                        }
                    }
                    None
                }
            })
            .collect();

        if device_candidates.is_empty() {
            if !potential_devices.is_empty() {
                let names = potential_devices
                    .iter()
                    .map(|e| {
                        format!(
                            "    vendorID: 0x{:04X} productID: 0x{:04X} name: {}",
                            e.0,
                            e.1,
                            e.2.unwrap_or("Unknown")
                        )
                    })
                    .collect::<Vec<String>>()
                    .join(",\n");
                //TODO: show as message in tray app
                eprintln!(
                    "Found the following Steam Controllers {}: [\n{}\n]\nHowever, either {} not supported or the product ID is not yet known.",
                    if potential_devices.len() > 1 { "s" } else { "" }, names, if potential_devices.len() > 1 { "they are" } else { "it is" }
                );
            }
            error?;
            return Err(DeviceError::NoDeviceFound());
        }

        Ok(device_candidates
            .into_iter()
            .map(|(hid_device, product_id, vendor_id)| {
                let device_name = hid_device.get_product_string().ok().flatten();
                DeviceState {
                    hid_device,
                    device_properties: DeviceProperties::new(product_id, vendor_id, device_name),
                }
            })
            .collect())
    }

    /// Write a HID report to the device.
    ///
    /// On Windows, devices expose commands as **Feature reports** only.
    /// In that case, `hidapi::HidDevice::write()` fails with:
    /// `WriteFile: (0x00000001) Incorrect function.`
    ///
    /// Linux/macOS hidraw paths often accept the same bytes via output reports, so this can look
    /// "Windows-exclusive". We transparently fall back to `send_feature_report` when we detect
    /// this specific failure.
    /// Adapted from PR #20 by @navrozashvili
    /// Source: https://github.com/LennardKittner/HyperHeadset/pull/20
    pub fn write_hid_report(&self, packet: &[u8]) -> Result<(), HidError> {
        match self.hid_device.send_feature_report(packet) {
            Ok(_) => Ok(()),
            Err(write_err) => {
                #[cfg(target_os = "windows")]
                {
                    if let HidError::HidApiError { message } = &write_err {
                        // Windows HID stack returns ERROR_INVALID_FUNCTION (0x1) when the device
                        // doesn't support output reports / interrupt OUT.
                        if message.contains("Incorrect function")
                            || message.contains("(0x00000001)")
                        {
                            // If the feature report also fails, prefer returning the original
                            // write() error since that's what callers attempted.
                            if let Err(_feature_err) = self.hid_device.send_feature_report(packet) {
                                return Err(write_err);
                            }
                            return Ok(());
                        }
                    }
                }
                Err(write_err)
            }
        }
    }

    fn update_self_with_event(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::BatteryLevel(level) => self.device_properties.battery_level = Some(*level),
            DeviceEvent::Charging(status) => self.device_properties.charging = Some(*status),
            DeviceEvent::WirelessConnected(connected) => {
                self.device_properties.connected = Some(*connected)
            }
            DeviceEvent::ButtonPressed(_controller_input) => {
                panic!("ButtonPressed should be handled in the refreshes")
            }
            DeviceEvent::UpdateBitmap(bitmap) => {
                self.device_properties.previous_button_bitmap = *bitmap
            }
            DeviceEvent::Command(_) => {}
        };
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum PropertyType {
    ReadOnly,
    AlwaysReadOnly,
    ReadWrite,
}

#[derive(Debug)]
pub enum PropertyDescriptorWrapper {
    Int(PropertyDescriptor<u8>, &'static [u8]),
    Bool(PropertyDescriptor<bool>),
    String(PropertyDescriptor<String>),
}

pub struct PropertyDescriptor<T: 'static> {
    pub name: &'static str,
    pub pretty_name: &'static str,
    pub data: Option<T>,
    pub suffix: &'static str,
    pub property_type: PropertyType,
    pub create_event: &'static (dyn Fn(T) -> Option<DeviceCommand> + Send + Sync),
}

impl<T: Debug> Debug for PropertyDescriptor<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyDescriptor")
            .field("pretty_name", &self.pretty_name)
            .field("data", &self.data)
            .field("suffix", &self.suffix)
            .field("property_type", &self.property_type)
            .finish()
    }
}

impl DeviceProperties {
    pub fn new(product_id: u16, vendor_id: u16, device_name: Option<String>) -> DeviceProperties {
        DeviceProperties {
            product_id,
            vendor_id,
            device_name,
            battery_level: None,
            charging: None,
            connected: None,
            previous_button_bitmap: 0,
            nintendo_layout: false,
            controller_id: 99,
        }
    }

    pub fn get_properties(&self) -> Vec<PropertyDescriptorWrapper> {
        vec![
            PropertyDescriptorWrapper::String(PropertyDescriptor {
                name: "charging_status",
                pretty_name: "Charging status",
                data: self.charging.map(|c| c.to_string()),
                suffix: "",
                property_type: PropertyType::AlwaysReadOnly,
                create_event: &|_| None,
            }),
            PropertyDescriptorWrapper::Int(
                PropertyDescriptor {
                    name: "battery_level",
                    pretty_name: "Battery level",
                    data: self.battery_level,
                    suffix: "%",
                    property_type: PropertyType::AlwaysReadOnly,
                    create_event: &|_| None,
                },
                &[],
            ),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                name: "connected",
                pretty_name: "Connected",
                data: self.connected,
                suffix: "",
                property_type: PropertyType::AlwaysReadOnly,
                create_event: &|_| None,
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                name: "nintendo_layout",
                pretty_name: "Use Nintendo Layout",
                data: Some(self.nintendo_layout),
                suffix: "",
                property_type: PropertyType::ReadWrite,
                create_event: &move |_| Some(DeviceCommand::NintendoLayoutToggle),
            }),
        ]
    }

    pub fn to_string_with_padding(&self, padding: usize) -> String {
        self.get_properties()
            .iter()
            .filter_map(|prop| {
                let (name, data, suffix) = match prop {
                    PropertyDescriptorWrapper::Int(property_descriptor, _) => (
                        property_descriptor.pretty_name,
                        &property_descriptor
                            .data
                            .map(|v| format_int_value(v, property_descriptor.suffix)),
                        "",
                    ),
                    PropertyDescriptorWrapper::Bool(property_descriptor) => (
                        property_descriptor.pretty_name,
                        &property_descriptor.data.map(|v| v.to_string()),
                        property_descriptor.suffix,
                    ),
                    PropertyDescriptorWrapper::String(property_descriptor) => (
                        property_descriptor.pretty_name,
                        &property_descriptor.data,
                        property_descriptor.suffix,
                    ),
                };
                data.as_ref()
                    .map(|data| format!("{:<padding$} {}{}", name.to_string() + ":", data, suffix))
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn to_string_with_readonly_info(&self, padding: usize) -> String {
        self.get_properties()
            .iter()
            .filter_map(|prop| {
                let (name, data, suffix, property_type) = match prop {
                    PropertyDescriptorWrapper::Int(property_descriptor, _) => (
                        property_descriptor.pretty_name,
                        &property_descriptor
                            .data
                            .map(|v| format_int_value(v, property_descriptor.suffix)),
                        "",
                        property_descriptor.property_type,
                    ),
                    PropertyDescriptorWrapper::Bool(property_descriptor) => (
                        property_descriptor.pretty_name,
                        &property_descriptor.data.map(|v| v.to_string()),
                        property_descriptor.suffix,
                        property_descriptor.property_type,
                    ),
                    PropertyDescriptorWrapper::String(property_descriptor) => (
                        property_descriptor.pretty_name,
                        &property_descriptor.data,
                        property_descriptor.suffix,
                        property_descriptor.property_type,
                    ),
                };

                data.as_ref().map(|data| {
                    let readonly_marker = if property_type == PropertyType::ReadOnly {
                        " (read-only)"
                    } else {
                        ""
                    };
                    format!(
                        "{:<padding$} {}{}{}",
                        name.to_string() + ":",
                        data,
                        suffix,
                        readonly_marker
                    )
                })
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[derive(TerminationFull)]
pub enum DeviceError {
    #[termination(msg("{0:?}"))]
    HidError(#[from] HidError),
    #[termination(msg("No device found."))]
    NoDeviceFound(),
    #[termination(msg("No response. Is the controller turned on?"))]
    ControllerOff(),
    #[termination(msg("No response."))]
    NoResponse(),
    #[termination(msg("Unknown response: {0:?} with length: {1:?}"))]
    UnknownResponse([u8; 8], usize),
}

#[derive(Debug, Copy, Clone)]
/// Events for or from the device
pub enum DeviceEvent {
    BatteryLevel(u8),
    Charging(ChargingStatus),
    WirelessConnected(bool),
    ButtonPressed(ControllerInput),
    UpdateBitmap(u64),
    Command(DeviceCommand),
}

#[derive(Debug, Copy, Clone)]
/// Commands targeting the controller
pub enum DeviceCommand {
    TurnOff,
    NintendoLayoutToggle,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChargingStatus {
    NotCharging,
    Charging,
    FullyCharged,
    ChargeError,
}

impl Display for ChargingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChargingStatus::NotCharging => "Not charging",
                ChargingStatus::Charging => "Charging",
                ChargingStatus::FullyCharged => "Fully charged",
                ChargingStatus::ChargeError => "Charging error!",
            }
        )
    }
}

impl From<u8> for ChargingStatus {
    fn from(value: u8) -> ChargingStatus {
        match value {
            0 => ChargingStatus::NotCharging,
            1 => ChargingStatus::Charging,
            2 => ChargingStatus::FullyCharged,
            _ => ChargingStatus::ChargeError,
        }
    }
}

pub trait Device {
    fn get_response_buffer(&self) -> Vec<u8> {
        [0u8; RESPONSE_BUFFER_SIZE].to_vec()
    }
    fn get_charging_packet(&self) -> Option<Vec<u8>>;
    fn get_battery_packet(&self) -> Option<Vec<u8>>;
    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>>;
    fn get_event_from_device_response(&mut self, response: &[u8]) -> Option<Vec<DeviceEvent>>;
    fn get_device_state(&self) -> &DeviceState;
    fn get_device_state_mut(&mut self) -> &mut DeviceState;
    fn prepare_write(&mut self) {}
    /// turn off the controller
    fn turn_off(&mut self) -> Result<(), DeviceError>;
    /// whether the app should periodically listen for packets from the controllers
    fn allow_passive_refresh(&mut self) -> bool;

    fn wait_for_updates(&mut self, duration: Duration) -> Option<Vec<DeviceEvent>> {
        let mut buf = self.get_response_buffer();
        let res = self
            .get_device_state()
            .hid_device
            .read_timeout(&mut buf[..], duration.as_millis() as i32)
            .ok()?;

        if res == 0 {
            return None;
        }

        self.get_event_from_device_response(&buf)
    }

    fn get_query_packets(&self) -> Vec<Vec<u8>> {
        vec![
            self.get_wireless_connected_status_packet(),
            self.get_charging_packet(),
            self.get_battery_packet(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Refreshes the state by querying all available information
    fn active_refresh_state(&mut self) -> Result<Vec<ControllerInput>, DeviceError> {
        let packets = self.get_query_packets();

        let mut pressed_buttons: Vec<ControllerInput> = vec![];
        let mut responded = false;
        for packet in packets.into_iter() {
            self.prepare_write();
            debug_println!("Write packet: {packet:?}");
            self.get_device_state().write_hid_report(&packet)?;
            std::thread::sleep(RESPONSE_DELAY);
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    match event {
                        DeviceEvent::ButtonPressed(button) => pressed_buttons.push(button),
                        _ => self.get_device_state_mut().update_self_with_event(&event),
                    }
                }
                responded = true;
            }
            if !matches!(
                self.get_device_state().device_properties.connected,
                Some(true)
            ) {
                break;
            }
        }

        if responded {
            Ok(pressed_buttons)
        } else {
            Err(DeviceError::NoResponse())
        }
    }

    /// Refreshes the state by listening for events
    /// Only the battery level is actively queried because it is not communicated by the device on its own
    fn passive_refresh_state(&mut self) -> Result<Vec<ControllerInput>, DeviceError> {
        let mut pressed_buttons = vec![];
        let nintendo_layout = self
            .get_device_state_mut()
            .device_properties
            .nintendo_layout;
        if self.allow_passive_refresh() {
            if let Some(events) = self.wait_for_updates(PASSIVE_REFRESH_TIME_OUT) {
                for event in events {
                    match (nintendo_layout, event) {
                        (true, DeviceEvent::ButtonPressed(ControllerInput::A(pressed))) => {
                            pressed_buttons.push(ControllerInput::B(pressed))
                        }
                        (true, DeviceEvent::ButtonPressed(ControllerInput::B(pressed))) => {
                            pressed_buttons.push(ControllerInput::A(pressed))
                        }
                        (true, DeviceEvent::ButtonPressed(ControllerInput::X(pressed))) => {
                            pressed_buttons.push(ControllerInput::Y(pressed))
                        }
                        (true, DeviceEvent::ButtonPressed(ControllerInput::Y(pressed))) => {
                            pressed_buttons.push(ControllerInput::X(pressed))
                        }
                        (_, DeviceEvent::ButtonPressed(button)) => pressed_buttons.push(button),
                        (_, DeviceEvent::Command(command)) => self.try_apply(command)?,
                        _ => self.get_device_state_mut().update_self_with_event(&event),
                    }
                }
            }
        }

        Ok(pressed_buttons)
    }

    // If possible apply a give command
    fn try_apply(&mut self, command: DeviceCommand) -> Result<(), DeviceError> {
        match command {
            DeviceCommand::TurnOff => self.turn_off()?,
            DeviceCommand::NintendoLayoutToggle => {
                self.get_device_state_mut()
                    .device_properties
                    .nintendo_layout = !self
                    .get_device_state_mut()
                    .device_properties
                    .nintendo_layout;
            }
        }
        Ok(())
    }

    fn clear_state(&mut self) {
        let product_id = self.get_device_state().device_properties.product_id;
        let vendor_id = self.get_device_state().device_properties.vendor_id;
        let device_name = self
            .get_device_state()
            .device_properties
            .device_name
            .clone();
        self.get_device_state_mut().device_properties =
            DeviceProperties::new(product_id, vendor_id, device_name)
    }
}
