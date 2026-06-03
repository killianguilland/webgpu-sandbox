use std::sync::Arc;

use wgpu::{RenderPipeline};
use winit::{
    window::Window
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::EventLoopExtWebSys;

use winit::{
    event_loop::{ActiveEventLoop}, keyboard::{KeyCode},
};

use crate::state::Pipeline::{ColorTriangle, WhiteTriangle};

enum Pipeline {
    WhiteTriangle,
    ColorTriangle,
}

// This will store the state of our game
pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    window: Arc<Window>,
    clear_color: wgpu::Color,
    pipeline: Pipeline,
    white_render_pipeline: wgpu::RenderPipeline,
    color_render_pipeline: wgpu::RenderPipeline,
}

fn build_pipeline(device: &wgpu::Device, config: &wgpu::wgt::SurfaceConfiguration<Vec<wgpu::TextureFormat>>, shader: &wgpu::ShaderModule) -> RenderPipeline {
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList, 
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw, 
            cull_mode: Some(wgpu::Face::Back),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

impl State {
    // We don't need this to be async right now,
    // but we will in the next tutorial
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            //display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web we'll have to disable some.
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result in all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        // Shader setup

        let white_shader = device.create_shader_module(wgpu::include_wgsl!("white_shader.wgsl"));
        let color_shader = device.create_shader_module(wgpu::include_wgsl!("color_shader.wgsl"));

        let white_render_pipeline = build_pipeline(&device, &config, &white_shader);
        let color_render_pipeline = build_pipeline(&device, &config, &color_shader);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            clear_color: wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            white_render_pipeline,
            color_render_pipeline,
            pipeline: Pipeline::WhiteTriangle
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
        }
    }

    pub fn handle_mouse_moved(&mut self, x: f64, y: f64) {
        // x and y are in physical pixels; convert to normalized [0.0, 1.0]
        let size = self.window.inner_size();
        let w = size.width.max(1) as f64;
        let h = size.height.max(1) as f64;
        let nx = x / w;
        let ny = y / h;
        self.clear_color.b = nx;
        self.clear_color.g = ny;
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            (KeyCode::Space, true) => match self.pipeline {
                WhiteTriangle => self.pipeline = Pipeline::ColorTriangle,
                ColorTriangle => self.pipeline = Pipeline::WhiteTriangle
            }
            _ => {}
        }
    }

    pub fn update(&mut self) {
        // remove `todo!()`
    }
    
    pub fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        // We can't render unless the surface is configured
        if !self.is_surface_configured {
            return Ok(());
        }
            
        let output = match self.surface.get_current_texture() {
            // SUCCESS! We got the SurfaceTexture.
            Ok(texture) => texture,
                
            Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            
            Err(wgpu::SurfaceError::OutOfMemory) => {
                // The system ran out of memory. This is fatal, so we log it.
                log::error!("OutOfMemory");
                // You might want to return an error here depending on your app's structure
                return Ok(()); 
            }
            
            Err(wgpu::SurfaceError::Timeout) => {
                // A timeout happened. We just log a warning and skip the frame.
                log::warn!("Surface timeout");
                return Ok(());
            }

            Err(wgpu::SurfaceError::Lost) => {
                // You could recreate the devices and all resources
                // created with it here, but we'll just bail
                anyhow::bail!("Lost device");
            }

            Err(wgpu::SurfaceError::Other) => {
                // This case didnt exist in the tutorial, I chose to bail for now
                anyhow::bail!("Other surface error");
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            match &self.pipeline {
                WhiteTriangle => render_pass.set_pipeline(&self.white_render_pipeline),
                ColorTriangle => render_pass.set_pipeline(&self.color_render_pipeline),
            }
            
            render_pass.draw(0..3, 0..1);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())

    }
}

// ...
