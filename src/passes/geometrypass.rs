use crate::context::GraphicsContext;
use crate::graphics::model;
use crate::graphics::model::Vertex;
use crate::graphics::renderer::Renderer;
use crate::graphics::viewer::ModelViewer;

pub struct GeometryPass {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl GeometryPass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        _settings: &crate::settings::RenderSettings,
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
                        Some(&renderer.environment_layout),
                        Some(&renderer.hierarchy_layout),
                    ],
                    immediate_size: 0,
                });

        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Opaque Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/geometry.wgsl").into()),
            });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Geometry Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[
                            model::ModelVertex::desc(),
                            crate::graphics::renderer::InstanceRaw::desc(),
                        ],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: crate::graphics::gbuffer::GBuffer::ALBEDO_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: crate::graphics::gbuffer::GBuffer::NORMAL_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: crate::graphics::gbuffer::GBuffer::PBR_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: crate::graphics::gbuffer::GBuffer::VELOCITY_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: hdr_format,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::graphics::gbuffer::GBuffer::DEPTH_FORMAT,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::Less),
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });

        Self { render_pipeline }
    }
}

impl crate::passes::RenderPass for GeometryPass {
    fn name(&self) -> &str {
        "Geometry"
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
            label: Some("Geometry Render Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.albedo.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.normal.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.pbr.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.velocity.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
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

        if let Some(env_bg) = resources.get_bind_group(&viewer.skybox_path) {
            render_pass.set_bind_group(3, env_bg, &[]);
        }

        use crate::graphics::model::DrawModel;
        for (model_name, (instance_buffer, count)) in &renderer.instance_buffers {
            if let Some(model) = resources.get_model(model_name) {
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                render_pass.draw_model_instanced(
                    model,
                    0..*count,
                    &renderer.camera_bind_group,
                    &renderer.light_bind_group,
                    crate::graphics::model::MeshFilter::OpaqueOnly,
                );
            }
        }
    }
}
