use std::collections::HashSet;

use anyhow::{anyhow, Result};
use log::*;
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;

use vulkanalia::vk::{ExtDebugUtilsExtension, DebugUtilsMessengerCreateInfoEXTBuilder};
use vulkanalia::{vk, Entry, Instance};

use winit::window::Window;

use crate::vulkan::{self, VALIDATION_ENABLED, VALIDATION_LAYER, PORTABILITY_MACOS_VERSION};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::window as vk_window;

#[derive(Clone, Debug)]
pub struct App {
    entry: Entry,
    instance: Instance,
    data: AppData
}

impl App {
    pub fn create(window: &Window) -> Result<Self> {
        let app_info = vk::ApplicationInfo::builder()
            .application_name(b"Radium Vulkan\0")
            .application_version(vk::make_version(1, 0, 0))
            .engine_name(b"No Engine\0")
            .engine_version(vk::make_version(1, 0, 0))
            .api_version(vk::make_version(1, 0, 0));   

       let entry = {
           let loader = unsafe { LibloadingLoader::new(LIBRARY)? };
           let entry = unsafe { Entry::new(loader).map_err(|b| anyhow!("{}", b))? };
           entry
       };

        let available_layers = unsafe {
            entry
                .enumerate_instance_layer_properties()?
                .iter()
                .map(|l| l.layer_name)
                .collect::<HashSet<_>>()
        };
    
        if VALIDATION_ENABLED && !available_layers.contains(&VALIDATION_LAYER) {
            return Err(anyhow!("Validation Layer requested but not supported"));
        }

        let (extensions, flags) = {
            let mut ext = vk_window::get_required_instance_extensions(window)
                .iter()
                .map(|e| e.as_ptr())
                .collect::<Vec<_>>();
    
            if VALIDATION_ENABLED {
                ext.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
            }
    
            let flags = if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
                info!("Enabling extensions for macOS portability.");
                ext.push(
                    vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION
                        .name
                        .as_ptr(),
                );
                ext.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());
                vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
            } else {
                vk::InstanceCreateFlags::empty()
            };
            (ext, flags)
        };
    
        let layers = if VALIDATION_ENABLED {
            vec![VALIDATION_LAYER.as_ptr()]
        } else {
            Vec::new()
        };

        let mut info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions)
            .flags(flags);

        let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
            .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
            .user_callback(Some(vulkan::debug_callback));

     
        let instance = unsafe { entry.create_instance(&info, None)? };

        let data = AppData::new_with_validation(&instance, &debug_info)?;

        if let Some(_) = data.messenger {
            info = info.push_next(&mut debug_info);
        }

        Ok(Self { entry, instance, data })
    }

    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        Ok(())
    }

    pub unsafe fn destroy(&mut self) {
        if let Some(msgr) = self.data.messenger {
            self.instance.destroy_debug_utils_messenger_ext(msgr, None);
        }
        self.instance.destroy_instance(None);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe { self.destroy() };
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppData {
    messenger: Option<vk::DebugUtilsMessengerEXT>
}

impl AppData {
    pub const fn new() -> Self {
        Self { messenger: None }
    }

    pub fn new_with_validation(instance: &Instance, debug_info: &DebugUtilsMessengerCreateInfoEXTBuilder) -> Result<Self> {

        let messenger = unsafe { instance.create_debug_utils_messenger_ext(&debug_info, None)? };
        let s = Self { messenger: Some(messenger) };
        Ok(s)
    
    }
}