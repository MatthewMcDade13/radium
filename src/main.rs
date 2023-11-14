use std::rc::Rc;

use anyhow::Result;
use eng::app::{RadApp, Radium};
use gfx::{draw::DrawCtx, geom::QuadBuffer, window::RenderWindow};
use winit::event::{ElementState, VirtualKeyCode};

mod eng;
mod gfx;
mod sys;

fn main() -> Result<()> {
    smol::block_on(run_loop())
}

pub async fn run_loop() -> anyhow::Result<()> {
    env_logger::init();

    let g = Game::new();
    Radium::start(g).await?;
    // Radium::start(g)g.await?;
    Ok(())
}

struct Game {
    quads: QuadBuffer,
}

impl Game {
    pub fn new() -> Self {
        Self {
            quads: QuadBuffer::empty(),
        }
    }
}

impl RadApp for Game {
    fn draw_frame(&mut self, ctx: &mut DrawCtx) -> std::result::Result<(), wgpu::SurfaceError> {
        Ok(())
    }

    fn frame_update(&mut self, dt: std::time::Duration) {}

    fn process_keyboard(
        &mut self,
        key: winit::event::VirtualKeyCode,
        state: winit::event::ElementState,
    ) {
        // if let ElementState::Pressed = state {
        //     match key {
        //         VirtualKeyCode::Escape => {
        //
        //         }
        //         _ => {}
        //     }
        // }
    }
}
