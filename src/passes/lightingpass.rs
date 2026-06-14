use crate::context::GraphicsContext;
use crate::gbuffer::GBuffer;
use crate::renderer::Renderer;

pub struct LightingPass {
    pipeline: wgpu::RenderPipeline,
}

impl LightingPass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        gbuffer: &GBuffer,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Lighting Pipeline Layout"),
                    bind_group_layouts: &[
                        Some(&gbuffer.layout),
                        Some(&renderer.camera_bind_group_layout),
                        Some(&renderer.light_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Lighting Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/lighting.wgsl").into()),
            });

        let pipeline = context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Lighting Pipeline"),
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
                        format: hdr_format,
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

impl crate::renderer::RenderPass for LightingPass {
    fn name(&self) -> &str {
        "Lighting"
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        gbuffer: &crate::gbuffer::GBuffer,
        _scene: &dyn crate::scenes::Scene,
        _resources: &crate::resources::ResourceManager,
        _context: &GraphicsContext,
        renderer: &crate::renderer::Renderer,
    ) {
        // Update the uniform

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Lighting Pass Render"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
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
        pass.set_bind_group(0, &gbuffer.bind_group, &[]);
        pass.set_bind_group(1, &renderer.camera_bind_group, &[]);
        pass.set_bind_group(2, &renderer.light_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
