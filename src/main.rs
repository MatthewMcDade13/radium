use anyhow::{anyhow, Result};
use log::*;

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{self, ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use crate::app::App;

mod app;
mod vulkan;

#[derive(PartialEq, Debug)]
enum AppState {
    Running,
    Destroying,
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Radium Vulkan")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build(&event_loop)?;

    let mut app = App::create(&window)?;
    let mut app_state = AppState::Running;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared if app_state == AppState::Running => {
                unsafe { app.render(&window) }.unwrap()
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                app_state = AppState::Destroying;
                *control_flow = ControlFlow::Exit;
                unsafe {
                    app.destroy();
                }
            }
            _ => {}
        }
    });
}
