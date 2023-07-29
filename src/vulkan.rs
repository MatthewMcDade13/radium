use std::collections::HashSet;
use std::ffi::c_void;
use std::ffi::CStr;

use anyhow::anyhow;
use anyhow::Result;
use log::*;

use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::window as vk_window;
use vulkanalia::Version;
use vulkanalia::{vk, Entry, Instance};

use winit::window::Window;

use crate::app::AppData;

pub const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);
pub const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

pub const VALIDATION_LAYER: vk::ExtensionName =
    vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

use thiserror::Error;

#[derive(Debug, Error)]
#[error("Missing {0}")]
pub struct DeviceMatchError(pub &'static str);

pub extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    type_: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    __: *mut c_void,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicalDeviceIndicies {
    pub gfx: u32,
    pub present: u32,
}

impl PhysicalDeviceIndicies {
    pub fn new(
        instance: &Instance,
        data: &AppData,
        physical_device: vk::PhysicalDevice,
    ) -> Result<PhysicalDeviceIndicies> {
        let props =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let graphics = props
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        let present = {
            let mut p = None;
            for (index, _) in props.iter().enumerate() {
                unsafe {
                    if instance.get_physical_device_surface_support_khr(
                        physical_device,
                        index as u32,
                        *data.surface_handle()?,
                    )? {
                        p = Some(index as u32);
                        break;
                    }
                }
            }
            p
        };

        if let (Some(gfx), Some(present)) = (graphics, present) {
            Ok(Self { gfx, present })
        } else {
            Err(anyhow!(DeviceMatchError("Missing required Queue Families")))
        }
    }
}

