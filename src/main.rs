use actix::prelude::*;
use anyhow::Result;
use rad::run_loop;
const TEMP: u32 = 0;

mod eng;
mod gfx;
mod sys;

fn main() -> Result<()> {
    let sys = System::new();
    sys.block_on(run_loop())
}
