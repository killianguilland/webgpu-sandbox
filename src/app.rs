use std::{sync::Arc, time::Instant};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::context::GraphicsContext;
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
    pub input: Input,
    pub hdr: Hdr,
    pub depth_texture: texture::Texture,
}

impl EngineState {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let context = GraphicsContext::new(window).await?;

        let mut renderer = Renderer::new(&context);

        let hdr = Hdr::new(&context.device, &context.config);
        let depth_texture = texture::Texture::create_depth_texture(
            &context.device,
            &context.config,
            "depth_texture",
        );

        let scene = DefaultScene::new();

        for model_name in scene.required_models() {
            renderer.load_model(model_name, &context).await?;
        }

        renderer.add_pass(Box::new(ClearPass));
        renderer.add_pass(Box::new(DebugPass::new(&context, &renderer, hdr.format())));
        renderer.add_pass(Box::new(OpaquePass::new(&context, &renderer, hdr.format())));

        if let Some(sky_path) = scene.skybox_path() {
            let hdr_loader = resources::HdrLoader::new(&context.device);
            let sky_bytes = resources::load_binary(sky_path).await?;
            let sky_texture = hdr_loader.from_equirectangular_bytes(
                &context.device,
                &context.queue,
                &sky_bytes,
                1080,
                Some("Sky Texture"),
            )?;
            renderer.add_pass(Box::new(SkyboxPass::new(
                &context,
                &renderer,
                hdr.format(),
                &sky_texture,
            )));
        }

        Ok(Self {
            context,
            renderer,
            scene: Box::new(scene),
            input: Input::new(),
            hdr,
            depth_texture,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.context.resize(width, height);
        self.renderer.resize(width, height);
        self.hdr.resize(&self.context.device, width, height);
        self.depth_texture = texture::Texture::create_depth_texture(
            &self.context.device,
            &self.context.config,
            "depth_texture",
        );
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
            &self.hdr.view(),
            &self.depth_texture.view,
            self.scene.as_ref(),
            &mut encoder,
        );

        self.hdr.process(&mut encoder, &view);

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

        let window_attributes = Window::default_attributes();
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
