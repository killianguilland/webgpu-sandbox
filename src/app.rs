use std::{sync::Arc, time::Instant};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::core::Core;

/*
 * The App struct acts as the platform layer.
 * Responsible for initializing the OS window, handling the Winit event loop,
 * catching raw OS events, and managing the application lifecycle (suspend/resume/close).
 * It acts as the bridge between the Operating System and the Core engine.
 */
pub struct App {
    state: Option<Core>,
    last_render_time: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            last_render_time: Instant::now(),
        }
    }
}

impl ApplicationHandler<Core> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_transparent(true)
            .with_blur(true)
            .with_title("Model viewer");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = Some(pollster::block_on(Core::new(window)).unwrap());
        self.last_render_time = Instant::now();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: Core) {
        self.state = Some(event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(state) => state,
            None => return,
        };
        if window_id != state.context.window.id() {
            return;
        }

        let response = state
            .ui
            .winit_state
            .on_window_event(&state.context.window, &event);

        if response.consumed {
            return;
        }

        let egui_ctx = state.ui.winit_state.egui_ctx();

        if egui_ctx.egui_wants_pointer_input()
            && matches!(
                event,
                WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. }
            )
        {
            return;
        }

        if egui_ctx.egui_wants_keyboard_input()
            && matches!(event, WindowEvent::KeyboardInput { .. })
        {
            return;
        }

        if state.input.process_event(&event) {
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
                let dt = now - self.last_render_time;
                self.last_render_time = now;

                state.update(dt);
                if let Err(error) = state.render() {
                    log::error!("{error:?}");
                    event_loop.exit();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = state.context.window.inner_size();
                state.resize(size.width, size.height);
            }
            WindowEvent::DroppedFile(path) => {
                log::info!("Dropped file: {:?}", path);
                let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                let path_str = path.to_string_lossy().to_string();

                if extension == "obj" || extension == "gltf" || extension == "glb" {
                    // Load the dropped model
                    pollster::block_on(state.resources.load_model(
                        &path_str,
                        &state.context.device,
                        &state.context.queue,
                        &state.renderer.texture_bind_group_layout,
                        &state.renderer.hierarchy_layout,
                    ))
                    .unwrap_or_else(|e| log::error!("Failed to load dropped model: {}", e));

                    state.viewer.model_name = path_str;
                    state.viewer.instances.clear();
                    state
                        .viewer
                        .instances
                        .push(crate::graphics::viewer::Instance {
                            position: cgmath::Vector3::new(0.0, 0.0, 0.0),
                            rotation: cgmath::Quaternion::new(1.0, 0.0, 0.0, 0.0),
                        });
                } else if extension == "hdr" {
                    // Load the dropped skybox environment map
                    pollster::block_on(state.resources.load_hdr_environment(
                        &path_str,
                        &state.context.device,
                        &state.context.queue,
                        &state.renderer.environment_layout,
                        1080,
                        Some("Sky Texture"),
                    ))
                    .unwrap_or_else(|e| log::error!("Failed to load dropped HDR map: {}", e));

                    state.viewer.skybox_path = path_str;
                }
            }
            _ => {}
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
            state.input.process_mouse_motion(delta);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.context.window.request_redraw();
        }
    }
}
