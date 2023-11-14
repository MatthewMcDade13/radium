use std::{cell::RefCell, rc::Rc};

use anyhow::*;

use crate::gfx::{
    camera::PanCamera,
    geom::{QuadBuffer, Rect},
    shader::{Shader, Uniform},
    wgpu_util::{
        buffer::{BufferType, GpuBuffer},
        texture::{Texture, TextureType},
        vertex::Vertex2D,
    },
    window::RenderWindow,
};

pub struct Renderer2D {
    pub window: RenderWindow,
    pub verts: GpuBuffer,
    pub indices: GpuBuffer,
    pub quads: QuadBuffer,
    pub shader: Shader,
    pub cam: PanCamera,
}

impl Renderer2D {
    pub const VERT_BUFFER_SIZE: u64 = std::mem::size_of::<Vertex2D>() as u64 * 4 * 1042;
    pub const INDEX_BUFFER_SIZE: u64 = std::mem::size_of::<u32>() as u64 * 6 * 1042;

    pub fn new(window: RenderWindow) -> anyhow::Result<Self> {
        let device = window.device_surface.as_ref();
        let verts = GpuBuffer::empty(device, Self::VERT_BUFFER_SIZE, BufferType::Vertex);
        let indices = GpuBuffer::empty(device, Self::INDEX_BUFFER_SIZE, BufferType::Index);
        let quads = QuadBuffer::empty();

        let config = &window.config;
        let cam = PanCamera::new(
            Rect {
                x: 0.,
                y: 0.,
                w: config.width as f32,
                h: config.height as f32,
            },
            4.0,
            0.4,
        );

        let white_texture = Rc::new(Texture::from_bytes(
            &device.device,
            window.device_surface.queue.as_ref(),
            &Texture::DEFAULT,
            TextureType::Diffuse,
            Some("Default White Texture"),
        )?);

        let shader_src = include_str!("../shaders/sprite.wgsl");

        let shader = Shader::new(
            shader_src,
            device,
            config.format,
            &[Vertex2D::buffer_layout()],
            &[
                Uniform::from_texture(device, &white_texture),
                cam.create_uniform(device),
            ],
            Some("Default Sprite Shader"),
        );

        let s = Self {
            window,
            verts,
            indices,
            quads,
            cam,
            shader,
        };
        Ok(s)
    }

    pub fn resize_window(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.window.resize(new_size);
        self.cam.projection.resize(new_size.width, new_size.height);
    }
}

// pub mod light {
//     use std::{ops::Range, rc::Rc, sync::Arc};
//
//     use wgpu::{util::DeviceExt, Device, RenderPass};
//
//     use crate::{
//         eng::command::GpuCommand,
//         gfx::{
//             light::LightUniform,
//             model::{Mesh, Model},
//             shader::create_render_pipeline,
//             wgpu_util::{texture::Texture, vertex::Vertex3D},
//         },
//     };
//
//     pub struct LightRenderer {
//         render_pipeline: Rc<wgpu::RenderPipeline>,
//         uniform: LightUniform,
//         buffer: Rc<wgpu::Buffer>,
//         bind_group: Rc<wgpu::BindGroup>,
//         layout: Rc<wgpu::BindGroupLayout>,
//     }
//
//     impl LightRenderer {
//         pub fn bind_group(&self) -> Rc<wgpu::BindGroup> {
//             self.bind_group.clone()
//         }
//         pub fn new(
//             device: &Device,
//             format: wgpu::TextureFormat,
//             cam_bind_group_layout: &wgpu::BindGroupLayout,
//         ) -> Self {
//             let uniform = LightUniform {
//                 position: [2.0, 2.0, 2.0, 0.0],
//                 color: [1.0, 1.0, 1.0, 0.0],
//             };
//
//             let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
//                 label: Some("Light VB"),
//                 contents: bytemuck::cast_slice(&[uniform]),
//                 usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
//             });
//
//             let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//                 entries: &[wgpu::BindGroupLayoutEntry {
//                     binding: 0,
//                     visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
//                     ty: wgpu::BindingType::Buffer {
//                         ty: wgpu::BufferBindingType::Uniform,
//                         has_dynamic_offset: false,
//                         min_binding_size: None,
//                     },
//                     count: None,
//                 }],
//                 label: None,
//             });
//
//             let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
//                 layout: &layout,
//                 entries: &[wgpu::BindGroupEntry {
//                     binding: 0,
//                     resource: buffer.as_entire_binding(),
//                 }],
//                 label: None,
//             });
//
//             let render_pipeline = {
//                 let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//                     label: Some("Light Pipeline Layout"),
//                     bind_group_layouts: &[cam_bind_group_layout, &layout],
//                     push_constant_ranges: &[],
//                 });
//                 let shader = wgpu::ShaderModuleDescriptor {
//                     label: Some("Light Shader"),
//                     source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/light.wgsl").into()),
//                 };
//                 create_render_pipeline(
//                     &device,
//                     &layout,
//                     format,
//                     Some(Texture::DEPTH_FORMAT),
//                     &[Vertex3D::buffer_layout()],
//                     shader,
//                 )
//             };
//
//             let render_pipeline = Rc::new(render_pipeline);
//             let buffer = Rc::new(buffer);
//             let bind_group = Rc::new(bind_group);
//             let layout = Rc::new(layout);
//
//             Self {
//                 render_pipeline,
//                 uniform,
//                 buffer,
//                 bind_group,
//                 layout,
//             }
//         }
//         pub fn pipeline(&self) -> Rc<wgpu::RenderPipeline> {
//             self.render_pipeline.clone()
//         }
//
//         pub fn layout(&self) -> Rc<wgpu::BindGroupLayout> {
//             self.layout.clone()
//         }
//     }
//
//     pub fn draw_light_mesh(
//         mesh: &Mesh,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         draw_light_mesh_instanced(mesh, 0..1, camera_bind_group, light_bind_group)
//     }
//
//     pub fn draw_light_mesh_instanced(
//         mesh: &Mesh,
//         instances: Range<u32>,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         vec![
//             GpuCommand::SetVertexBuffer(0, mesh.vert_buff.clone()),
//             GpuCommand::SetIndexBuffer(mesh.index_buff.clone(), wgpu::IndexFormat::Uint32),
//             GpuCommand::SetBindGroup(0, camera_bind_group.clone(), None),
//             GpuCommand::SetBindGroup(1, light_bind_group.clone(), None),
//             GpuCommand::DrawIndexed(0..mesh.num_elements, 0, instances),
//         ]
//     }
//
//     pub fn draw_light_model(
//         model: &Model,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         draw_light_model_instanced(
//             model,
//             0..1,
//             camera_bind_group.clone(),
//             light_bind_group.clone(),
//         )
//     }
//
//     pub fn draw_light_model_instanced(
//         model: &Model,
//         instances: Range<u32>,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         let mut buffer = Vec::new();
//         for mesh in &model.meshes {
//             let cmds = draw_light_mesh_instanced(
//                 mesh,
//                 instances.clone(),
//                 camera_bind_group.clone(),
//                 light_bind_group.clone(),
//             );
//             buffer.extend(cmds);
//         }
//         buffer
//     }
// }

// pub mod mesh {
//     use std::{ops::Range, rc::Rc, sync::Arc};
//
//     use wgpu::RenderPass;
//
//     use crate::{
//         eng::command::GpuCommand,
//         gfx::model::{Material, Mesh, Model},
//     };
//
//     pub fn draw_mesh(
//         mesh: &Mesh,
//         mat: &Material,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         draw_mesh_instanced(
//             mesh,
//             mat,
//             0..1,
//             camera_bind_group.clone(),
//             light_bind_group.clone(),
//         )
//     }
//
//     pub fn draw_mesh_instanced(
//         mesh: &Mesh,
//         mat: &Material,
//         instances: Range<u32>,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         vec![
//             GpuCommand::SetVertexBuffer(0, mesh.vert_buff.clone()),
//             GpuCommand::SetIndexBuffer(mesh.index_buff.clone(), wgpu::IndexFormat::Uint32),
//             GpuCommand::SetBindGroup(0, mat.bind_group.clone(), None),
//             GpuCommand::SetBindGroup(1, camera_bind_group.clone(), None),
//             GpuCommand::SetBindGroup(2, light_bind_group.clone(), None),
//             GpuCommand::DrawIndexed(0..mesh.num_elements, 0, instances),
//         ]
//     }
//
//     pub fn draw_model(
//         model: &Model,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group)
//     }
//
//     pub fn draw_model_instanced(
//         model: &Model,
//         instances: Range<u32>,
//         camera_bind_group: Rc<wgpu::BindGroup>,
//         light_bind_group: Rc<wgpu::BindGroup>,
//     ) -> Vec<GpuCommand> {
//         let mut buffer = Vec::new();
//         for mesh in &model.meshes {
//             let mat = &model.materials[mesh.material];
//
//             let cmds = draw_mesh_instanced(
//                 mesh,
//                 mat,
//                 instances.clone(),
//                 camera_bind_group.clone(),
//                 light_bind_group.clone(),
//             );
//             buffer.extend(cmds);
//         }
//         buffer
//     }
// }
