use crate::context::GraphicsContext;
use crate::graphics::renderer::Renderer;

pub struct BlurPass {
    pipeline: wgpu::RenderPipeline,
}

impl BlurPass {
    pub fn new(context: &GraphicsContext, renderer: &Renderer) -> Self {
        let device = &context.device;

        // Pipeline Layout uses Renderer's ssao_target layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blur Pipeline Layout"),
            bind_group_layouts: &[Some(&renderer.single_texture_bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/blur.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blur Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }
}

impl crate::passes::RenderPass for BlurPass {
    fn name(&self) -> &str {
        "Blur"
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        _view: &wgpu::TextureView,
        _gbuffer: &crate::graphics::gbuffer::GBuffer,
        _viewer: &crate::graphics::viewer::ModelViewer,
        _resources: &crate::graphics::resources::ResourceManager,
        _context: &GraphicsContext,
        renderer: &Renderer,
        _settings: &crate::settings::RenderSettings,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blur Pass Render"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &renderer.blur_target.texture.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &renderer.ssao_target.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
