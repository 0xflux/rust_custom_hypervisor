#![no_std]

pub static NT_DEVICE_NAME: &str = "\\Device\\HyperVisor";
pub static DOS_DEVICE_NAME: &str = "\\??\\HyperVisor";

const SIOCTL_TYPE : u32 = 40000;
const FILE_DEVICE_UNKNOWN: u32 = 34u32;

const METHOD_BUFFERED: u32 = 0;
const METHOD_IN_DIRECT: u32 = 1;
const METHOD_OUT_DIRECT: u32 = 2;
const METHOD_NEITHER: u32 = 3;
const METHOD_DIRECT_TO_HARDWARE: u32 = 1;
const METHOD_DIRECT_FROM_HARDWARE: u32 = 2;

const FILE_ANY_ACCESS: u32 = 0u32;

/// A macro to generate a control code.
macro_rules! CTL_CODE {
    ($DeviceType:expr, $Function:expr, $Method:expr, $Access:expr) => {
        ($DeviceType << 16) | ($Access << 14) | ($Function << 2) | $Method
    };
}

pub const IOCTL_SIOCTL_METHOD_IN_DIRECT : u32 =
    CTL_CODE!(SIOCTL_TYPE, 0x900, METHOD_IN_DIRECT, FILE_ANY_ACCESS);

pub const IOCTL_SIOCTL_METHOD_OUT_DIRECT: u32 =
    CTL_CODE!(SIOCTL_TYPE, 0x901, METHOD_OUT_DIRECT, FILE_ANY_ACCESS);

pub const IOCTL_SIOCTL_METHOD_BUFFERED: u32 =
    CTL_CODE!(SIOCTL_TYPE, 0x902, METHOD_BUFFERED, FILE_ANY_ACCESS);

pub const IOCTL_SIOCTL_METHOD_NEITHER: u32 =
    CTL_CODE!(SIOCTL_TYPE, 0x903, METHOD_NEITHER, FILE_ANY_ACCESS);