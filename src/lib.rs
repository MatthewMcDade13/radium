mod eng;
mod gfx;
mod sys;

#[cfg(test)]
mod tests;

pub async fn run_loop() -> anyhow::Result<()> {
    env_logger::init();

    // Radium::start(|rw| Renderer::new(rw)).await?;
    Ok(())
}
