# OpenSteamController

This tool provides support for the Steam Controller (2026) for non Steam games, with full button support and status updates via built in tray app.
This is done by reading straight from the HID Device itself and spawning a virtual controller per connected physical controller.

It provides sane defaults with buttons mapped like the XBox controllers, all capacitive "buttons" disabled and paddles mapped to shoulder buttons.

## Features

- Connectivity via cable, puck or Bluetooth
- Works on Linux and Windows
- Disables "Lizard Mode" (the  default mouse controlls when not using steam)
- Status updates in tray app, this includes:
    - Available controller slots
    - Charging state
    - Battery level
    - Connectivity
- All digital and analog buttons, sticks and trackpads
- Support for all buttons, including:
    - All digital buttons, including paddles
    - Both sticks
    - Both Trigger
    - Both Trackpads

## Planned Features

- Switch between default and Nintendo layout (swap A/B X/Y)
- Provide custom, non-XBox buttons for paddle/capacitive buttons mapping
- Read and send gyro/accelerometer events
- Toggle mouse/stick/dpad behaviour for trackpads
- Store controller configurations
- Provide AUR package
- Expand informations provided in tray
- Show battery status of lowest controller in tray icon
- Support rumble/haptics
- Optionally "shadow" default steam controller
- Discover newly & reconnected devices after launch
    - Currently, devices will only be discovered on launch
    - Connecting new controllers to an already connected puck works though
- Pairing of new devices
- Shutdown via tool

### Maybe Features

- Support for MacOS

## Known Bugs

### Linux

- The puck will show up as a controller
    - This creates a virtual controller for it
    - Shows connected status, based on if controller is docked or not
- After launch, will not pick up on new devices
    - No new wired controllers, Bluetooth controllers or pucks
    - Connected pucks **will** pick up new controllers
    - A connected device will not be picked up if it's **re**connected

## Usage

### Requirements

#### Linux

- `uinput` kernel module

#### Windows

- TBD

### Installation

#### Reccommended

Download the latest release and store it in an easy to reach place.
On Linux, additionally mark it as executeable by navigating to the folder and using the following command:
```bash
chmod +x ./open-steam-controller
```

After that, just start it. The tool works, if the trackpads no longer controll the mouse and/or a tray icon appears

## Screenshots
