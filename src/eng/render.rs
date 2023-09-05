use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
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
    pipeline: wgpu::RenderPipeline,

    camera: RenderCamera,

    light_render: light::LightRenderer,

    depth_texture: Texture,

    event_loop: Option<Rc<EventLoop<()>>>,
    mouse_state: MouseState,
    texture_bind_group_layout: wgpu::BindGroupLayout,
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
    pub fn depth_texture(&self) -> &Texture {
        &self.depth_texture
    }

    #[inline]
    pub fn light_bind_group(&self) -> &wgpu::BindGroup {
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
                    camera.layout(),
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

        let light_render =
            light::LightRenderer::new(&surface.device, surface.config.format, camera.layout());

        let s = Self {
            surface,
            size,
            window,
            clear_color: wgpu::Color::BLACK,
            pipeline: render_pipeline,
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

    pub fn draw_light_model<'a>(&'a self, ctx: &DrawCtx<'_>, model: &'a Model) {
        self.draw_light_model_instanced(ctx, model, 0..1);
    }
    pub fn draw_light_model_instanced<'a>(
        &'a self,
        ctx: &DrawCtx<'_>,
        model: &'a Model,
        instances: Range<u32>,
    ) {
        ctx.set_pipeline(self.light_render.pipeline());
        ctx.draw_light_model_instanced(
            model,
            self.camera.bind_group(),
            self.light_render.bind_group(),
            instances,
        );
    }
    pub fn draw_light_mesh<'a>(&'a self, mesh: &'a Mesh, rp: &'a mut RenderPass<'a>) {
        self.draw_light_mesh_instanced(mesh, 0..1, rp);
    }
    pub fn draw_light_mesh_instanced<'a>(
        &'a self,
        mesh: &'a Mesh,
        instances: Range<u32>,
        rp: &'a mut RenderPass<'a>,
    ) {
        rp.set_pipeline(self.light_render.pipeline());
        draw_light_mesh_instanced(
            rp,
            mesh,
            instances,
            self.camera.bind_group(),
            self.light_render.bind_group(),
        );
    }
    pub fn draw_mesh<'a>(&'a self, mesh: &'a Mesh, mat: &'a Material, rp: &'a mut RenderPass<'a>) {
        self.draw_mesh_instanced(mesh, mat, 0..1, rp);
    }
    pub fn draw_mesh_instanced<'a>(
        &'a self,
        mesh: &'a Mesh,
        mat: &'a Material,
        instances: Range<u32>,
        rp: &'a mut RenderPass<'a>,
    ) {
        rp.set_pipeline(self.light_render.pipeline());
        draw_mesh_instanced(
            rp,
            mesh,
            mat,
            instances,
            self.camera.bind_group(),
            self.light_render.bind_group(),
        );
    }

    pub fn draw_model<'a>(&self, ctx: &DrawCtx<'_>, model: &'a Model) {
        self.draw_model_instanced(ctx, model, 0..1);
    }
    pub fn draw_model_instanced<'a>(
        &self,
        ctx: &DrawCtx<'_>,
        model: &'a Model,
        instances: Range<u32>,
    ) {
        ctx.set_pipeline(self.light_render.pipeline());

        ctx.draw_model_instanced(
            model,
            self.camera.bind_group(),
            self.light_render.bind_group(),
            instances,
        )
    }

    pub fn draw_frame<D>(&self, drawer: &D) -> Result<(), wgpu::SurfaceError>
    where
        D: DrawFrame,
    {
        let encoder = self
            .surface
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Command Encoder"),
            });
        let encoder = RefCell::new(encoder);

        let frame = self.surface_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // window: &'a RenderWindow,
        // frame: wgpu::SurfaceTexture,
        // view: &'a TextureView,
        // queue: &'a wgpu::Queue,
        // encoder: &'a mut wgpu::CommandEncoder,

        let draw_ctx = DrawCtx::begin(&self, frame, &view, &self.surface.queue, encoder);

        drawer.draw_frame(&draw_ctx);

        draw_ctx.submit(encoder.into_inner());
        std::result::Result::Ok(())
    }

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
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
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

        Self {
            cam: pan_cam,
            buffer: cam_buffer,
            bind_group: cam_bind_group,
            layout: cam_bind_group_layout,
            projection,
        }
    }

    pub const fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub const fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
    pub const fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
    pub const fn projection(&self) -> &Projection {
        &self.projection
    }
    pub fn projection_mut(&mut self) -> &mut Projection {
        &mut self.projection
    }
}

pub mod light {
    use std::ops::Range;

    use wgpu::{util::DeviceExt, Device, RenderPass};

    use crate::gfx::{
        light::LightUniform,
        model::{Mesh, Model},
        wgpu::{buffer::create_render_pipeline, texture::Texture, vertex::Vertex},
    };

    pub struct LightRenderer {
        render_pipeline: wgpu::RenderPipeline,
        uniform: LightUniform,
        buffer: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
        layout: wgpu::BindGroupLayout,
    }

    impl LightRenderer {
        pub const fn bind_group(&self) -> &wgpu::BindGroup {
            &self.bind_group
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

            Self {
                render_pipeline,
                uniform,
                buffer,
                bind_group,
                layout,
            }
        }
        pub const fn pipeline(&self) -> &wgpu::RenderPipeline {
            &self.render_pipeline
        }
    }

    pub fn draw_light_mesh<'a, 'b>(
        rp: &mut RenderPass<'a>,
        mesh: &'b Mesh,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        draw_light_mesh_instanced(rp, mesh, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_light_mesh_instanced<'a, 'b>(
        rp: &mut RenderPass<'a>,
        mesh: &'b Mesh,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        rp.set_vertex_buffer(0, mesh.vert_buff.slice(..));
        rp.set_index_buffer(mesh.index_buff.slice(..), wgpu::IndexFormat::Uint32);
        rp.set_bind_group(0, camera_bind_group, &[]);
        rp.set_bind_group(1, light_bind_group, &[]);
        rp.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    pub fn draw_light_model<'a, 'b>(
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        draw_light_model_instanced(rp, model, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_light_model_instanced<'a, 'b>(
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        for mesh in &model.meshes {
            draw_light_mesh_instanced(
                rp,
                mesh,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}

pub mod mesh {
    use std::ops::Range;

    use wgpu::RenderPass;

    use crate::gfx::model::{Material, Mesh, Model};

    pub fn draw_mesh<'a, 'b>(
        rp: &mut RenderPass<'a>,
        mesh: &'b Mesh,
        mat: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        draw_mesh_instanced(rp, mesh, mat, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_mesh_instanced<'a, 'b>(
        rp: &mut RenderPass<'a>,
        mesh: &'b Mesh,
        mat: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        rp.set_vertex_buffer(0, mesh.vert_buff.slice(..));
        rp.set_index_buffer(mesh.index_buff.slice(..), wgpu::IndexFormat::Uint32);
        rp.set_bind_group(0, &mat.bind_group, &[]);
        rp.set_bind_group(1, camera_bind_group, &[]);
        rp.set_bind_group(2, light_bind_group, &[]);
        rp.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    pub fn draw_model<'a, 'b>(
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        draw_model_instanced(rp, model, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_model_instanced<'a, 'b>(
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        for mesh in &model.meshes {
            let mat = &model.materials[mesh.material];
            draw_mesh_instanced(
                rp,
                mesh,
                mat,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}
pub type RenderWindowMut = Rc<RefCell<RenderWindow>>;

pub struct EncoderCtx<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    view: Rc<wgpu::TextureView>,
}
pub struct DrawCtx<'a> {
    pub window: &'a RenderWindow,

    pub render_pass: RefCell<RenderPass<'a>>,
    view: &'a wgpu::TextureView,
    queue: &'a wgpu::Queue,

    surface: wgpu::SurfaceTexture,
    encoder: RefCell<wgpu::CommandEncoder>,
}
impl<'a> DrawCtx<'a> {
    pub fn begin(
        window: &'a RenderWindow,
        frame: wgpu::SurfaceTexture,
        view: &'a TextureView,
        queue: &'a wgpu::Queue,
        encoder: RefCell<wgpu::CommandEncoder>,
    ) -> Self {
        let render_pass = encoder
            .borrow_mut()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &window.depth_texture().view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

        let ctx = DrawCtx {
            window,
            render_pass: RefCell::new(render_pass),
            view,
            queue,
            surface: frame,
            encoder,
        };
        ctx
    }

    pub fn submit(self, encoder: wgpu::CommandEncoder) {
        self.queue.submit(std::iter::once(encoder.finish()));
        self.surface.present();
    }

    pub fn set_vertex_buffer(&self, slot: u32, buffer: &wgpu::Buffer) {
        self.render_pass
            .get_mut()
            .set_vertex_buffer(slot, buffer.slice(..));
    }

    // rp.set_vertex_buffer(0, mesh.vert_buff.slice(..));
    // rp.set_index_buffer(mesh.index_buff.slice(..), wgpu::IndexFormat::Uint32);
    // rp.set_bind_group(0, camera_bind_group, &[]);
    // rp.set_bind_group(1, light_bind_group, &[]);
    // rp.draw_indexed(0..mesh.num_elements, 0, instances);
    pub fn set_index_buffer(&self, buffer: &wgpu::Buffer, index_format: wgpu::IndexFormat) {
        self.render_pass
            .get_mut()
            .set_index_buffer(buffer.slice(..), index_format);
    }

    pub fn set_bind_group(
        &self,
        index: u32,
        bind_group: &wgpu::BindGroup,
        offsets: &[DynamicOffset],
    ) {
        self.render_pass
            .get_mut()
            .set_bind_group(index, bind_group, offsets);
    }

    pub fn draw_indexed(&self, indices: Range<u32>, base_vert: u32, instances: Range<u32>) {
        self.render_pass
            .get_mut()
            .draw_indexed(indices, base_vert as i32, instances);
    }

    pub fn set_pipeline(&self, pipeline: &wgpu::RenderPipeline) {
        self.render_pass.get_mut().set_pipeline(pipeline);
    }

    pub fn draw_model_instanced(
        &self,
        model: &Model,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
        instances: Range<u32>,
    ) {
        draw_model_instanced(
            self.render_pass.get_mut(),
            model,
            instances,
            camera_bind_group,
            light_bind_group,
        );
    }

    pub fn draw_light_model_instanced(
        &self,
        model: &Model,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
        instances: Range<u32>,
    ) {
        draw_light_model_instanced(
            self.render_pass.get_mut(),
            model,
            instances,
            camera_bind_group,
            light_bind_group,
        )
    }
}
