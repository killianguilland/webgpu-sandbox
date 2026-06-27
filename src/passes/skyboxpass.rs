use crate::context::GraphicsContext;
use crate::renderer::{RenderPass, Renderer, create_render_pipeline};
use crate::graphics::texture;
use crate::graphics::viewer::ModelViewer;

pub struct SkyboxPass {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl SkyboxPass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        _settings: &crate::settings::RenderSettings,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let layout = context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&renderer.camera_bind_group_layout),
                    Some(&renderer.environment_layout),
                ],
                immediate_size: 0,
            });
        let shader = wgpu::include_wgsl!("../shaders/skybox.wgsl");
        let render_pipeline = create_render_pipeline(
            &context.device,
            &layout,
            hdr_format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            shader,
        );

        Self { render_pipeline }
    }
}

impl RenderPass for SkyboxPass {
    fn name(&self) -> &str {
        "Skybox"
    }
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        gbuffer: &crate::graphics::gbuffer::GBuffer,
        viewer: &ModelViewer,
        resources: &crate::graphics::resources::ResourceManager,
        _context: &GraphicsContext,
        renderer: &Renderer,
        _settings: &crate::settings::RenderSettings,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skybox Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &gbuffer.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        // Fetch the environment bind group by the scene's skybox path
        let env_bg = resources
            .get_bind_group(&viewer.skybox_path)
            .expect("Skybox pass requires an environment map to be loaded");

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &renderer.camera_bind_group, &[]);
        render_pass.set_bind_group(1, env_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
