use std::io::{BufReader, Cursor};
const TEMP: u32 = 0;

use cfg_if::cfg_if;
use wgpu::util::DeviceExt;

use crate::gfx::{
    model::{Material, Mesh, Model},
    wgpu::{texture, vertex::Vertex},
};

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let win = web_sys::window().expect("Error loading Browser window");
    let location = win.location();
    let mut origin = location
        .origin()
        .expect("Failed loading Window Location Origin");

    if !origin.ends_with("radium") {
        origin = format!("{}/radium", origin);
    }
    let base = reqwest::Url::parse(&format!("{}/", origin)).expect("Failed to parse reqwest URL");
    base.join(file_name)
        .expect("Failed to join URL with filename")
}

pub async fn load_to_bytes(filename: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(filename);
            let data = reqwest::get(url)
                .await?
                .bytes().await?.to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("public")
                .join(filename);

            let data = tokio::fs::read(path).await?;
        }
    }
    Ok(data)
}

pub async fn load_to_str(filename: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(filename);
            let data = reqwest::get(url)
                .await?
                .bytes().await?.to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("public")
                .join(filename);
            let data = tokio::fs::read_to_string(path).await?;
        }
    }
    Ok(data)
}
pub async fn load_texture(
    filename: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_to_bytes(filename).await?;
    texture::Texture::from_bytes(device, queue, &data, Some(filename))
}

pub async fn load_model(
    filename: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
    let obj_text = load_to_str(filename).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |x| async move {
            let mat_text = load_to_str(&x)
                .await
                .expect("unable to load path for material");
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let dt = m
            .diffuse_texture
            .as_ref()
            .expect("Failed to find diffuse texture for object materail");

        let diffuse_texture = load_texture(dt, device, queue).await?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: None,
        });

        materials.push(Material {
            name: m.name,
            diffuse_texture,
            bind_group,
        })
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let verticies = (0..m.mesh.positions.len() / 3)
                .map(|i| Vertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                })
                .collect::<Vec<_>>();

            let vert_buff = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", filename)),
                contents: bytemuck::cast_slice(&verticies),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buff = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", filename)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            Mesh {
                name: filename.to_string(),
                vert_buff,
                index_buff,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(Model { meshes, materials })
}
