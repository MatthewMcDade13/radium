use cgmath::{Vector3, Zero};

// TODO :: Finish this
#[derive(Debug, Clone)]
pub struct Transform {
    position: cgmath::Vector3<f32>,
    size: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
    model: cgmath::Matrix4<f32>,
    needs_update: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector3::zero(),
            size: Vector3::new(1.0, 1.0, 1.0),
            rotation: cgmath::Quaternion::zero(),
            model: cgmath::Matrix4::zero(),
            needs_update: false,
        }
    }
}
