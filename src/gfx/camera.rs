use std::time::Duration;

use cgmath::{perspective, InnerSpace, Matrix4, Point3, Rad, Vector3};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, KeyboardInput, MouseScrollDelta, VirtualKeyCode, WindowEvent},
};

use crate::{
    eng::hooks::{FrameUpdate, ProcessInput, WindowEventHandler},
    sys::math::{OPENGL_TO_WGPU_MATRIX, SAFE_FRAC_PI_2},
};

#[derive(Debug, Copy, Clone)]
pub struct Camera {
    position: Point3<f32>,
    yaw: Rad<f32>,
    pitch: Rad<f32>,
}

impl Camera {
    pub fn new<V, Y, P>(pos: V, yaw: Y, pitch: P) -> Self
    where
        V: Into<Point3<f32>>,
        Y: Into<Rad<f32>>,
        P: Into<Rad<f32>>,
    {
        Self {
            position: pos.into(),
            yaw: yaw.into(),
            pitch: pitch.into(),
        }
    }

    pub fn calc_view_matrix(&self) -> Matrix4<f32> {
        let (pitch_sin, pitch_cos) = self.pitch.0.sin_cos();
        let (yaw_sin, yaw_cos) = self.yaw.0.sin_cos();

        Matrix4::look_to_rh(
            self.position,
            Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize(),
            Vector3::unit_y(),
        )
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_position: [0.0; 4],
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }
}

impl CameraUniform {
    pub fn new(mat4: &[[f32; 4]; 4], view_pos: &[f32; 4]) -> Self {
        Self {
            view_position: view_pos.clone(),
            view_proj: mat4.clone(),
        }
    }

    pub fn from_camera(camera: &Camera, projection: &Projection) -> Self {
        CameraUniform {
            view_position: camera.position.to_homogeneous().into(),
            view_proj: (projection.calc_matrix() * camera.calc_view_matrix()).into(),
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct CameraControl {
    units_left: f32,
    units_right: f32,
    units_forward: f32,
    units_back: f32,
    units_up: f32,
    units_down: f32,
    horizontal_rotation: f32,
    vertical_rotation: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
}

impl CameraControl {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            sensitivity,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PlayerCamera {
    pub cam: Camera,
    pub uniform: CameraUniform,
    pub ctrl: CameraControl,
}

impl PlayerCamera {}

impl FrameUpdate for PlayerCamera {
    fn frame_update(&mut self, dt: Duration) {
        let dt = dt.as_secs_f32();

        // movement froward/back left/right
        let (yaw_sin, yaw_cos) = self.cam.yaw.0.sin_cos();
        let forward = Vector3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right = Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();
        self.cam.position +=
            forward * (self.ctrl.units_forward - self.ctrl.units_back) * self.ctrl.speed * dt;
        self.cam.position +=
            right * (self.ctrl.units_right - self.ctrl.units_left) * self.ctrl.speed * dt;

        // move in/out (zoom)
        let (pitch_sin, pitch_cos) = self.cam.pitch.0.sin_cos();
        let scrollward =
            Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        self.cam.position +=
            scrollward * self.ctrl.scroll * self.ctrl.speed * self.ctrl.sensitivity * dt;
        self.ctrl.scroll = 0.0;

        // move up/down
        self.cam.position.y += (self.ctrl.units_up - self.ctrl.units_down) * self.ctrl.speed * dt;

        // rotate
        self.cam.yaw += Rad(self.ctrl.horizontal_rotation) * self.ctrl.sensitivity * dt;
        self.cam.pitch += Rad(-self.ctrl.vertical_rotation) * self.ctrl.sensitivity * dt;

        self.ctrl.horizontal_rotation = 0.0;
        self.ctrl.vertical_rotation = 0.0;

        // clamp camera angle
        if self.cam.pitch < -Rad(SAFE_FRAC_PI_2) {
            self.cam.pitch = -Rad(SAFE_FRAC_PI_2);
        } else if self.cam.pitch > Rad(SAFE_FRAC_PI_2) {
            self.cam.pitch = Rad(SAFE_FRAC_PI_2);
        }
    }
}

impl ProcessInput for PlayerCamera {
    fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match key {
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.ctrl.units_forward = amount;
                true
            }
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.ctrl.units_back = amount;
                true
            }
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.ctrl.units_left = amount;
                true
            }
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.ctrl.units_right = amount;
                true
            }
            VirtualKeyCode::Space => {
                self.ctrl.units_up = amount;
                true
            }
            VirtualKeyCode::LControl => {
                self.ctrl.units_down = amount;
                true
            }
            _ => false,
        }
    }

    fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.ctrl.horizontal_rotation = mouse_dx as f32;
        self.ctrl.vertical_rotation = mouse_dy as f32;
    }

    fn process_scroll(&mut self, delta: &winit::event::MouseScrollDelta) {
        self.ctrl.scroll = -match delta {
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => *scroll as f32,
        };
    }
}

pub struct Projection {
    aspect: f32,
    fovy: Rad<f32>,
    znear: f32,
    zfar: f32,
}

impl Projection {
    pub fn new<Fovy>(width: u32, height: u32, fovy: Fovy, znear: f32, zfar: f32) -> Self
    where
        Fovy: Into<Rad<f32>>,
    {
        Self {
            aspect: width as f32 / height as f32,
            fovy: fovy.into(),
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * perspective(self.fovy, self.aspect, self.znear, self.zfar)
    }
}
