use std::time::Duration;

use winit::event::{ElementState, MouseScrollDelta, VirtualKeyCode, WindowEvent};
const TEMP: u32 = 0;

pub trait WindowEventHandler {
    fn handle_window_events(&mut self, event: &WindowEvent) -> bool;
}

pub trait FrameUpdate: Clone {
    /// clones self, runs clone.frame_update(dt), returns mutated state without altering current
    /// state.
    fn frame_update_into(&self, dt: Duration) -> Self {
        let mut next = self.clone();
        next.frame_update(dt);
        next
    }

    fn frame_update(&mut self, dt: Duration);
}

pub trait ProcessInput {
    // TODO :: Refactor this to return enum instead of bool.
    fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        false
    }

    fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {}
    fn process_scroll(&mut self, delta: &MouseScrollDelta) {}
}
