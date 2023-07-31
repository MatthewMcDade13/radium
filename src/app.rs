use std::collections::HashSet;
use std::rc::Rc;

use anyhow::{anyhow, Result};
use log::*;
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;

use vulkanalia::vk::{
    DebugUtilsMessengerCreateInfoEXTBuilder, Device, ExtDebugUtilsExtension, KhrSurfaceExtension,
};
use vulkanalia::{vk, Entry, Instance};

use winit::window::Window;

use crate::vulkan::{
    PhysicalDeviceIndicies, PORTABILITY_MACOS_VERSION, VALIDATION_ENABLED, VALIDATION_LAYER,
};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::window as vk_window;

#[derive(Clone, Debug)]
pub struct App {
    entry: Entry,
    instance: Instance,
    data: AppData,
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

            let flags =
                if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
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

        let layers = default_layers();

        let mut info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions)
            .flags(flags);

        let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
            .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
            .user_callback(Some(crate::vulkan::debug_callback));

        let instance = unsafe { entry.create_instance(&info, None)? };

        let surface = unsafe { vk_window::create_surface(&instance, &window, &window)? };
        let data = AppData::builder()
            .validation(&instance, &debug_info)?
            .find_physical_device(&instance)?
            .create_logical_device(&instance)?
            .build();

        if let Some(_) = data.messenger {
            info = info.push_next(&mut debug_info);
        }

        Ok(Self {
            entry,
            instance,
            data,
        })
    }

    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        Ok(())
    }

    pub unsafe fn destroy(&mut self) {
        if let AppData {
            messenger: Some(msgr),
            logical_device: Some(ld),
            surface: Some(surface),
            ..
        } = self.data.clone()
        {
            self.instance.destroy_debug_utils_messenger_ext(*msgr, None);
            ld.destroy_device(None);
            self.instance.destroy_surface_khr(*surface, None);
        }
        self.instance.destroy_instance(None);
    }

    pub fn device(&self) -> Rc<vulkanalia::Device> {
        self.data.logical_device.as_ref().unwrap().clone()
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe { self.destroy() };
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppData {
    messenger: Option<Box<vk::DebugUtilsMessengerEXT>>,
    physical_device: Option<Rc<vk::PhysicalDevice>>,
    logical_device: Option<Rc<vulkanalia::Device>>,
    gfx_queue: Option<Box<vk::Queue>>,
    surface: Option<Rc<vk::SurfaceKHR>>,
    present_queue: Option<Box<vk::Queue>>,
}

impl AppData {
    pub const fn new() -> Self {
        Self {
            messenger: None,
            physical_device: None,
            logical_device: None,
            gfx_queue: None,
            surface: None,
            present_queue: None,
        }
    }

    pub fn builder() -> AppDataBuilder {
        AppDataBuilder(Box::new(AppData::new()))
    }

    pub fn surface_handle(&self) -> Result<Rc<vk::SurfaceKHR>> {
        if let Some(sh) = self.surface.clone() {
            Ok(sh)
        } else {
            Err(anyhow!("SurfaceKHR not found on AppData for Application"))
        }
    }

    pub fn physical_device(&self) -> Result<Rc<vk::PhysicalDevice>> {
        if let Some(pd) = self.physical_device.clone() {
            Ok(pd)
        } else {
            Err(anyhow!(
                "PhysicalDevice not found on AppData for Application"
            ))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppDataBuilder(pub Box<AppData>);

impl AppDataBuilder {
    pub fn new() -> Self {
        Self(Box::new(AppData::new()))
    }

    pub fn validation(
        self,
        instance: &Instance,
        debug_info: &DebugUtilsMessengerCreateInfoEXTBuilder,
    ) -> Result<Self> {
        let messenger = unsafe { instance.create_debug_utils_messenger_ext(&debug_info, None)? };
        Ok(Self(Box::new(AppData {
            messenger: Some(Box::new(messenger)),
            ..*self.0
        })))
    }

    pub fn find_physical_device(self, instance: &Instance) -> Result<Self> {
        unsafe {
            for physical_device in instance.enumerate_physical_devices()? {
                let props = instance.get_physical_device_properties(physical_device);
                if let Err(err) = check_physical_device(instance, &self.0, physical_device) {
                    warn!(
                        "Skipping physical device (`{}`): {}",
                        props.device_name, err
                    )
                } else {
                    info!("Selected physical device (`{}`)", props.device_name);
                    return Ok(Self(Box::new(AppData {
                        physical_device: Some(Rc::new(physical_device)),
                        ..*self.0
                    })));
                }
            }
        }

        Err(anyhow!("Failed to find physical device."))
    }

    pub fn create_logical_device(self, instance: &Instance) -> Result<Self> {
        let layers = default_layers();
        let exts = default_extensions();
        let features = vk::PhysicalDeviceFeatures::builder();

        let physical_device = self.0.physical_device()?;
        let device_indicies =
            PhysicalDeviceIndicies::new(instance, &self.0, *self.0.physical_device()?)?;

        let unique_indices = {
            let mut uis = HashSet::new();
            uis.insert(device_indicies.gfx);
            uis.insert(device_indicies.present);
            uis
        };

        let queue_priorities = &[1.0];
        let queue_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(device_indicies.gfx)
            .queue_priorities(queue_priorities);

        let queue_infos = unique_indices
            .iter()
            .map(|i| {
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(*i)
                    .queue_priorities(queue_priorities)
            })
            .collect::<Vec<_>>();
        let info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&exts)
            .enabled_features(&features)
            .build();

        let device = unsafe { instance.create_device(*physical_device, &info, None)? };
        let gfx_queue = unsafe { device.get_device_queue(device_indicies.gfx, 0) };
        let present_queue = unsafe { device.get_device_queue(device_indicies.present, 0) };
        Ok(Self(Box::new(AppData {
            logical_device: Some(Rc::new(device)),
            gfx_queue: Some(Box::new(gfx_queue)),
            present_queue: Some(Box::new(present_queue)),
            ..*self.0
        })))
    }

    pub fn build(self) -> AppData {
        *self.0
    }
}

pub fn check_physical_device(
    instance: &Instance,
    data: &AppData,
    physical_device: vk::PhysicalDevice,
) -> Result<()> {
    let props = unsafe { instance.get_physical_device_properties(physical_device) };
    let features = unsafe { instance.get_physical_device_features(physical_device) };

    Ok(())
}

fn default_layers() -> Vec<*const i8> {
    if VALIDATION_ENABLED {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        Vec::new()
    }
}

const fn default_extensions() -> Vec<*const i8> {
    let mut extensions = vec![];
    extensions
    // TODO :: Make this compatible with macOS.
    //      :: As for now, if you are on macOS, im sorry you feel you have to use that bloated proprietary garbage that apple puts out.
    //      :: Your computer hardware deserves so much better. Have you considered installing Gentoo?
    //S
    // Required by Vulkan SDK on macOS since 1.3.216.
    // if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
    //     extensions.push(vk::KHR_PORTABILITY_SUBSET_EXTENSION.name.as_ptr());
    // }
}
