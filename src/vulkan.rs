use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::c_void;

use anyhow::anyhow;
use anyhow::Result;
use log::*;

use vulkanalia::prelude::v1_0::*;
use vulkanalia::window as vk_window;
use vulkanalia::Version;
use vulkanalia::{vk, Entry, Instance};

use winit::window::Window;

use crate::app::AppData;

pub const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);
pub const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

pub const VALIDATION_LAYER: vk::ExtensionName =
    vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");


pub extern "system" fn debug_callback(
        severity: vk::DebugUtilsMessageSeverityFlagsEXT, 
        type_: vk::DebugUtilsMessageTypeFlagsEXT, 
        data: *const vk::DebugUtilsMessengerCallbackDataEXT, 
        __: *mut c_void
    ) -> vk::Bool32 {
        let data = unsafe { *data };
        let message = unsafe { CStr::from_ptr(data.message).to_string_lossy() };

       if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::ERROR {
            error!("({:?}) {}", type_, message);
       } else if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::WARNING {
            warn!("({:?}) {}", type_, message);
       } else if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::INFO {
            debug!("({:?}) {}", type_, message);
       } else {
            trace!("({:?}) {}", type_, message);
       }

       vk::FALSE
}
