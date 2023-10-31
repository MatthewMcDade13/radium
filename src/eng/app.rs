use std::{cell::RefCell, future::Future, rc::Rc, time::Duration};

use winit::{
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::gfx::{self, draw::DrawCtx, window::RenderWindow};

use super::command::RenderPassOp;

pub trait RadApp {
    fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> InputEventStatus {
        InputEventStatus::Done
    }

    fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {}
    fn process_scroll(&mut self, delta: &MouseScrollDelta) {}
    fn draw_frame(&mut self, ctx: &mut DrawCtx) -> Result<(), wgpu::SurfaceError>;
    fn frame_update(&mut self, dt: Duration);

    fn handle_window_events(&mut self, event: &WindowEvent) -> InputEventStatus {
        InputEventStatus::Done
    }
}

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

impl From<InputEventStatus> for bool {
    fn from(value: InputEventStatus) -> Self {
        match value {
            InputEventStatus::Done => false,
            InputEventStatus::Processing => true,
        }
    }
}

pub struct Radium;
impl Radium {
    pub async fn start<A, F, Fut>(factory: F) -> anyhow::Result<()>
    where
        A: RadApp + 'static,
        F: Fn(RadWindow) -> Fut,
        Fut: Future<Output = anyhow::Result<A>>,
    {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop)?;
        let mut render_window = RadWindow::new(window).await?;

        let mut last_dt = std::time::Instant::now();

        let mut app = factory(render_window.clone()).await?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == render_window.id() => match app.handle_window_events(event) {
                    InputEventStatus::Processing => {}
                    InputEventStatus::Done => match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => {
                            *control_flow = ControlFlow::Exit;
                        }
                        WindowEvent::Resized(physical_size) => {
                            render_window.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            render_window.resize(**new_inner_size);
                        }
                        _ => {}
                    },
                },
                Event::RedrawRequested(window_id) if window_id == render_window.id() => {
                    let now = std::time::Instant::now();
                    let dt = now - last_dt;
                    last_dt = now;

                    // render_window.handle_mut().update_camera(dt);
                    app.frame_update(dt);

                    let mut ctx = render_window.create_draw_context();
                    ctx.begin_render_pass(RenderPassOp::CLEAR_BLACK);

                    app.draw_frame(&mut ctx)
                        .expect("Error occured while drawing frame");

                    if let Err(error) = ctx.submit() {
                        match error {
                            wgpu::SurfaceError::Lost => {
                                let size = render_window.size();
                                render_window.resize(size)
                            }
                            wgpu::SurfaceError::OutOfMemory => *control_flow = ControlFlow::Exit,
                            _ => eprintln!("{:?}", error),
                        };
                    }
                }
                Event::MainEventsCleared => {
                    render_window.request_redraw();
                }
                Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta },
                    ..
                } => match render_window.mouse_state() {
                    MouseState::Pressed => app.process_mouse(delta.0, delta.1),
                    _ => {}
                },
                _ => {}
            }
        });
    }
}
