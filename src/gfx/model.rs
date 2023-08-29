use super::wgpu::texture::Texture;

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

pub struct Mesh {
    pub name: String,
    pub vert_buff: wgpu::Buffer,
    pub index_buff: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize, // ???
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: Texture,
    pub bind_group: wgpu::BindGroup,
}
const TEMP: u32 = 0;
