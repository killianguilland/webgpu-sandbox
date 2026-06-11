pub mod app;
mod camera;
pub mod context;
mod hdr;
pub mod input;
mod model;
pub mod passes;
pub mod renderer;
mod resources;
pub mod scenes;
mod texture;

use crate::app::App;

use winit::event_loop::EventLoop;

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::with_user_event().build()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
