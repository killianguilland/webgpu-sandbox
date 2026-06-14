use crate::context::GraphicsContext;
use crate::renderer::{RenderPass, Renderer};
use crate::scenes::Scene;

pub struct ClearPass;

impl RenderPass for ClearPass {
    fn name(&self) -> &str {
        "Clear"
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        _scene: &dyn Scene,
        _resources: &crate::resources::ResourceManager,
        _context: &GraphicsContext,
        _renderer: &Renderer,
    ) {
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Clear Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.2,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
}
