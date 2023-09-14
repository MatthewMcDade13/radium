use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc, time::Duration};

use actix::{Arbiter, SyncArbiter, System};
use cgmath::prelude::*;
use eng::{
    app::{InputEventStatus, RadApp, Radium},
    command::RenderCommand,
    render::{light::draw_light_model, mesh::draw_model_instanced, RenderWindow},
};
use gfx::{
    camera::{Camera, CameraControl, CameraUniform, PanCamera, Projection},
    draw::DrawCtx,
    model::{Material, Mesh, Model},
    wgpu::{
        buffer::{Instance, InstanceRaw},
        texture::{Texture, TextureType},
        vertex::Vertex,
    },
};
use sys::fs::load_model;
use winit::{
    dpi::PhysicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use wgpu::{util::DeviceExt, RenderPass};

use crate::gfx::{light::LightUniform, wgpu::buffer::create_render_pipeline};

mod eng;
mod gfx;
mod sys;

const NUM_INSTANCES_PER_ROW: u32 = 10;
const INSTANCE_DISPLACEMENT: cgmath::Vector3<f32> = cgmath::Vector3::new(
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
    0.,
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
);

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];
pub struct GfxState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    clear_color: wgpu::Color,
    render_pipeline: Arc<wgpu::RenderPipeline>,

    camera: PanCamera,
    projection: Projection,
    cam_buffer: Arc<wgpu::Buffer>,
    cam_bind_group: Arc<wgpu::BindGroup>,

    mouse_pressed: bool,

    instances: Vec<Instance>,
    instance_buffer: Arc<wgpu::Buffer>,

    depth_texture: Texture,
    obj_model: Model,

    light_render_pipeline: Arc<wgpu::RenderPipeline>,
    light_uniform: LightUniform,
    light_buffer: Arc<wgpu::Buffer>,
    light_bind_group: Arc<wgpu::BindGroup>,
}

impl GfxState {
    pub async fn new(window: Window) -> anyhow::Result<Self> {
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

        let camera = Camera::new((0.0, 5.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection =
            Projection::new(config.width, config.height, cgmath::Deg(45.0), 0.1, 100.0);

        let cam_uniform = CameraUniform::from_camera(&camera, &projection);
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

        let player_cam = PanCamera {
            cam: camera,
            uniform: cam_uniform,
            ctrl: CameraControl::new(4.0, 0.4),
        };

        let light_uniform = LightUniform {
            position: [2.0, 2.0, 2.0, 0.0],
            color: [1.0, 1.0, 1.0, 0.0],
        };

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
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
                label: None,
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &cam_bind_group_layout,
                    &light_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/basic.wgsl").into()),
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

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = INDICES.len() as u32;

        const SPACE_BETWEEN: f32 = 3.0;
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = cgmath::Vector3 { x, y: 0.0, z };

                    let rotation = if position.is_zero() {
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
                    };

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();
        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let depth_texture = Texture::depth_texture(&device, &config, "Depth Texture".into());
        let obj_model = load_model("cube.obj", &device, &queue, &texture_bind_group_layout).await?;

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[&cam_bind_group_layout, &light_bind_group_layout],
                push_constant_ranges: &[],
            });
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            };
            create_render_pipeline(
                &device,
                &layout,
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[Vertex::buffer_layout()],
                shader,
            )
        };
        let cam_buffer = Arc::new(cam_buffer);
        let cam_bind_group = Arc::new(cam_bind_group);
        let instance_buffer = Arc::new(instance_buffer);
        let light_buffer = Arc::new(light_buffer);
        let light_bind_group = Arc::new(light_bind_group);
        let light_render_pipeline = Arc::new(light_render_pipeline);
        let render_pipeline = Arc::new(render_pipeline);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            camera: player_cam,
            cam_buffer,
            cam_bind_group,
            clear_color: wgpu::Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 1.,
            },
            render_pipeline,
            instances,
            instance_buffer,
            depth_texture,
            obj_model,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
            projection,
            mouse_pressed: false,
        })
    }

    pub const fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub const fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = self.create_depth_texture(Some("Depth Texture"));
        }

        self.projection.resize(new_size.width, new_size.height);
    }

    pub fn update(&mut self, dt: Duration) {
        self.camera.frame_update(dt);
        self.camera.uniform = CameraUniform::from_camera(&self.camera.cam, &self.projection);
        self.queue.write_buffer(
            &self.cam_buffer,
            0,
            bytemuck::cast_slice(&[self.camera.uniform]),
        );

        let old_pos = {
            let cgmath::Vector4 { x, y, z, .. } = self.light_uniform.position.into();
            cgmath::Vector3::new(x, y, z)
        };
        self.light_uniform.position = {
            let cgmath::Vector3 { x, y, z } = cgmath::Quaternion::from_axis_angle(
                (0.0, 1.0, 0.0).into(),
                cgmath::Deg(60.0 * dt.as_secs_f32()),
            ) * old_pos;
            [x, y, z, 0.0]
        };
        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }

    pub fn set_clear_color(&mut self, color: wgpu::Color) {
        self.clear_color = color;
    }

    pub fn texture_from_bytes(
        &self,
        ty: TextureType,
        bytes: &[u8],
        label: Option<&str>,
    ) -> anyhow::Result<Texture> {
        Texture::from_bytes(&self.device, &self.queue, bytes, ty, label)
    }

    pub fn create_depth_texture(&self, label: Option<&str>) -> Texture {
        Texture::depth_texture(&self.device, &self.config, label)
    }

    /// return true if finished with polling inputs
    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => self.camera.process_keyboard(*key, *state).into(),
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            _ => false,
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

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
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            render_pass.set_pipeline(&self.light_render_pipeline);

            let mut cmds = Vec::new();
            let mut rp = render_pass;
            cmds.push(RenderCommand::SetPipeline(
                self.light_render_pipeline.clone(),
            ));
            let commands = draw_light_model(
                &self.obj_model,
                self.cam_bind_group.clone(),
                self.light_bind_group.clone(),
            );
            cmds.extend(commands);
            cmds.push(RenderCommand::SetPipeline(self.render_pipeline.clone()));
            let commands = draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                self.cam_bind_group.clone(),
                self.light_bind_group.clone(),
            );
            cmds.extend(commands);

            for cmd in cmds.iter() {
                match cmd {
                    RenderCommand::SetPipeline(pipeline) => rp.set_pipeline(&pipeline),
                    RenderCommand::SetBindGroup(slot, bind_group, offsets) => {
                        let offsets = match offsets {
                            Some(os) => os.as_slice(),
                            None => &[],
                        };
                        rp.set_bind_group(*slot, bind_group.as_ref(), offsets);
                    }
                    RenderCommand::SetBlendConstant(color) => rp.set_blend_constant(*color),
                    RenderCommand::SetIndexBuffer(buffer, index_format) => {
                        rp.set_index_buffer(buffer.slice(..), *index_format)
                    }
                    RenderCommand::SetVertexBuffer(slot, buffer) => {
                        rp.set_vertex_buffer(*slot, buffer.slice(..))
                    }
                    RenderCommand::SetScissorRect(x, y, width, height) => {
                        rp.set_scissor_rect(*x, *y, *width, *height)
                    }
                    RenderCommand::SetViewPort(x, y, w, h, min_depth, max_depth) => {
                        rp.set_viewport(*x, *y, *w, *h, *min_depth, *max_depth)
                    }
                    RenderCommand::SetStencilReference(reference) => {
                        rp.set_stencil_reference(*reference)
                    }
                    RenderCommand::Draw(vertices, instances) => {
                        rp.draw(vertices.clone(), instances.clone())
                    }
                    RenderCommand::InsertDebugMarker(label) => rp.insert_debug_marker(&label),
                    RenderCommand::PushDebugGroup(label) => rp.push_debug_group(&label),
                    RenderCommand::PopDebugGroup => rp.pop_debug_group(),
                    RenderCommand::DrawIndexed(indices, base_vertex, instances) => {
                        rp.draw_indexed(indices.clone(), *base_vertex, instances.clone())
                    }
                    RenderCommand::DrawIndirect(indirect_buffer, indirect_offset) => {
                        rp.draw_indirect(&indirect_buffer, *indirect_offset)
                    }
                    RenderCommand::DrawIndexedIndirect(indirect_buffer, indirect_offset) => {
                        rp.draw_indexed_indirect(&indirect_buffer, *indirect_offset)
                    }
                    RenderCommand::ExecuteBundles() => todo!(),
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

pub async fn _run_loop() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let mut renderer = GfxState::new(window).await?;
    let mut last_dt = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == renderer.window().id() => {
                if !renderer.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => {
                            *control_flow = ControlFlow::Exit;
                        }
                        WindowEvent::Resized(physical_size) => {
                            renderer.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            renderer.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == renderer.window().id() => {
                let now = std::time::Instant::now();
                let dt = now - last_dt;
                last_dt = now;
                renderer.update(dt);
                match renderer.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => renderer.resize(renderer.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                renderer.window().request_redraw();
            }
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if renderer.mouse_pressed {
                    renderer.camera.process_mouse(delta.0, delta.1);
                }
            }
            _ => {}
        }
    });
}

struct Renderer {
    instances: Vec<Instance>,
    instance_buffer: Arc<wgpu::Buffer>,
    obj_model: Model,
    window: Rc<RefCell<RenderWindow>>,
}

impl Renderer {
    pub async fn new(window: Rc<RefCell<RenderWindow>>) -> anyhow::Result<Self> {
        const SPACE_BETWEEN: f32 = 3.0;
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = cgmath::Vector3 { x, y: 0.0, z };

                    let rotation = if position.is_zero() {
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
                    };

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();

        let (instance_buffer, obj_model) = {
            let window = window.borrow();
            let device = window.device();
            let queue = window.device_queue();
            let texture_layout = window.texture_bind_group_layout();
            let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
            let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let obj = load_model("cube.obj", &device, queue, texture_layout).await?;
            (instance_buffer, obj)
        };

        let instance_buffer = Arc::new(instance_buffer);

        Ok(Self {
            instances,
            instance_buffer,
            obj_model,
            window: window.clone(),
        })
    }
}

impl RadApp for Renderer {
    fn frame_update(&mut self, dt: Duration) {

        // self.queue.write_buffer(
        // &self.cam_buffer,
        // 0,
        // bytemuck::cast_slice(&[self.camera.uniform]),
        // );

        // let old_pos = {
        // let cgmath::Vector4 { x, y, z, .. } = self.light_uniform.position.into();
        // cgmath::Vector3::new(x, y, z)
        // };
        // self.light_uniform.position = {
        // let cgmath::Vector3 { x, y, z } = cgmath::Quaternion::from_axis_angle(
        // (0.0, 1.0, 0.0).into(),
        // cgmath::Deg(60.0 * dt.as_secs_f32()),
        // ) * old_pos;
        // [x, y, z, 0.0]
        // };
        // self.queue.write_buffer(
        // &self.light_buffer,
        // 0,
        // bytemuck::cast_slice(&[self.light_uniform]),
        // );
    }

    fn handle_window_events(&mut self, event: &WindowEvent) -> InputEventStatus {
        let mut window = self.window.borrow_mut();
        let camera = window.camera_mut();
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => camera.process_keyboard(*key, *state).into(),
            WindowEvent::MouseWheel { delta, .. } => {
                camera.process_scroll(delta);
                InputEventStatus::Processing
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                // self.mouse_pressed = *state == ElementState::Pressed;
                // true
                InputEventStatus::Processing
            }
            _ => InputEventStatus::Done,
        }
    }

    fn draw_frame(&mut self, ctx: &mut gfx::draw::DrawCtx) -> Result<(), wgpu::SurfaceError> {
        ctx.set_vertex_buffer(1, self.instance_buffer.clone());

        ctx.draw_light_model(&self.obj_model);
        ctx.draw_model_instanced(&self.obj_model, 0..self.instances.len() as u32);

        Ok(())
    }
}

pub async fn run_loop() -> anyhow::Result<()> {
    env_logger::init();

    Radium::start(|rw| Renderer::new(rw)).await?;
    Ok(())
}
