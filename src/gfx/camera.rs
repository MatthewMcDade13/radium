use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

use crate::{
    eng::hooks::{FrameUpdate, WindowEventHandler},
    sys::math::OPENGL_TO_WGPU_MATRIX,
};

#[derive(Debug, Copy, Clone)]
pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn build_view_proj_matrix(&self) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        OPENGL_TO_WGPU_MATRIX * proj * view
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }
}

impl CameraUniform {
    pub fn new(mat4: &[[f32; 4]; 4]) -> Self {
        Self {
            view_proj: mat4.clone(),
        }
    }

    pub fn from_camera(cam: &Camera) -> Self {
        Self {
            view_proj: cam.build_view_proj_matrix().into(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CameraControl {
    speed: f32,
    forward_pressed: bool,
    backward_pressed: bool,
    left_pressed: bool,
    right_pressed: bool,
}

impl CameraControl {
    pub const fn new(speed: f32) -> Self {
        Self {
            speed,
            forward_pressed: false,
            backward_pressed: false,
            left_pressed: false,
            right_pressed: false,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PlayerCamera {
    cam: Camera,
    uniform: CameraUniform,
    ctrl: CameraControl,
}

impl PlayerCamera {}

impl WindowEventHandler for PlayerCamera {
    fn handle_window_events(&mut self, event: &winit::event::WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        self.ctrl.forward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        self.ctrl.left_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        self.ctrl.backward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        self.ctrl.right_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl FrameUpdate for PlayerCamera {
    fn frame_update(&mut self, dt: f32) {
        use cgmath::InnerSpace;
        let forward = self.cam.target - self.cam.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.magnitude();

        // prevents glitching when camera gets too close tot he center of the scene
        if self.ctrl.forward_pressed && forward_mag > self.ctrl.speed {
            self.cam.eye += forward_norm * self.ctrl.speed;
        }
        if self.ctrl.backward_pressed {
            self.cam.eye -= forward_norm * self.ctrl.speed;
        }

        let right = forward_norm.cross(self.cam.up);

        // redo radius calc in case forward/backward is pressed
        let forward = self.cam.target - self.cam.eye;
        let forward_mag = forward.magnitude();

        if self.ctrl.right_pressed {
            // Rescale the distance between the target and eye so
            // that it doesn't change. The eye therefore still
            // lies on the circle made by the target and eye.
            self.cam.eye =
                self.cam.target - (forward + right * self.ctrl.speed).normalize() * forward_mag;
        }

        if self.ctrl.left_pressed {
            self.cam.eye =
                self.cam.target - (forward - right * self.ctrl.speed).normalize() * forward_mag;
        }
    }
}
