const TEMP: u32 = 0;
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

impl Vertex3D {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x3, 4 => Float32x3];

    pub const fn zero() -> Vertex3D {
        Vertex3D {
            position: [0.; 3],
            tex_coords: [0.; 2],
            normal: [0.; 3],
            tangent: [0.; 3],
            bitangent: [0.; 3],
        }
    }

    pub const fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

impl Default for Vertex3D {
    fn default() -> Self {
        Self::zero()
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex2D {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    _padding: f32,
}

impl Vertex2D {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    pub fn new(position: &[f32; 3], tex_coords: &[f32; 2]) -> Self {
        let position = position.clone();
        let tex_coords = tex_coords.clone();
        Self {
            position,
            tex_coords,
            ..Default::default()
        }
    }

    pub const fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }

    pub const fn zero() -> Self {
        Self {
            position: [0.; 3],
            tex_coords: [0.; 2],
            _padding: 0.,
        }
    }
}

impl Default for Vertex2D {
    fn default() -> Self {
        Self::zero()
    }
}
