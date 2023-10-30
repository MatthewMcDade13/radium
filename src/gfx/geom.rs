use super::wgpu_util::vertex::Vertex2D;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl From<Quad> for Rect {
    fn from(value: Quad) -> Self {
        Self {
            x: value.origin.x,
            y: value.origin.y,
            w: value.size.x,
            h: value.size.y,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Quad {
    origin: cgmath::Vector2<f32>,
    size: cgmath::Vector2<f32>,
}

impl From<Rect> for Quad {
    fn from(value: Rect) -> Self {
        Self {
            origin: cgmath::Vector2::new(value.x, value.y),
            size: cgmath::Vector2::new(value.w, value.h),
        }
    }
}

impl Quad {
    pub fn as_verts(&self) -> Vec<Vertex2D> {
        let hh = self.size.y * 0.5;
        let hl = self.size.x * 0.5;

        let top_left = [self.origin.x - hl, self.origin.y - hh, 0.];
        let top_right = [self.origin.x + hl, self.origin.y - hh, 0.];
        let bottom_right = [self.origin.x + hl, self.origin.y + hh, 0.];
        let bottom_left = [self.origin.x - hl, self.origin.y + hh, 0.];

        vec![
            Vertex2D::new(&top_left, &[0., 0.]),
            Vertex2D::new(&top_right, &[1., 0.]),
            Vertex2D::new(&bottom_right, &[1., 1.]),
            Vertex2D::new(&bottom_left, &[0., 1.]),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct QuadBuffer {
    vert_buf: Vec<Vertex2D>,
    index_buf: Vec<usize>,
    current: usize,
}

impl QuadBuffer {
    pub fn empty() -> Self {
        Self {
            vert_buf: Vec::with_capacity(16),
            index_buf: Vec::with_capacity(32),
            current: 0,
        }
    }

    pub fn push_quad(&mut self, quad: &Quad) {
        let verts = quad.as_verts();
        self.vert_buf.extend(verts);

        self.index_buf.extend(&[
            self.current * 4 + 0,
            self.current * 4 + 1,
            self.current * 4 + 2,
            self.current * 4 + 0,
            self.current * 4 + 2,
            self.current * 4 + 3,
        ]);
        self.current += 1;
    }

    pub fn push_with_xform(&mut self, quad: &Quad, xform: &cgmath::Matrix4<f32>) {
        let verts = translate_verts(quad.as_verts().as_slice(), xform);
        self.vert_buf.extend(verts);

        self.index_buf.extend(&[
            self.current * 4 + 0,
            self.current * 4 + 1,
            self.current * 4 + 2,
            self.current * 4 + 0,
            self.current * 4 + 2,
            self.current * 4 + 3,
        ]);
        self.current += 1;
    }
}

fn translate_verts(verts: &[Vertex2D], xform: &cgmath::Matrix4<f32>) -> Vec<Vertex2D> {
    let mut result = Vec::with_capacity(verts.len());
    for v in verts.iter() {
        let pos = cgmath::Vector4::new(v.position[0], v.position[1], v.position[2], 1.);
        let new_pos = xform * pos;
        let new_pos = [new_pos.x, new_pos.y, new_pos.z];
        result.push(Vertex2D::new(&new_pos, &v.tex_coords))
    }
    result
}

fn normalize_texture_coords(
    verts: &mut [Vertex2D],
    texture_rect: &Rect,
    texture_size: &cgmath::Vector2<f32>,
) {
    let texture_rect_size = cgmath::Vector2::new(texture_rect.w, texture_rect.h);
    let uv_offset = cgmath::Vector2::new(texture_rect.x, texture_rect.y);

    for v in verts.iter_mut() {
        let flipped = cgmath::Vector2::new(v.tex_coords[0], 1. - v.tex_coords[1]);

        let texture_dim = cgmath::Vector2::new(
            texture_rect_size.x * flipped.x,
            texture_rect_size.y * flipped.y,
        );
        let texture_coord = texture_dim + uv_offset;
        let normalized = cgmath::Vector2::new(
            texture_coord.x / texture_size.x,
            texture_coord.y / texture_size.y,
        );
        v.tex_coords = [normalized.x, normalized.y];
    }
}
