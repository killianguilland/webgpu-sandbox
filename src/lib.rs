pub mod app;
pub mod context;
pub mod core;
pub mod graphics;
pub mod input;
pub mod passes;
pub mod postprocess;
pub mod settings;
mod ui;

use crate::app::App;

use winit::event_loop::EventLoop;

/*
 * Initializes logging, creates the Winit event loop, instantiates the App,
 * and kicks off the main execution loop.
 * (Note: this is the application entry point when running on the web, but this project is not meant to be run on the web)
 */
pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::with_user_event().build()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
