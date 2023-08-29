/// Represents a colored point in space.
/// NOTE :: Due to uniforms requiring 16 byte (4 float) spacing, we need to use padding
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 4],

    pub color: [f32; 4],
}
