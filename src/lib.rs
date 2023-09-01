use std::ops::Range;
const TEMP: u32 = 0;

use cgmath::prelude::*;
use eng::hooks::{FrameUpdate, WindowEventHandler};
use gfx::{
    camera::{Camera, CameraControl, CameraUniform, PlayerCamera},
    model::{Material, Mesh, Model},
    wgpu::{
        buffer::{Instance, InstanceRaw},
        texture::Texture,
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
    render_pipeline: wgpu::RenderPipeline,

    num_indices: u32,
    index_buffer: wgpu::Buffer,

    camera: PlayerCamera,
    cam_buffer: wgpu::Buffer,
    cam_bind_group: wgpu::BindGroup,

    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,

    depth_texture: Texture,
    obj_model: Model,

    light_render_pipeline: wgpu::RenderPipeline,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
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

        let camera = Camera {
            eye: (0., 1., 2.).into(),
            target: (0., 0., 0.).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };

        let cam_uniform = CameraUniform::from_camera(&camera);
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

        let player_cam = PlayerCamera {
            cam: camera,
            uniform: cam_uniform,
            ctrl: CameraControl::new(0.2),
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
            num_indices,
            index_buffer,
            instances,
            instance_buffer,
            depth_texture,
            obj_model,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
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
    }
    pub fn draw_light_mesh<'a, 'b>(
        &self,
        rp: &mut RenderPass<'a>,
        mesh: &'b Mesh,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        self.draw_light_mesh_instanced(rp, mesh, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_light_mesh_instanced<'a, 'b>(
        &self,
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

    fn draw_light_model<'a, 'b>(
        &self,
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        self.draw_light_model_instanced(rp, model, 0..1, camera_bind_group, light_bind_group);
    }
    fn draw_light_model_instanced<'a, 'b>(
        &self,
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        for mesh in &model.meshes {
            self.draw_light_mesh_instanced(
                rp,
                mesh,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
    pub fn draw_mesh<'a, 'b>(
        &self,
        rp: &mut RenderPass<'a>,
        mesh: &'b Mesh,
        mat: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        self.draw_mesh_instanced(rp, mesh, mat, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_mesh_instanced<'a, 'b>(
        &self,
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
        &self,
        rp: &mut RenderPass<'a>,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) where
        'b: 'a,
    {
        self.draw_model_instanced(rp, model, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_model_instanced<'a, 'b>(
        &self,
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
            self.draw_mesh_instanced(
                rp,
                mesh,
                mat,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }

    pub fn texture_from_bytes(&self, bytes: &[u8], label: Option<&str>) -> anyhow::Result<Texture> {
        Texture::from_bytes(&self.device, &self.queue, bytes, label)
    }

    pub fn create_depth_texture(&self, label: Option<&str>) -> Texture {
        Texture::depth_texture(&self.device, &self.config, label)
    }

    /// return true if finished with polling inputs
    pub fn input(&mut self, event: &WindowEvent) -> bool {
        self.camera.handle_window_events(event)
    }

    pub fn update(&mut self) {
        self.camera.frame_update(1.0);
        self.camera.uniform = CameraUniform::from_camera(&self.camera.cam);
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
            let cgmath::Vector3 { x, y, z } =
                cgmath::Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(1.0))
                    * old_pos;
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
            self.draw_light_model(
                &mut render_pass,
                &self.obj_model,
                &self.cam_bind_group,
                &self.light_bind_group,
            );
            render_pass.set_pipeline(&self.render_pipeline);
            self.draw_model_instanced(
                &mut render_pass,
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.cam_bind_group,
                &self.light_bind_group, // NEW
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

pub async fn run_loop() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let mut renderer = GfxState::new(window).await?;

    event_loop.run(move |event, _, control_flow| match event {
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
                    WindowEvent::CursorMoved { position, .. } => {
                        let PhysicalSize { width, height } = renderer.size();
                        renderer.set_clear_color(wgpu::Color {
                            r: position.x / width as f64,
                            g: position.x + position.y / (width + height) as f64,
                            b: position.y / height as f64,
                            a: 1.,
                        })
                    }
                    _ => {}
                }
            }
        }
        Event::RedrawRequested(window_id) if window_id == renderer.window().id() => {
            renderer.update();
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
        _ => {}
    });
}
