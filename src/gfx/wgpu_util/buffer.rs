use std::{rc::Rc, sync::Arc};

use wgpu::{util::DeviceExt, VertexAttribute};

use crate::gfx::window::DeviceSurface;
const TEMP: u32 = 0;

pub struct Instance {
    pub position: cgmath::Vector3<f32>,
    pub rotation: cgmath::Quaternion<f32>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation))
            .into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
        }
    }
}
impl InstanceRaw {
    const ATTRIBS: [VertexAttribute; 7] = wgpu::vertex_attr_array![5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x3, 10 => Float32x3, 11 => Float32x3];
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
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

pub struct GpuBuffer {
    pub buf: Rc<wgpu::Buffer>,
    pub size: wgpu::BufferAddress,
}

impl GpuBuffer {
    pub fn new<T>(ds: &DeviceSurface, data: &[T], buffer_type: BufferType) -> Self
    where
        T: bytemuck::Pod + Sized,
    {
        let usage = wgpu::BufferUsages::COPY_DST
            | match buffer_type {
                BufferType::Index => wgpu::BufferUsages::INDEX,
                BufferType::Vertex => wgpu::BufferUsages::VERTEX,
            };
        let buf = ds
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Gpu Buffer"),
                contents: bytemuck::cast_slice(data),
                usage,
            });

        let buf = Rc::new(buf);
        let size = (std::mem::size_of::<T>() * data.len()) as u64;

        Self { buf, size }
    }

    pub fn empty(ds: &DeviceSurface, size: u64, buffer_type: BufferType) -> Self {
        let usage = wgpu::BufferUsages::COPY_DST
            | match buffer_type {
                BufferType::Index => wgpu::BufferUsages::INDEX,
                BufferType::Vertex => wgpu::BufferUsages::VERTEX,
            };
        let buf = ds.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),

            usage,
            size,
            mapped_at_creation: false,
        });

        let buf = Rc::new(buf);

        Self { buf, size }
    }
}
