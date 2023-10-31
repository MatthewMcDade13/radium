use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    time::Duration,
};

use wgpu::SurfaceConfiguration;
use winit::{dpi::PhysicalSize, window::WindowId};

use crate::eng::app::MouseState;

use super::{
    camera::PanCamera,
    draw::DrawCtx,
    geom::Rect,
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
}

impl DeviceSurface {
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn create_command_encoder(&self) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Command Encoder"),
            })
    }

    pub fn handle(&self) -> &wgpu::Device {
        &self.device
    }
}

#[derive(Debug)]
pub struct RenderWindow {
    pub device_surface: Rc<DeviceSurface>,
    pub config: wgpu::SurfaceConfiguration,

    size: winit::dpi::PhysicalSize<u32>,
    window: Rc<winit::window::Window>,
    default_shader: Shader,

    camera: PanCamera,

    depth_texture: Rc<Texture>,

    mouse_state: MouseState,
}

impl RenderWindow {
    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn handle(&self) -> &winit::window::Window {
        self.window.as_ref()
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw()
    }

    pub fn mouse_state(&self) -> MouseState {
        self.mouse_state
    }

    pub async fn new(window: winit::window::Window) -> anyhow::Result<Self> {
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

        let surface = DeviceSurface {
            surface,
            device,
            queue,
        };

        let device = &surface.device;

        let camera = PanCamera::new(
            Rect {
                x: 0.,
                y: 0.,
                w: config.width as f32,
                h: config.height as f32,
            },
            4.0,
            0.4,
        );

        let depth_texture = Rc::new(Texture::depth_texture(
            &device,
            &config,
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
            config.format,
            &[Vertex2D::buffer_layout()],
            &[
                Uniform::from_texture(surface.as_ref(), &white_texture),
                camera.create_uniform(surface.as_ref()),
            ],
            Some("Default Sprite Shader"),
        );

        let window = Rc::new(window);

        let s = Self {
            device_surface: surface,
            size,
            window,
            camera,
            depth_texture,
            default_shader,
            config,

            mouse_state: MouseState::Idle,
        };
        Ok(s)
    }

    pub fn create_draw_context(&self) -> DrawCtx {
        DrawCtx::new(&self.device_surface, &self.depth_texture)
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;

            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.device_surface
                .surface
                .configure(&self.device_surface.device, &self.config);
            self.depth_texture = {
                let c = self.config;
                let t =
                    Texture::depth_texture(self.device_surface.handle(), &c, Some("Depth Texture"));
                Rc::new(t)
            }
        }

        self.camera
            .projection
            .resize(new_size.width, new_size.height);
    }
}
