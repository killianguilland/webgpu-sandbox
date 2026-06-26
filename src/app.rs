use std::{sync::Arc, time::Instant};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::passes::{BlurPass, ClearPass, GeometryPass, GizmoPass, SkyboxPass, SsaoPass};
use crate::postprocess::hdr::Hdr;
use crate::postprocess::visualizer::Visualizer;
use crate::renderer::Renderer;
use crate::resources;
use crate::viewer::ModelViewer;
use crate::{context::GraphicsContext, gbuffer::GBuffer};
use crate::{gbuffer, passes::lightingpass::LightingPass};
use crate::{input::Input, passes::TransparentPass};

pub struct EngineState {
    pub context: GraphicsContext,
    pub renderer: Renderer,
    pub viewer: ModelViewer,
    pub resources: resources::ResourceManager,
    pub input: Input,
    pub hdr_visualizer: Hdr,
    pub visualizer: Visualizer,
    pub gbuffer: GBuffer,
    pub ui: crate::ui::UiState,
    pub app_ui: crate::ui::AppUi,
    pub settings: crate::settings::RenderSettings,
}

impl EngineState {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let context = GraphicsContext::new(window).await?;

        let mut renderer = Renderer::new(&context);
        let mut resources = resources::ResourceManager::new(&context.device);

        let ui = crate::ui::UiState::new(&context);
        let app_ui = crate::ui::AppUi::new();

        let hdr = Hdr::new(
            &context.device,
            &context.config,
            &renderer.settings_bind_group_layout,
        );

        let gbuffer = gbuffer::GBuffer::new(&context.device, &context.config);

        let viewer = ModelViewer::new();

        resources
            .load_model(
                &viewer.model_name,
                &context.device,
                &context.queue,
                &renderer.texture_bind_group_layout,
                &renderer.hierarchy_layout,
            )
            .await?;

        let mut settings = crate::settings::RenderSettings::new();
        renderer.add_pass(Box::new(ClearPass));
        settings.pass_states.insert("Clear".to_string(), true);
        renderer.add_pass(Box::new(GizmoPass::new(
            &context,
            &renderer,
            &settings,
            hdr.format(),
        )));
        settings.pass_states.insert("Gizmo".to_string(), true);
        renderer.add_pass(Box::new(GeometryPass::new(
            &context,
            &renderer,
            &settings,
            hdr.format(),
        )));
        settings.pass_states.insert("Geometry".to_string(), true);
        let ssao_pass = SsaoPass::new(&context, &renderer, &settings, &gbuffer);
        let blur_pass = BlurPass::new(&context, &renderer);
        let visualizer =
            Visualizer::new(&context.device, context.config.format, &gbuffer, &renderer);
        let lighting_pass =
            LightingPass::new(&context, &renderer, &settings, &gbuffer, hdr.format());
        // Now that everyone who needs to look at the texture is done,
        // we can finally hand ownership of the passes over to the renderer!
        renderer.add_pass(Box::new(ssao_pass));
        settings.pass_states.insert("SSAO".to_string(), true);
        renderer.add_pass(Box::new(blur_pass));
        settings.pass_states.insert("Blur".to_string(), true);
        renderer.add_pass(Box::new(lighting_pass));
        settings.pass_states.insert("Lighting".to_string(), true);

        renderer.add_pass(Box::new(TransparentPass::new(
            &context,
            &renderer,
            &settings,
            hdr.format(),
        )));
        settings.pass_states.insert("Transparent".to_string(), true);

        resources
            .load_hdr_environment(
                &viewer.skybox_path,
                &context.device,
                &context.queue,
                &renderer.environment_layout,
                1080,
                Some("Sky Texture"),
            )
            .await?;
        renderer.add_pass(Box::new(SkyboxPass::new(
            &context,
            &renderer,
            &settings,
            hdr.format(),
        )));
        settings.pass_states.insert("Skybox".to_string(), true);

        Ok(Self {
            context,
            renderer,
            viewer,
            resources,
            input: Input::new(),
            hdr_visualizer: hdr,
            visualizer,
            gbuffer,
            ui,
            app_ui,
            settings,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.context.resize(width, height);

        self.settings.width = (width as f32 * self.settings.resolution_scale).max(1.0) as u32;
        self.settings.height = (height as f32 * self.settings.resolution_scale).max(1.0) as u32;

        let mut scaled_config = self.context.config.clone();
        scaled_config.width = self.settings.width;
        scaled_config.height = self.settings.height;

        self.renderer.resize(&self.context.device, &scaled_config);
        self.hdr_visualizer.resize(
            &self.context.device,
            self.settings.width,
            self.settings.height,
        );
        self.gbuffer.resize(&self.context.device, &scaled_config);
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        if self.settings.changed {
            let size = self.context.window.inner_size();
            self.resize(size.width, size.height);
        }

        if self.settings.light_follows_camera {
            self.settings.light_position = [
                self.viewer.camera.position.x,
                self.viewer.camera.position.y,
                self.viewer.camera.position.z,
            ];
        }

        self.viewer.update(dt, &mut self.input, &self.settings);
        self.renderer
            .update(&self.context, &self.viewer, &self.settings);
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        self.context.window.request_redraw();

        let output = match self.context.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.context
                    .surface
                    .configure(&self.context.device, &self.context.config);
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.context
                    .surface
                    .configure(&self.context.device, &self.context.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.renderer.render(
            &self.context,
            &self.hdr_visualizer.view(),
            &self.gbuffer,
            &self.viewer,
            &self.resources,
            &mut encoder,
            &mut self.settings,
        );

        self.hdr_visualizer
            .process(&mut encoder, &view, &self.renderer.settings_bind_group);

        if self.settings.debug_mode > 0 {
            self.visualizer.process(
                &mut encoder,
                &view,
                &self.gbuffer,
                &self.context.queue,
                &self.renderer,
                self.settings.debug_mode - 1,
            );
        } else {
            self.hdr_visualizer
                .process(&mut encoder, &view, &self.renderer.settings_bind_group);
        }

        let models = &self.resources.models;
        let mut requested_model = None;

        self.settings.changed = false;
        self.ui.draw(&self.context, &mut encoder, &view, |ui| {
            requested_model = self
                .app_ui
                .show(ui, &self.viewer, models, &mut self.settings);
        });

        if let Some(path_str) = requested_model {
            log::info!("UI requested model: {}", path_str);
            pollster::block_on(self.resources.load_model(
                &path_str,
                &self.context.device,
                &self.context.queue,
                &self.renderer.texture_bind_group_layout,
                &self.renderer.hierarchy_layout,
            ))
            .unwrap_or_else(|e| log::error!("Failed to load UI requested model: {}", e));

            self.viewer.model_name = path_str;
            self.viewer.instances.clear();
            self.viewer.instances.push(crate::viewer::Instance {
                position: cgmath::Vector3::new(0.0, 0.0, 0.0),
                rotation: cgmath::Quaternion::new(1.0, 0.0, 0.0, 0.0),
            });
        }

        self.context.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub struct App {
    state: Option<EngineState>,
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

impl ApplicationHandler<EngineState> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_transparent(true)
            .with_blur(true)
            .with_title("Model viewer");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = Some(pollster::block_on(EngineState::new(window)).unwrap());
        self.last_render_time = Instant::now();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: EngineState) {
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
                    state.viewer.instances.push(crate::viewer::Instance {
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
