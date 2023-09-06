use std::{
    cell::{Cell, RefCell},
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
    command::RenderCommand,
    hooks::{DrawFrame, FrameUpdate, InputEventStatus, MouseState},
    RadApp,
};
use anyhow::*;

pub struct RenderSurface {
    su: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

pub struct RenderWindow {
    surface: RenderSurface,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    clear_color: wgpu::Color,
    pipeline: Arc<wgpu::RenderPipeline>,

    camera: RenderCamera,

    light_render: light::LightRenderer,

    depth_texture: Texture,

    event_loop: Option<Rc<EventLoop<()>>>,
    mouse_state: MouseState,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    command_queue: Vec<RenderCommand>,
}

impl RenderWindow {
    pub const fn handle(&self) -> &Window {
        &self.window
    }
    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn surface_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.su.get_current_texture()
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
    pub fn depth_texture(&self) -> &Texture {
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
        &self.surface.queue
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.surface.device
    }

    #[inline]
    pub fn event_loop(&self) -> Option<Rc<EventLoop<()>>> {
        self.event_loop.clone()
    }

    #[inline]
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.surface.config
    }

    #[inline]
    pub const fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub const fn mouse_state(&self) -> MouseState {
        self.mouse_state
    }

    #[inline]
    pub fn command_queue(&self) -> &Vec<RenderCommand> {
        &self.command_queue
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
                    features: wgpu::Features::empty(),
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

        let surface = RenderSurface {
            su: surface,
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

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    camera.layout().as_ref(),
                    // &light_bind_group_layout,
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
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[Vertex::buffer_layout(), InstanceRaw::buffer_layout()],
                shader,
            )
        };

        let depth_texture = Texture::depth_texture(&device, &config, "Depth Texture".into());

        let light_render = light::LightRenderer::new(
            &surface.device,
            surface.config.format,
            camera.layout().as_ref(),
        );

        let s = Self {
            surface,
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
            command_queue: Vec::new(),
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

    pub fn draw_light_model(&mut self, model: &Model) {
        self.draw_light_model_instanced(model, 0..1);
    }
    pub fn draw_light_model_instanced(&mut self, model: &Model, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(self.light_render.pipeline()));

        let cmds = draw_light_model_instanced(
            model,
            instances,
            self.camera.bind_group(),
            self.light_render.bind_group(),
        );
        self.command_queue.extend(cmds);
    }
    pub fn draw_light_mesh(&mut self, mesh: &Mesh) {
        self.draw_light_mesh_instanced(mesh, 0..1);
    }
    pub fn draw_light_mesh_instanced(&mut self, mesh: &Mesh, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(self.light_render.pipeline()));
        let cmds = draw_light_mesh_instanced(
            mesh,
            instances,
            self.camera.bind_group(),
            self.light_render.bind_group(),
        );
        self.command_queue.extend(cmds);
    }
    pub fn draw_mesh(&mut self, mesh: &Mesh, mat: &Material) {
        self.draw_mesh_instanced(mesh, mat, 0..1);
    }
    pub fn draw_mesh_instanced(&mut self, mesh: &Mesh, mat: &Material, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(self.pipeline.clone()));

        let cmds = draw_mesh_instanced(
            mesh,
            mat,
            instances,
            self.camera.bind_group(),
            self.light_render.bind_group(),
        );
        self.command_queue.extend(cmds);
    }

    pub fn draw_model(&mut self, model: &Model) {
        self.draw_model_instanced(model, 0..1);
    }
    pub fn draw_model_instanced(&mut self, model: &Model, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(self.pipeline.clone()));

        let cmds = draw_model_instanced(
            model,
            instances,
            self.camera.bind_group(),
            self.light_render.bind_group(),
        );
        self.command_queue.extend(cmds);
    }

    pub fn draw_frame<D>(&self, drawer: &mut D) -> Result<(), wgpu::SurfaceError>
    where
        D: DrawFrame,
    {
        let mut encoder =
            self.surface
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Command Encoder"),
                });

        let frame = self.surface_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            let mut ctx = DrawCtx::from_window(self);
            drawer.draw_frame(&mut ctx)?;
            // render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            // render_pass.set_pipeline(&self.light_render_pipeline);
            // draw_light_model(
            // &self.obj_model,
            // &self.cam_bind_group,
            // &self.light_bind_group,
            // );
            // render_pass.set_pipeline(&self.render_pipeline);const
            // draw_model_instanced(
            // &self.obj_model,
            // 0..self.instances.len() as u32,
            // &self.cam_bind_group,
            // &self.light_bind_group, // NEW
            // );
        }

        self.surface.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        std::result::Result::Ok(())
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
            self.surface.config.width = new_size.width;
            self.surface.config.height = new_size.height;
            self.surface
                .su
                .configure(&self.surface.device, &self.surface.config);
            self.depth_texture = Texture::depth_texture(
                &self.surface.device,
                &self.surface.config,
                Some("Depth Texture"),
            );
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
    /// Creates a RenderCamera from a RenderWindow with default values.
    pub fn from_surface(rs: &RenderSurface) -> Self {
        let camera = Camera::new((0.0, 5.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection = Projection::new(
            rs.config.width,
            rs.config.height,
            cgmath::Deg(45.0),
            0.1,
            100.0,
        );

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
