use std::str::FromStr;

use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

/// A trait defining shared behavior between compatible devices.
pub trait Device {
    /// Run any initialization logic the device requires.
    fn initialize(&self) -> Result<(), DeviceError>;

    /// Read a DSU frame from the device.
    fn read_frame(&self) -> Result<DSUFrame, DeviceError>;
}

/// A trait defining shared behavior dependant on the frame type `F` between compatible devices.
pub trait FrameDevice<F> {
    fn to_dsu_frame(frame: &F, gyro_disabled: bool) -> DSUFrame;
    /// Test if a [`DeviceButton`](crate::devices::DeviceButton) is pressed.
    fn is_device_button_pressed(button: &DeviceButton, frame: &F) -> bool;
}

/// Device buttons not specific to any one device.
#[derive(Debug, Clone)]
pub enum DeviceButton {
    /// Directional pad Left
    DpadLeft,
    /// Directional pad Down
    DpadDown,
    /// Directional pad Right
    DpadRight,
    /// Directional pad Up
    DpadUp,
    /// Start, options, etc.
    Start,
    /// Select, share, etc.
    Select,
    /// PS button, steam button, etc.
    Guide,
    /// QAM, Mic, etc.
    Quaternary,
    /// XB layout A
    A,
    /// XB layout B
    B,
    /// XB layout X
    X,
    /// XB layout Y
    Y,
    /// L1, Left Bumper
    L1,
    /// R1, Right Bumper
    R1,
    /// L2, Left Trigger
    L2,
    /// R2, Right Trigger
    R2,
    /// L3, Left stick click
    L3,
    /// R3, Right stick click
    R3,
    /// Triton only
    L4,
    /// Triton only
    L5,
    /// Triton only
    R4,
    /// Triton only
    R5,
    /// Triton only
    LeftStickTouch,
    /// Triton only
    RightStickTouch,
    /// Triton only
    LeftPadTouch,
    /// Triton only
    RightPadTouch,
    /// Triton only
    LeftGrip,
    /// Triton only
    RightGrip,
    /// Unknown button
    Unknown,
}

impl FromStr for DeviceButton {
    type Err = DeviceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dpad_left" => Ok(Self::DpadLeft),
            "dpad_down" => Ok(Self::DpadDown),
            "dpad_right" => Ok(Self::DpadRight),
            "dpad_up" => Ok(Self::DpadUp),
            "start" => Ok(Self::Start),
            "select" => Ok(Self::Select),
            "guide" => Ok(Self::Guide),
            "quaternary" => Ok(Self::Quaternary),
            "a" => Ok(Self::A),
            "b" => Ok(Self::B),
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            "l1" => Ok(Self::L1),
            "r1" => Ok(Self::R1),
            "l2" => Ok(Self::L2),
            "r2" => Ok(Self::R2),
            "l3" => Ok(Self::L3),
            "r3" => Ok(Self::R3),
            "l4" => Ok(Self::L4),
            "l5" => Ok(Self::L5),
            "r4" => Ok(Self::R4),
            "r5" => Ok(Self::R5),
            "left_stick_touch" => Ok(Self::LeftStickTouch),
            "right_stick_touch" => Ok(Self::RightStickTouch),
            "left_pad_touch" => Ok(Self::LeftPadTouch),
            "right_pad_touch" => Ok(Self::RightPadTouch),
            "left_grip" => Ok(Self::LeftGrip),
            "right_grip" => Ok(Self::RightGrip),
            _ => Err(DeviceError::InvalidDeviceButton(s.to_string())),
        }
    }
}

impl std::fmt::Display for DeviceButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DpadLeft => write!(f, "dpad_left"),
            Self::DpadDown => write!(f, "dpad_down"),
            Self::DpadRight => write!(f, "dpad_right"),
            Self::DpadUp => write!(f, "dpad_up"),
            Self::Start => write!(f, "start"),
            Self::Select => write!(f, "select"),
            Self::Guide => write!(f, "guide"),
            Self::Quaternary => write!(f, "quaternary"),
            Self::A => write!(f, "a"),
            Self::B => write!(f, "b"),
            Self::X => write!(f, "x"),
            Self::Y => write!(f, "y"),
            Self::L1 => write!(f, "l1"),
            Self::R1 => write!(f, "r1"),
            Self::L2 => write!(f, "l2"),
            Self::R2 => write!(f, "r2"),
            Self::L3 => write!(f, "l3"),
            Self::R3 => write!(f, "r3"),
            Self::L4 => write!(f, "l4"),
            Self::L5 => write!(f, "l5"),
            Self::R4 => write!(f, "r4"),
            Self::R5 => write!(f, "r5"),
            Self::LeftStickTouch => write!(f, "left_stick_touch"),
            Self::RightStickTouch => write!(f, "right_stick_touch"),
            Self::LeftPadTouch => write!(f, "left_pad_touch"),
            Self::RightPadTouch => write!(f, "right_pad_touch"),
            Self::LeftGrip => write!(f, "left_grip"),
            Self::RightGrip => write!(f, "right_grip"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Gyro activation toggle mode.
///
/// Any => At least one button must be pressed to activate gyro.
///
/// All => All buttons must be pressed to activate gyro.
#[derive(Default, Debug, Clone)]
pub enum GyroActivationMode {
    #[default]
    Any,
    All,
}

impl FromStr for GyroActivationMode {
    type Err = DeviceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "any" => Ok(Self::Any),
            "all" => Ok(Self::All),
            _ => Err(DeviceError::InvalidGyroActivationMode(s.to_string())),
        }
    }
}

impl std::fmt::Display for GyroActivationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any => write!(f, "any"),
            Self::All => write!(f, "all"),
        }
    }
}

/// Device configuration.
///
/// Defines configurable behavior within device adapters themselves.
///
/// For configuration affecting the behavior of the DSU server,
/// see ['ServerConfig'](crate::server::ServerConfig)
#[derive(Default, Debug, Clone)]
pub struct DeviceConfig {
    /// Don't enable lizard mode when the device is dropped (Triton)
    pub no_enable_lizard_mode_on_close: bool,
    /// Inputs that must be pressed to send gyro data through the DSU server.
    pub gyro_activation_inputs: Vec<DeviceButton>,
    /// See ['GyroActivationMode'](crate::devices::GyroActivationMode)
    pub gyro_activation_mode: GyroActivationMode,
}
