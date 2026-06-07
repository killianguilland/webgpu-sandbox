pub mod app;
mod state;
mod texture;
mod model;
mod resources;
mod camera;

use crate::app::App;

use winit::{
    event_loop::{ EventLoop }
};


pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::with_user_event().build()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}