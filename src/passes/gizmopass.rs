use crate::context::GraphicsContext;
use crate::renderer::{Renderer, create_render_pipeline};
use crate::texture;
use crate::viewer::ModelViewer;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GizmoVertex {
    pub position: [f32; 3],
}
impl GizmoVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GizmoVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

pub struct GizmoPass {
    pub render_pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
}

impl GizmoPass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        _settings: &crate::settings::RenderSettings,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let layout = context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Gizmo Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&renderer.camera_bind_group_layout),
                    Some(&renderer.light_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let shader = wgpu::ShaderModuleDescriptor {
            label: Some("Gizmo Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gizmo.wgsl").into()),
        };
        let render_pipeline = create_render_pipeline(
            &context.device,
            &layout,
            hdr_format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[GizmoVertex::desc(), crate::renderer::InstanceRaw::desc()],
            wgpu::PrimitiveTopology::LineList,
            shader,
        );

        // The 8 corners of a cube
        let v = [
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ];

        // 12 lines * 2 vertices each
        let vertices = [
            // Bottom
            GizmoVertex { position: v[0] },
            GizmoVertex { position: v[1] },
            GizmoVertex { position: v[1] },
            GizmoVertex { position: v[2] },
            GizmoVertex { position: v[2] },
            GizmoVertex { position: v[3] },
            GizmoVertex { position: v[3] },
            GizmoVertex { position: v[0] },
            // Top
            GizmoVertex { position: v[4] },
            GizmoVertex { position: v[5] },
            GizmoVertex { position: v[5] },
            GizmoVertex { position: v[6] },
            GizmoVertex { position: v[6] },
            GizmoVertex { position: v[7] },
            GizmoVertex { position: v[7] },
            GizmoVertex { position: v[4] },
            // Pillars
            GizmoVertex { position: v[0] },
            GizmoVertex { position: v[4] },
            GizmoVertex { position: v[1] },
            GizmoVertex { position: v[5] },
            GizmoVertex { position: v[2] },
            GizmoVertex { position: v[6] },
            GizmoVertex { position: v[3] },
            GizmoVertex { position: v[7] },
        ];

        use wgpu::util::DeviceExt;
        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Gizmo Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        Self {
            render_pipeline,
            vertex_buffer,
        }
    }
}

impl crate::renderer::RenderPass for GizmoPass {
    fn name(&self) -> &str {
        "Gizmo"
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        gbuffer: &crate::gbuffer::GBuffer,
        viewer: &ModelViewer,
        _resources: &crate::resources::ResourceManager,
        context: &GraphicsContext,
        renderer: &Renderer,
        _settings: &crate::settings::RenderSettings,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Gizmo Render Pass"),
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

        render_pass.set_pipeline(&self.render_pipeline);

        let gizmo_nodes = viewer.gizmo_nodes();
        if gizmo_nodes.is_empty() {
            return;
        }

        let mut gizmo_instances = Vec::new();
        for node in gizmo_nodes {
            let transform = node.get_transform();
            gizmo_instances.push(crate::renderer::InstanceRaw {
                model: transform.into(),
                // Normal matrix isn't used by our gizmo shader, so we just provide identity
                normal: cgmath::Matrix3::from_scale(1.0).into(),
            });
        }

        use wgpu::util::DeviceExt;
        let instance_bytes = bytemuck::cast_slice(&gizmo_instances);
        let instance_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Gizmo Instance Buffer"),
                    contents: instance_bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &renderer.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &renderer.light_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

        // 24 vertices for the wireframe cube, and 1 instance per debug node!
        render_pass.draw(0..24, 0..gizmo_instances.len() as u32);
    }
}
