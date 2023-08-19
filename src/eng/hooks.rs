use winit::event::WindowEvent;

pub trait WindowEventHandler {
    fn handle_window_events(&mut self, event: &WindowEvent) -> bool;
}

pub trait FrameUpdate: Clone {
    /// clones self, runs clone.frame_update(dt), returns mutated state without altering current
    /// state.
    fn frame_update_into(&self, dt: f32) -> Self {
        let mut next = self.clone();
        next.frame_update(dt);
        next
    }

    fn frame_update(&mut self, dt: f32);
}
