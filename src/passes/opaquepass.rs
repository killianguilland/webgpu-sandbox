use crate::context::GraphicsContext;
use crate::model::Vertex;
use crate::renderer::{InstanceRaw, RenderPass, Renderer, create_render_pipeline};
use crate::scenes::Scene;
use crate::{model, texture};

pub struct OpaquePass {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl OpaquePass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Model Pipeline Layout"),
                    bind_group_layouts: &[
                        Some(&renderer.texture_bind_group_layout),
                        Some(&renderer.camera_bind_group_layout),
                        Some(&renderer.light_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let shader = wgpu::ShaderModuleDescriptor {
            label: Some("Opaque Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/opaque.wgsl").into()),
        };

        let render_pipeline = create_render_pipeline(
            &context.device,
            &render_pipeline_layout,
            hdr_format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[model::ModelVertex::desc(), InstanceRaw::desc()],
            wgpu::PrimitiveTopology::TriangleList,
            shader,
        );

        Self { render_pipeline }
    }
}

impl RenderPass for OpaquePass {
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene: &dyn Scene,
        _context: &GraphicsContext,
        renderer: &Renderer,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Opaque Render Pass"),
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
                view: depth_view,
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

        // render_pass.set_vertex_buffer(1, renderer.instance_buffer.slice(..));
        render_pass.set_pipeline(&self.render_pipeline);

        use crate::model::DrawModel;
        for (model_name, model) in &renderer.models {
            // If this model has active instances in the scene, draw it!
            if let Some((instance_buffer, count)) = renderer.instance_buffers.get(model_name) {
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                render_pass.draw_model_instanced(
                    model,
                    0..*count,
                    &renderer.camera_bind_group,
                    &renderer.light_bind_group,
                );
            }
        }
    }
}
