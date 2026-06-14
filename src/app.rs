use std::{sync::Arc, time::Instant};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::context::GraphicsContext;
use crate::depth::Depth;
use crate::hdr::Hdr;
use crate::input::Input;
use crate::passes::{ClearPass, DebugPass, OpaquePass, SkyboxPass};
use crate::renderer::Renderer;
use crate::resources;
use crate::scenes::Scene;
use crate::scenes::default_scene::DefaultScene;
use crate::texture;

pub struct EngineState {
    pub context: GraphicsContext,
    pub renderer: Renderer,
    pub scene: Box<dyn Scene>,
    pub resources: resources::ResourceManager,
    pub input: Input,
    pub hdr_visualizer: Hdr,
    pub depth_visualizer: Depth,
    pub depth_texture: texture::Texture,
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

        let depth_texture = texture::Texture::create_depth_texture(
            &context.device,
            &context.config,
            "depth_texture",
        );

        let depth_visualizer =
            crate::depth::Depth::new(&context.device, &context.config, &depth_texture);

        let scene = DefaultScene::new();

        for model_name in scene.required_models() {
            resources
                .load_model(
                    model_name,
                    &context.device,
                    &context.queue,
                    &renderer.texture_bind_group_layout,
                )
                .await?;
        }

        let mut settings = crate::settings::RenderSettings::new();
        renderer.add_pass(Box::new(ClearPass));
        settings.pass_states.insert("Clear".to_string(), true);
        renderer.add_pass(Box::new(DebugPass::new(&context, &renderer, hdr.format())));
        settings.pass_states.insert("Debug".to_string(), true);
        renderer.add_pass(Box::new(OpaquePass::new(&context, &renderer, hdr.format())));
        settings.pass_states.insert("Opaque".to_string(), true);

        resources
            .load_hdr_environment(
                scene.skybox_path(),
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
            scene: Box::new(scene),
            resources,
            input: Input::new(),
            hdr_visualizer: hdr,
            depth_visualizer: depth_visualizer,
            depth_texture,
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
        self.depth_texture = texture::Texture::create_depth_texture(
            &self.context.device,
            &self.context.config,
            "depth_texture",
        );
        self.depth_visualizer
            .resize(&self.context.device, &self.depth_texture);
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        self.scene.update(dt, &mut self.input);
        self.renderer.update(&self.context, self.scene.as_ref());
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
            &self.depth_texture.view,
            self.scene.as_ref(),
            &self.resources,
            &mut encoder,
            &mut self.settings,
        );

        self.hdr_visualizer.process(&mut encoder, &view);

        if self.settings.show_depthmap {
            self.depth_visualizer.process(&mut encoder, &view);
        } else {
            self.hdr_visualizer.process(&mut encoder, &view);
        }

        let models = &self.resources.models;
        self.ui.draw(&self.context, &mut encoder, &view, |ui| {
            self.app_ui
                .show(ui, self.scene.as_ref(), models, &mut self.settings);
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
