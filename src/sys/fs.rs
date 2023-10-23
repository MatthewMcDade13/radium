use std::{
    io::{BufReader, Cursor},
    sync::Arc,
};
const TEMP: u32 = 0;

use cfg_if::cfg_if;
use wgpu::util::DeviceExt;

use crate::gfx::{
    model::{Material, Mesh, Model},
    wgpu::{
        texture::{self, TextureType},
        vertex::Vertex3D,
    },
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
    ty: TextureType,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_to_bytes(filename).await?;
    texture::Texture::from_bytes(device, queue, &data, ty, Some(filename))
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
        let diffuse_texture = {
            let dt = m
                .diffuse_texture
                .as_ref()
                .expect("Failed to find diffuse texture for object materail");
            load_texture(dt, TextureType::Diffuse, device, queue).await?
        };

        // TODO :: Probably want to make Normal textures optional? not sure yet.
        let normal_texture = {
            let nt = m
                .normal_texture
                .as_ref()
                .expect("Failed to find normal texture for object material");
            load_texture(nt, TextureType::Normal, device, queue).await?
        };

        materials.push(Material::new(
            device,
            diffuse_texture,
            normal_texture,
            layout,
            Some(&m.name),
        ));
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let mut verticies = (0..m.mesh.positions.len() / 3)
                .map(|i| Vertex3D {
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
                    ..Default::default()
                })
                .collect::<Vec<_>>();

            let indicies = &m.mesh.indices;
            let mut triangels_included = vec![0; verticies.len()];

            // Calculate tangents and bitangents using Triangles.
            // Loop through indicies in chunks of 3
            for c in indicies.chunks(3) {
                let v0 = verticies[c[0] as usize];
                let v1 = verticies[c[1] as usize];
                let v2 = verticies[c[2] as usize];

                let pos0: cgmath::Vector3<_> = v0.position.into();
                let pos1: cgmath::Vector3<_> = v1.position.into();
                let pos2: cgmath::Vector3<_> = v2.position.into();

                let uv0: cgmath::Vector2<_> = v0.tex_coords.into();
                let uv1: cgmath::Vector2<_> = v1.tex_coords.into();
                let uv2: cgmath::Vector2<_> = v2.tex_coords.into();

                // Calc edges of triangle
                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                // Gives us a direction to calc the tangent and bitangent
                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                // System of Equations solves for tangent and bitangent
                // delta_pos1 = delta_uv1.x * T + delta_u.y * B
                // delta_pos2 = delta_uv2.x * T + delta_uv2.y * B
                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;

                // Flip bitangent to enable right-handed normal
                // maps with wgpu texture coordinate system.
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

                // Use the same tangent and bitangent for each vertex in the triangle.
                verticies[c[0] as usize].tangent =
                    (tangent + cgmath::Vector3::from(verticies[c[0] as usize].tangent)).into();
                verticies[c[1] as usize].tangent =
                    (tangent + cgmath::Vector3::from(verticies[c[1] as usize].tangent)).into();
                verticies[c[2] as usize].tangent =
                    (tangent + cgmath::Vector3::from(verticies[c[2] as usize].tangent)).into();

                verticies[c[0] as usize].bitangent =
                    (bitangent + cgmath::Vector3::from(verticies[c[0] as usize].bitangent)).into();
                verticies[c[1] as usize].bitangent =
                    (bitangent + cgmath::Vector3::from(verticies[c[1] as usize].bitangent)).into();
                verticies[c[2] as usize].bitangent =
                    (bitangent + cgmath::Vector3::from(verticies[c[2] as usize].bitangent)).into();

                // used to average the tangents and bitangents.
                triangels_included[c[0] as usize] += 1;
                triangels_included[c[1] as usize] += 1;
                triangels_included[c[2] as usize] += 1;
            }

            // Average the tangents and bitangents.
            for (i, n) in triangels_included.into_iter().enumerate() {
                let denom = 1.0 / n as f32;
                let v = &mut verticies[i];
                v.tangent = (cgmath::Vector3::from(v.tangent) * denom).into();
                v.bitangent = (cgmath::Vector3::from(v.bitangent) * denom).into();
            }

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
            let vert_buff = Arc::new(vert_buff);
            let index_buff = Arc::new(index_buff);
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
