use std::{cell::RefCell, rc::Rc};

use crate::eng::app::MouseState;

use super::{
    camera::PanCamera,
    shader::{Shader, Uniform},
    wgpu_util::{
        texture::{Texture, TextureType},
        vertex::Vertex2D,
    },
};

#[derive(Debug)]
pub struct DeviceSurface {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: Rc<wgpu::Queue>,
    pub config: RefCell<wgpu::SurfaceConfiguration>,
}

impl DeviceSurface {
    #[inline]
    pub fn height(&self) -> u32 {
        self.config.borrow().height
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.config.borrow().width
    }

    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn create_command_encoder(&self) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Command Encoder"),
            })
    }
}

pub struct RenderWindow {
    device_surface: Rc<DeviceSurface>,
    size: winit::dpi::PhysicalSize<u32>,
    window: winit::window::Window,
    default_shader: Shader,

    camera: PanCamera,

    depth_texture: Rc<Texture>,

    mouse_state: MouseState,
}

impl RenderWindow {
    pub async fn from_winit(window: winit::window::Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window)? };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request compatible adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None,
            )
            .await
            .expect("Failed to request compatible device");

        let surface_caps = surface.get_capabilities(&adapter);

        // Assumes sRGB shader format.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let queue = Rc::new(queue);

        let config = RefCell::new(config);
        let surface = DeviceSurface {
            surface,
            device,
            queue,
            config,
        };

        let device = &surface.device;
        let config = &surface.config;

        let camera = PanCamera::new(&surface, 4.0, 0.4);

        let depth_texture = Rc::new(Texture::depth_texture(
            &device,
            &*config.borrow(),
            "Depth Texture".into(),
        ));

        let white_texture = Rc::new(Texture::from_bytes(
            &surface.device,
            surface.queue.as_ref(),
            &Texture::DEFAULT,
            TextureType::Diffuse,
            Some("Default White Texture"),
        )?);

        let surface = Rc::new(surface);

        let shader_src = include_str!("../shaders/sprite.wgsl");

        let default_shader = Shader::new(
            shader_src,
            surface.as_ref(),
            &[Vertex2D::buffer_layout()],
            &[
                Uniform::from_texture(surface.as_ref(), &white_texture),
                camera.create_uniform(surface.as_ref()),
            ],
            Some("Default Sprite Shader"),
        );

        let s = Self {
            device_surface: surface,
            size,
            window,
            camera,
            depth_texture,
            default_shader,

            mouse_state: MouseState::Idle,
        };
        Ok(s)
    }
}
