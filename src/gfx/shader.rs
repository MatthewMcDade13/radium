use std::rc::Rc;

use wgpu::{util::DeviceExt, VertexBufferLayout};

use crate::eng::command::{CommandQueue, GpuCommand};

pub fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}

pub enum BufferType {
    Index,
    Vertex,
}

pub struct StagingBuffer {
    pub buf: Rc<wgpu::Buffer>,
    pub size: wgpu::BufferAddress,
}

impl StagingBuffer {
    pub fn new<T>(ds: &DeviceSurface, data: &[T], buffer_type: BufferType) -> Self
    where
        T: bytemuck::Pod + Sized,
    {
        let usage = wgpu::BufferUsages::COPY_SRC
            | match buffer_type {
                BufferType::Index => wgpu::BufferUsages::INDEX,
                BufferType::Vertex => wgpu::BufferUsages::VERTEX,
            };
        let buf = ds
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Staging Buffer"),
                contents: bytemuck::cast_slice(data),
                usage,
            });

        let buf = Rc::new(buf);
        let size = (std::mem::size_of::<T>() * data.len()) as u64;

        Self { buf, size }
    }
}

#[derive(Debug, Clone)]
pub enum UniformData {
    Texture(Rc<Texture>),
    Buffer(Rc<wgpu::Buffer>),
}

#[derive(Debug, Clone)]
pub struct Uniform {
    pub bgroup: Rc<wgpu::BindGroup>,
    pub layout: Rc<wgpu::BindGroupLayout>,
    pub data: UniformData,
}

impl Uniform {
    pub fn new<T>(ds: &DeviceSurface, data: &[T]) -> Self
    where
        T: bytemuck::Pod + Sized,
    {
        let buf = ds
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let layout = ds
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("Uniform Buffer Bind Group Layout"),
            });

        let bgroup = ds.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
            label: Some("Uniform Buffer Bind Group"),
        });

        let bgroup = Rc::new(bgroup);
        let layout = Rc::new(layout);
        let buf = Rc::new(buf);
        Self {
            bgroup,
            layout,
            data: UniformData::Buffer(buf),
        }
    }

    pub fn texture_file(ds: &DeviceSurface, texture_path: &str) -> anyhow::Result<Self> {
        let bytes = std::fs::read_to_string(texture_path)?;
        let texture = Texture::from_bytes(
            &ds.device,
            &ds.queue,
            bytes.as_bytes(),
            TextureType::Diffuse,
            Some("Texture Uniform"),
        )?;
        let texture = Rc::new(texture);
        let s = Self::from_texture(ds, &texture);
        Ok(s)
    }

    pub fn from_texture(ds: &DeviceSurface, texture: &Rc<Texture>) -> Self {
        let layout = ds
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("Uniform Texture Bind Group Layout"),
            });

        let bgroup = ds.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
            label: Some("Uniform Texture Bind Group"),
        });

        let layout = Rc::new(layout);
        let bgroup = Rc::new(bgroup);

        Self {
            layout,
            bgroup,
            data: UniformData::Texture(texture.clone()),
        }
    }

    // TODO :: Refactor/Move this to the EncoderCommand Queue (So we don't have to pass in/require
    // a &mut wgpu::CommandEncoder)
    pub fn write_buffer(&self, src: &StagingBuffer, encoder: &mut wgpu::CommandEncoder) {
        match &self.data {
            UniformData::Texture(t) => encoder.copy_buffer_to_texture(
                wgpu::ImageCopyBuffer {
                    buffer: src.buf.as_ref(),
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * t.size.x),
                        rows_per_image: Some(t.size.y),
                    },
                },
                wgpu::ImageCopyTexture {
                    texture: &t.handle,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::default(),
                },
                wgpu::Extent3d {
                    width: t.size.x,
                    height: t.size.y,
                    depth_or_array_layers: 1,
                },
            ),
            UniformData::Buffer(b) => {
                encoder.copy_buffer_to_buffer(&src.buf, 0, b.as_ref(), 0, src.size)
            }
        }
    }
}

use super::{
    wgpu_util::texture::{Texture, TextureType},
    window::DeviceSurface,
};

pub struct Shader {
    pub pipeline: Rc<wgpu::RenderPipeline>,
    uniforms: Vec<Uniform>,
    vert_layouts: Vec<VertexBufferLayout<'static>>,
}

impl Shader {
    pub fn new(
        shader_str: &str,
        device_surface: &DeviceSurface,
        vert_layouts: &[VertexBufferLayout<'static>],
        uniforms: &[Uniform],
        label: Option<&str>,
    ) -> Self {
        let layouts = uniforms
            .iter()
            .map(|u| u.layout.as_ref())
            .collect::<Vec<&wgpu::BindGroupLayout>>();

        let device = &device_surface.device;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shader Pipeline Layout"),
            bind_group_layouts: layouts.as_slice(),
            push_constant_ranges: &[],
        });

        let shader = wgpu::ShaderModuleDescriptor {
            label,
            source: wgpu::ShaderSource::Wgsl(shader_str.into()),
        };

        let pipeline = create_render_pipeline(
            device,
            &pipeline_layout,
            device_surface.config.borrow().format,
            Some(Texture::DEPTH_FORMAT),
            vert_layouts,
            shader,
        );
        let pipeline = Rc::new(pipeline);

        let uniforms = uniforms.to_vec();
        let vert_layouts = vert_layouts.to_vec();

        Self {
            pipeline,
            uniforms,
            vert_layouts,
        }
    }

    pub fn write_uniform(
        &self,
        uniform_index: usize,
        src: &StagingBuffer,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let dst = self.uniform(uniform_index);
        dst.write_buffer(src, encoder);
    }

    pub fn uniform(&self, index: usize) -> Uniform {
        assert!(index < self.uniforms.len());
        self.uniforms[index].clone()
    }

    pub fn activate(&self, cq: &mut CommandQueue) {
        cq.draw_commands
            .push(GpuCommand::SetPipeline(self.pipeline.clone()));
    }
}
