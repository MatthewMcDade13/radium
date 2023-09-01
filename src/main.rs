use actix::prelude::*;
use anyhow::Result;
use rad::run_loop;

mod eng;
mod gfx;
mod sys;

fn main() -> Result<()> {
    let sys = System::new();
    sys.block_on(run_loop())
}
