use crate::graphics::gbuffer::GBuffer;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SettingsUniform {
    mode: u32,
    _padding: [u32; 3], // padding to 16 bytes (required by WGSL structs)
}

pub struct Visualizer {
    pipeline: wgpu::RenderPipeline,
    settings_buffer: wgpu::Buffer,
    settings_bind_group: wgpu::BindGroup,
}

impl Visualizer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        gbuffer: &GBuffer,
        renderer: &crate::graphics::renderer::Renderer,
    ) -> Self {
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Visualizer Settings Buffer"),
            contents: bytemuck::cast_slice(&[SettingsUniform {
                mode: 0,
                _padding: [0; 3],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let settings_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Visualizer Settings Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let settings_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Visualizer Settings Bind Group"),
            layout: &settings_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: settings_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visualizer Pipeline Layout"),
            bind_group_layouts: &[
                Some(&gbuffer.layout),
                Some(&settings_bind_group_layout),
                Some(&renderer.single_texture_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visualizer Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/visualizer.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visualizer Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            settings_buffer,
            settings_bind_group,
        }
    }

    pub fn process(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        gbuffer: &GBuffer,
        queue: &wgpu::Queue,
        renderer: &crate::graphics::renderer::Renderer,
        mode: u32,
    ) {
        // Update the uniform
        queue.write_buffer(
            &self.settings_buffer,
            0,
            bytemuck::cast_slice(&[SettingsUniform {
                mode,
                _padding: [0; 3],
            }]),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Visualizer::process"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
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
        pass.set_bind_group(1, &self.settings_bind_group, &[]);
        pass.set_bind_group(2, &renderer.blur_target.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
