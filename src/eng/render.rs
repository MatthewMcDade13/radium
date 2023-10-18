use std::{
    borrow::BorrowMut,
    cell::{Cell, Ref, RefCell, RefMut},
    collections::VecDeque,
    ops::Range,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use cgmath::Rad;
use wgpu::{util::DeviceExt, Device, DynamicOffset, RenderPass, TextureView};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder, WindowId},
};

use crate::gfx::{
    camera::{Camera, CameraControl, CameraUniform, PanCamera, Projection},
    draw::DrawCtx,
    light::LightUniform,
    model::{Material, Mesh, Model},
    wgpu::{
        buffer::{create_render_pipeline, InstanceRaw},
        texture::Texture,
        vertex::Vertex,
    },
};

use self::{
    light::{draw_light_mesh_instanced, draw_light_model_instanced},
    mesh::{draw_mesh_instanced, draw_model_instanced},
};

use super::{
    app::{InputEventStatus, MouseState},
    command::RenderCommand,
};
use anyhow::*;

#[derive(Debug)]
pub struct DeviceSurface {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: Arc<wgpu::Queue>,
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
    window: Window,
    clear_color: wgpu::Color,
    pipeline: Arc<wgpu::RenderPipeline>,

    camera: RenderCamera,

    light_render: light::LightRenderer,

    depth_texture: Rc<Texture>,

    event_loop: Option<Rc<EventLoop<()>>>,
    mouse_state: MouseState,
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl RenderWindow {
    pub fn gfx_queue(&self) -> Arc<wgpu::Queue> {
        self.device_surface.queue.clone()
    }
    pub fn camera(&self) -> &RenderCamera {
        &self.camera
    }

    pub fn camera_mut(&mut self) -> &mut RenderCamera {
        &mut self.camera
    }
    pub fn camera_uniform(&self) -> &CameraUniform {
        &self.camera.cam.uniform
    }
    pub const fn handle(&self) -> &Window {
        &self.window
    }
    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn surface_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.device_surface().get_current_texture()
    }

    pub fn device_surface(&self) -> &Rc<DeviceSurface> {
        &self.device_surface
    }

    #[inline]
    pub fn pipeline(&self) -> Arc<wgpu::RenderPipeline> {
        self.pipeline.clone()
    }
    #[inline]
    pub fn light_render_pipeline(&self) -> Arc<wgpu::RenderPipeline> {
        self.light_render.pipeline()
    }

    #[inline]
    pub fn depth_texture(&self) -> &Rc<Texture> {
        &self.depth_texture
    }

    #[inline]
    pub fn light_bind_group(&self) -> Arc<wgpu::BindGroup> {
        self.light_render.bind_group()
    }

    #[inline]
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    #[inline]
    pub fn device_queue(&self) -> &wgpu::Queue {
        &self.device_surface().queue
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.device_surface().device
    }

    #[inline]
    pub fn event_loop(&self) -> Option<Rc<EventLoop<()>>> {
        self.event_loop.clone()
    }

    #[inline]
    pub fn surface_config(&self) -> Ref<wgpu::SurfaceConfiguration> {
        self.device_surface().config.borrow()
    }

    #[inline]
    pub fn surface_config_mut(&self) -> RefMut<wgpu::SurfaceConfiguration> {
        self.device_surface().config.borrow_mut()
    }

    #[inline]
    pub fn surface_device(&self) -> &wgpu::Device {
        &self.device_surface.device
    }

    #[inline]
    pub const fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub const fn mouse_state(&self) -> MouseState {
        self.mouse_state
    }

    #[inline]
    pub fn camera_bind_group(&self) -> Arc<wgpu::BindGroup> {
        self.camera.bind_group()
    }

    pub async fn new() -> anyhow::Result<Self> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop)?;
        Self::from_winit(window, Some(Rc::new(event_loop))).await
    }

    pub async fn from_winit<EvntLoop>(
        window: winit::window::Window,
        event_loop: EvntLoop,
    ) -> anyhow::Result<Self>
    where
        EvntLoop: Into<Option<Rc<EventLoop<()>>>>,
    {
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
        let queue = Arc::new(queue);

        let config = RefCell::new(config);
        let surface = DeviceSurface {
            surface,
            device,
            queue,
            config,
        };

        let device = &surface.device;
        let config = &surface.config;

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This needs to match filterable filed of the corresponding Texture entry
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let camera = RenderCamera::from_surface(&surface);

        let light_render = light::LightRenderer::new(
            &surface.device,
            surface.config.borrow().format,
            camera.layout().as_ref(),
        );

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    camera.layout().as_ref(),
                    light_render.layout().as_ref(),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/basic.wgsl").into()),
            };
            create_render_pipeline(
                &device,
                &render_pipeline_layout,
                config.borrow().format,
                Some(Texture::DEPTH_FORMAT),
                &[Vertex::buffer_layout(), InstanceRaw::buffer_layout()],
                shader,
            )
        };

        let depth_texture = Rc::new(Texture::depth_texture(
            &device,
            &*config.borrow(),
            "Depth Texture".into(),
        ));
        let surface = Rc::new(surface);

        let s = Self {
            device_surface: surface,
            size,
            window,
            clear_color: wgpu::Color::BLACK,
            pipeline: Arc::new(render_pipeline),
            camera,
            depth_texture,
            light_render,
            event_loop: event_loop.into(),
            mouse_state: MouseState::Idle,
            texture_bind_group_layout,
        };
        Ok(s)
    }

    // pub fn begin_draw(&self) -> Result<DrawCtx, wgpu::SurfaceError> {
    // let mut encoder =
    // self.surface
    // .device
    // .create_command_encoder(&wgpu::CommandEncoderDescriptor {
    // label: Some("Render Command Encoder"),
    // });
    // let s = DrawCtx::begin(
    // self,
    // &self.surface.device,
    // &self.surface.queue,
    // &mut encoder,
    // )?;
    // Result::Ok(s)
    // }
    //
    //
    //
    pub fn update_camera(&mut self, dt: std::time::Duration) {
        self.camera.frame_update(dt);
        self.set_camera_uniform(CameraUniform::from_camera(
            &self.camera.cam.cam,
            &self.camera.projection,
        ));
        self.write_camera_buffer();
    }
    pub fn set_camera_uniform(&mut self, uniform: CameraUniform) {
        self.camera.set_uniform(uniform);
    }

    pub fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        self.device_surface()
            .queue
            .write_buffer(buffer, offset, data);
    }

    pub fn write_camera_buffer(&self) {
        self.write_buffer(
            &self.camera.buffer,
            0,
            bytemuck::cast_slice(&[*self.camera_uniform()]),
        );
    }
    pub fn create_draw_context(&self) -> DrawCtx {
        DrawCtx::from_window(self)
    }

    // pub fn submit_draw_ctx(&mut self, ctx: &DrawCtx) -> Result<(), wgpu::SurfaceError> {
    // self.submit_frame()
    // }
    pub fn submit_frame(&self, ctx: DrawCtx) -> Result<(), wgpu::SurfaceError> {
        ctx.submit()
    }

    // window: &'a RenderWindow,
    // frame: wgpu::SurfaceTexture,
    // view: &'a TextureView,
    // queue: &'a wgpu::Queue,
    // encoder: &'a mut wgpu::CommandEncoder,

    // drawer.draw_frame(&draw_ctx);

    // draw_ctx.submit(encoder.into_inner());
    // std::result::Result::Ok(())

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;

            self.surface_config_mut().width = new_size.width;
            self.surface_config_mut().height = new_size.height;
            self.device_surface
                .surface
                .configure(self.surface_device(), &*self.surface_config());
            self.depth_texture = {
                let c = self.surface_config();
                let t = Texture::depth_texture(self.surface_device(), &*c, Some("Depth Texture"));
                Rc::new(t)
            }
        }

        self.camera
            .projection_mut()
            .resize(new_size.width, new_size.height);
    }
}

pub struct RenderCamera {
    cam: PanCamera,
    buffer: Arc<wgpu::Buffer>,
    bind_group: Arc<wgpu::BindGroup>,
    layout: Arc<wgpu::BindGroupLayout>,
    projection: Projection,
}

impl RenderCamera {
    pub fn process_keyboard(
        &mut self,
        key: VirtualKeyCode,
        state: ElementState,
    ) -> InputEventStatus {
        self.cam.process_keyboard(key, state)
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.cam.process_mouse(mouse_dx, mouse_dy);
    }

    pub fn process_scroll(&mut self, delta: &winit::event::MouseScrollDelta) {
        self.cam.process_scroll(delta);
    }
}

impl RenderCamera {
    pub fn frame_update(&mut self, dt: Duration) {
        self.cam.frame_update(dt);
    }
}

impl RenderCamera {
    pub fn set_uniform(&mut self, uniform: CameraUniform) {
        self.cam.uniform = uniform;
    }
    /// Creates a RenderCamera from a RenderWindow with default values.
    pub fn from_surface(rs: &DeviceSurface) -> Self {
        let camera = Camera::new((0.0, 5.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection = Projection::new(rs.width(), rs.height(), cgmath::Deg(45.0), 0.1, 100.0);

        let cam_uniform = CameraUniform::from_camera(&camera, &projection);
        let device = &rs.device;

        let cam_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[cam_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let cam_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });
        let cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &cam_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cam_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let pan_cam = PanCamera {
            cam: camera,
            uniform: cam_uniform,
            ctrl: CameraControl::new(4.0, 0.4),
        };

        let buffer = Arc::new(cam_buffer);
        let bind_group = Arc::new(cam_bind_group);
        let layout = Arc::new(cam_bind_group_layout);

        Self {
            cam: pan_cam,
            buffer,
            bind_group,
            layout,
            projection,
        }
    }

    pub fn layout(&self) -> Arc<wgpu::BindGroupLayout> {
        self.layout.clone()
    }

    pub fn buffer(&self) -> Arc<wgpu::Buffer> {
        self.buffer.clone()
    }
    pub fn bind_group(&self) -> Arc<wgpu::BindGroup> {
        self.bind_group.clone()
    }
    pub fn projection(&self) -> &Projection {
        &self.projection
    }
    pub fn projection_mut(&mut self) -> &mut Projection {
        &mut self.projection
    }
}

pub mod light {
    use std::{ops::Range, sync::Arc};

    use wgpu::{util::DeviceExt, Device, RenderPass};

    use crate::{
        eng::command::RenderCommand,
        gfx::{
            light::LightUniform,
            model::{Mesh, Model},
            wgpu::{buffer::create_render_pipeline, texture::Texture, vertex::Vertex},
        },
    };

    pub struct LightRenderer {
        render_pipeline: Arc<wgpu::RenderPipeline>,
        uniform: LightUniform,
        buffer: Arc<wgpu::Buffer>,
        bind_group: Arc<wgpu::BindGroup>,
        layout: Arc<wgpu::BindGroupLayout>,
    }

    impl LightRenderer {
        pub fn bind_group(&self) -> Arc<wgpu::BindGroup> {
            self.bind_group.clone()
        }
        pub fn new(
            device: &Device,
            format: wgpu::TextureFormat,
            cam_bind_group_layout: &wgpu::BindGroupLayout,
        ) -> Self {
            let uniform = LightUniform {
                position: [2.0, 2.0, 2.0, 0.0],
                color: [1.0, 1.0, 1.0, 0.0],
            };

            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light VB"),
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
                label: None,
            });

            let render_pipeline = {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Light Pipeline Layout"),
                    bind_group_layouts: &[cam_bind_group_layout, &layout],
                    push_constant_ranges: &[],
                });
                let shader = wgpu::ShaderModuleDescriptor {
                    label: Some("Light Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/light.wgsl").into()),
                };
                create_render_pipeline(
                    &device,
                    &layout,
                    format,
                    Some(Texture::DEPTH_FORMAT),
                    &[Vertex::buffer_layout()],
                    shader,
                )
            };

            let render_pipeline = Arc::new(render_pipeline);
            let buffer = Arc::new(buffer);
            let bind_group = Arc::new(bind_group);
            let layout = Arc::new(layout);

            Self {
                render_pipeline,
                uniform,
                buffer,
                bind_group,
                layout,
            }
        }
        pub fn pipeline(&self) -> Arc<wgpu::RenderPipeline> {
            self.render_pipeline.clone()
        }

        pub fn layout(&self) -> Arc<wgpu::BindGroupLayout> {
            self.layout.clone()
        }
    }

    pub fn draw_light_mesh(
        mesh: &Mesh,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        draw_light_mesh_instanced(mesh, 0..1, camera_bind_group, light_bind_group)
    }

    pub fn draw_light_mesh_instanced(
        mesh: &Mesh,
        instances: Range<u32>,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        vec![
            RenderCommand::SetVertexBuffer(0, mesh.vert_buff.clone()),
            RenderCommand::SetIndexBuffer(mesh.index_buff.clone(), wgpu::IndexFormat::Uint32),
            RenderCommand::SetBindGroup(0, camera_bind_group.clone(), None),
            RenderCommand::SetBindGroup(1, light_bind_group.clone(), None),
            RenderCommand::DrawIndexed(0..mesh.num_elements, 0, instances),
        ]
    }

    pub fn draw_light_model(
        model: &Model,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        draw_light_model_instanced(
            model,
            0..1,
            camera_bind_group.clone(),
            light_bind_group.clone(),
        )
    }

    pub fn draw_light_model_instanced(
        model: &Model,
        instances: Range<u32>,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        let mut buffer = Vec::new();
        for mesh in &model.meshes {
            let cmds = draw_light_mesh_instanced(
                mesh,
                instances.clone(),
                camera_bind_group.clone(),
                light_bind_group.clone(),
            );
            buffer.extend(cmds);
        }
        buffer
    }
}

pub mod mesh {
    use std::{ops::Range, sync::Arc};

    use wgpu::RenderPass;

    use crate::{
        eng::command::RenderCommand,
        gfx::model::{Material, Mesh, Model},
    };

    pub fn draw_mesh(
        mesh: &Mesh,
        mat: &Material,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        draw_mesh_instanced(
            mesh,
            mat,
            0..1,
            camera_bind_group.clone(),
            light_bind_group.clone(),
        )
    }

    pub fn draw_mesh_instanced(
        mesh: &Mesh,
        mat: &Material,
        instances: Range<u32>,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        vec![
            RenderCommand::SetVertexBuffer(0, mesh.vert_buff.clone()),
            RenderCommand::SetIndexBuffer(mesh.index_buff.clone(), wgpu::IndexFormat::Uint32),
            RenderCommand::SetBindGroup(0, mat.bind_group.clone(), None),
            RenderCommand::SetBindGroup(1, camera_bind_group.clone(), None),
            RenderCommand::SetBindGroup(2, light_bind_group.clone(), None),
            RenderCommand::DrawIndexed(0..mesh.num_elements, 0, instances),
        ]
    }

    pub fn draw_model(
        model: &Model,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group)
    }

    pub fn draw_model_instanced(
        model: &Model,
        instances: Range<u32>,
        camera_bind_group: Arc<wgpu::BindGroup>,
        light_bind_group: Arc<wgpu::BindGroup>,
    ) -> Vec<RenderCommand> {
        let mut buffer = Vec::new();
        for mesh in &model.meshes {
            let mat = &model.materials[mesh.material];

            let cmds = draw_mesh_instanced(
                mesh,
                mat,
                instances.clone(),
                camera_bind_group.clone(),
                light_bind_group.clone(),
            );
            buffer.extend(cmds);
        }
        buffer
    }
}
pub type RenderWindowMut = Rc<RefCell<RenderWindow>>;
