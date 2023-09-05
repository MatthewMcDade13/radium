use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
    time::Duration,
};

use wgpu::{Device, DynamicOffset, RenderPass};
use winit::event::{ElementState, MouseScrollDelta, VirtualKeyCode, WindowEvent};

use crate::gfx::model::Model;

use super::render::{
    light::draw_light_model_instanced, mesh::draw_model_instanced, DrawCtx, RenderWindow,
    RenderWindowMut,
};

// pub trait RadApp: FrameUpdate + DrawFrame + WindowEventHandler + ProcessInput {}
//
#[derive(Debug, Clone, Copy)]
pub enum InputEventStatus {
    Processing,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseState {
    Pressed,
    Idle,
    Moving,
}

pub trait WindowEventHandler {
    fn handle_window_events(&mut self, event: &WindowEvent) -> InputEventStatus {
        InputEventStatus::Done
    }
}

pub trait DrawFrame {
    fn draw_frame<'a>(&'a self, ctx: &DrawCtx<'a>) -> Result<(), wgpu::SurfaceError>;
}

pub trait FrameUpdate {
    fn frame_update(&mut self, dt: Duration);
}

pub trait AppSetup {
    fn setup(&mut self, window: RenderWindowMut) {}
}

pub trait ProcessInput {
    // TODO :: Refactor this to return enum instead of bool.
    fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> InputEventStatus {
        InputEventStatus::Done
    }

    fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {}
    fn process_scroll(&mut self, delta: &MouseScrollDelta) {}
}

impl From<InputEventStatus> for bool {
    fn from(value: InputEventStatus) -> Self {
        match value {
            InputEventStatus::Done => false,
            InputEventStatus::Processing => true,
        }
    }
}
