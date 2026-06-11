use crate::context::GraphicsContext;
use crate::model::Vertex;
use crate::renderer::{RenderPass, Renderer, create_render_pipeline};
use crate::scenes::Scene;
use crate::{model, texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugVertex {
    pub position: [f32; 3],
}
impl DebugVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DebugVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

pub struct DebugPass {
    pub render_pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
}

impl DebugPass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let layout = context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Debug Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&renderer.camera_bind_group_layout),
                    Some(&renderer.light_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let shader = wgpu::ShaderModuleDescriptor {
            label: Some("Debug Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/debug.wgsl").into()),
        };
        let render_pipeline = create_render_pipeline(
            &context.device,
            &layout,
            hdr_format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[DebugVertex::desc(), crate::renderer::InstanceRaw::desc()],
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
            DebugVertex { position: v[0] },
            DebugVertex { position: v[1] },
            DebugVertex { position: v[1] },
            DebugVertex { position: v[2] },
            DebugVertex { position: v[2] },
            DebugVertex { position: v[3] },
            DebugVertex { position: v[3] },
            DebugVertex { position: v[0] },
            // Top
            DebugVertex { position: v[4] },
            DebugVertex { position: v[5] },
            DebugVertex { position: v[5] },
            DebugVertex { position: v[6] },
            DebugVertex { position: v[6] },
            DebugVertex { position: v[7] },
            DebugVertex { position: v[7] },
            DebugVertex { position: v[4] },
            // Pillars
            DebugVertex { position: v[0] },
            DebugVertex { position: v[4] },
            DebugVertex { position: v[1] },
            DebugVertex { position: v[5] },
            DebugVertex { position: v[2] },
            DebugVertex { position: v[6] },
            DebugVertex { position: v[3] },
            DebugVertex { position: v[7] },
        ];

        use wgpu::util::DeviceExt;
        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Debug Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        Self {
            render_pipeline,
            vertex_buffer,
        }
    }
}

impl crate::renderer::RenderPass for DebugPass {
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene: &dyn Scene,
        context: &GraphicsContext,
        renderer: &Renderer,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Debug Render Pass"),
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

        render_pass.set_pipeline(&self.render_pipeline);

        let debug_nodes = scene.debug_nodes();
        if debug_nodes.is_empty() {
            return;
        }

        // 1. Collect transforms into InstanceRaw
        let mut debug_instances = Vec::new();
        for node in debug_nodes {
            let transform = node.get_transform();
            debug_instances.push(crate::renderer::InstanceRaw {
                model: transform.into(),
                // Normal matrix isn't used by our debug shader, so we just provide identity
                normal: cgmath::Matrix3::from_scale(1.0).into(),
            });
        }

        // 2. Create instance buffer
        use wgpu::util::DeviceExt;
        let instance_bytes = bytemuck::cast_slice(&debug_instances);
        let instance_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Debug Instance Buffer"),
                    contents: instance_bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                });

        // 3. Draw!
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &renderer.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &renderer.light_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

        // 24 vertices for the wireframe cube, and 1 instance per debug node!
        render_pass.draw(0..24, 0..debug_instances.len() as u32);
    }
}
