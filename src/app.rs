use std::{sync::Arc, time::Instant};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::passes::{ClearPass, GeometryPass, GizmoPass, SkyboxPass, SsaoPass};
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

        let hdr = Hdr::new(&context.device, &context.config);

        let gbuffer = gbuffer::GBuffer::new(&context.device, &context.config);

        let visualizer = Visualizer::new(&context.device, context.config.format, &gbuffer);

        let viewer = ModelViewer::new();

        for instance in &viewer.instances {
            resources
                .load_model(
                    &instance.model_name,
                    &context.device,
                    &context.queue,
                    &renderer.texture_bind_group_layout,
                )
                .await?;
        }

        let mut settings = crate::settings::RenderSettings::new();
        renderer.add_pass(Box::new(ClearPass));
        settings.pass_states.insert("Clear".to_string(), true);
        renderer.add_pass(Box::new(GizmoPass::new(&context, &renderer, hdr.format())));
        settings.pass_states.insert("Gizmo".to_string(), true);
        renderer.add_pass(Box::new(GeometryPass::new(
            &context,
            &renderer,
            hdr.format(),
        )));
        settings.pass_states.insert("Geometry".to_string(), true);
        renderer.add_pass(Box::new(SsaoPass::new(&context, &renderer, &gbuffer)));
        settings.pass_states.insert("SSAO".to_string(), true);
        renderer.add_pass(Box::new(LightingPass::new(
            &context,
            &renderer,
            &gbuffer,
            hdr.format(),
        )));
        settings.pass_states.insert("Lighting".to_string(), true);

        renderer.add_pass(Box::new(TransparentPass::new(
            &context,
            &renderer,
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
        renderer.add_pass(Box::new(SkyboxPass::new(&context, &renderer, hdr.format())));
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
        self.renderer.resize(width, height);
        self.hdr_visualizer
            .resize(&self.context.device, width, height);
        self.gbuffer
            .resize(&self.context.device, &self.context.config);
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        self.viewer.update(dt, &mut self.input);
        self.renderer.update(&self.context, &self.viewer);
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

        self.hdr_visualizer.process(&mut encoder, &view);

        if self.settings.debug_mode > 0 {
            self.visualizer.process(
                &mut encoder,
                &view,
                &self.gbuffer,
                &self.context.queue,
                self.settings.debug_mode - 1,
            );
        } else {
            self.hdr_visualizer.process(&mut encoder, &view);
        }

        let models = &self.resources.models;
        self.ui.draw(&self.context, &mut encoder, &view, |ui| {
            self.app_ui
                .show(ui, &self.viewer, models, &mut self.settings);
        });

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
            .with_blur(true);
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
                    ))
                    .unwrap_or_else(|e| log::error!("Failed to load dropped model: {}", e));

                    state.viewer.instances.clear();
                    state.viewer.instances.push(crate::viewer::Instance {
                        model_name: path_str,
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
