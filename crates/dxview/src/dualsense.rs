//! DualSense is a desktop HID feature. Android keeps the same gameplay-facing API without opening HID devices.
#[cfg(not(target_os = "android"))]
#[path = "dualsense_desktop.rs"]
mod imp;
#[cfg(target_os = "android")]
#[path = "dualsense_android.rs"]
mod imp;
pub use imp::*;
