use std::f32::consts::FRAC_PI_2;

const TEMP: u32 = 0;
#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;
