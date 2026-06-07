use std::{sync::Arc, time::Instant};

use crate::state::State;

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub struct App {
    state: Option<State>,
    last_frame: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            last_frame: Instant::now(),
        }
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes();

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        // If we are not on web we can use pollster to
        // await the window creation
        self.state = Some(pollster::block_on(State::new(window)).unwrap());
        self.last_frame = Instant::now();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: State) {
        self.state = Some(event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };
        if window_id != state.window().id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                state.resize(physical_size.width, physical_size.height);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now - self.last_frame;
                self.last_frame = now;

                state.update(dt);
                if let Err(error) = state.render() {
                    log::error!("{error:?}");
                    event_loop.exit();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = state.window().inner_size();
                state.resize(size.width, size.height);
            }
            event => {
                let _ = state.input(&event);
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        if let DeviceEvent::MouseMotion { delta } = event {
            state.handle_mouse_motion(delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window().request_redraw();
        }
    }
}
