use crate::context::GraphicsContext;
use crate::gbuffer::GBuffer;
use crate::renderer::{RenderPass, Renderer};
use crate::resources::ResourceManager;
use crate::texture::Texture;
use crate::viewer::ModelViewer;
use wgpu::util::DeviceExt;

pub const SSAO_NOISE_DATA: [[u8; 4]; 16] = [
    [100, 200, 0, 255],
    [210, 45, 0, 255],
    [55, 88, 0, 255],
    [199, 180, 0, 255],
    [22, 110, 0, 255],
    [230, 220, 0, 255],
    [80, 30, 0, 255],
    [150, 150, 0, 255],
    [188, 66, 0, 255],
    [44, 210, 0, 255],
    [240, 90, 0, 255],
    [10, 199, 0, 255],
    [110, 20, 0, 255],
    [175, 230, 0, 255],
    [33, 140, 0, 255],
    [205, 100, 0, 255],
];

// Put this outside your SsaoPass struct
fn generate_ssao_samples(kernel_size: u32) -> [[f32; 4]; 64] {
    let mut samples = [[0.0; 4]; 64];
    for i in 0..kernel_size {
        let phi = (i as f32) * 2.39996323;
        // Use the actual kernel size here!
        let cos_theta = 1.0 - (i as f32 + 0.5) / (kernel_size as f32);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

        // And here!
        let scale = (i as f32) / (kernel_size as f32);
        let distance = 0.1 + (1.0 - 0.1) * (scale * scale);

        samples[i as usize] = [
            phi.cos() * sin_theta * distance,
            phi.sin() * sin_theta * distance,
            cos_theta * distance,
            0.0,
        ];
    }
    samples
}

pub struct SsaoPass {
    pub pipeline: wgpu::RenderPipeline,
    pub ssao_bind_group: wgpu::BindGroup,
    pub noise_texture: Texture,
    pub samples_buffer: wgpu::Buffer,
}

impl SsaoPass {
    pub fn new(
        context: &GraphicsContext,
        renderer: &Renderer,
        settings: &crate::settings::RenderSettings,
        gbuffer: &GBuffer,
    ) -> Self {
        let device = &context.device;

        let noise_texture = Texture::from_raw_rgba(
            device,
            &context.queue,
            bytemuck::cast_slice(&SSAO_NOISE_DATA),
            4,
            4,
            Some("SSAO Noise Texture"),
        )
        .unwrap();

        let ssao_samples = generate_ssao_samples(settings.ssao_kernel_size);
        let samples_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSAO Samples Buffer"),
            contents: bytemuck::cast_slice(&ssao_samples),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 2. Create Noise Bind Group Layout
        let ssao_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SSAO Noise Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // 3. Create Noise Bind Group with a custom NON-FILTERING sampler
        let custom_noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSAO Noise Sampler (Nearest)"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let ssao_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Noise Bind Group"),
            layout: &ssao_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&noise_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    // Bind our new custom sampler instead of noise_texture.sampler!
                    resource: wgpu::BindingResource::Sampler(&custom_noise_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: samples_buffer.as_entire_binding(),
                },
            ],
        });

        // 4. (SSAO output texture is now in Renderer)

        // 5. Pipeline Layout (GBuffer + Camera + Noise)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Pipeline Layout"),
            bind_group_layouts: &[
                Some(&gbuffer.layout),
                Some(&renderer.camera_bind_group_layout),
                Some(&ssao_bind_group_layout),
                Some(&renderer.settings_bind_group_layout),
            ],
            immediate_size: 0,
        });

        // 6. Shader Module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSAO Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ssao.wgsl").into()),
        });

        // 7. Pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SSAO Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"), // Fullscreen triangle vertex shader
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

        Self {
            pipeline,
            ssao_bind_group,
            noise_texture,
            samples_buffer,
        }
    }
}

impl RenderPass for SsaoPass {
    fn name(&self) -> &str {
        "SSAO"
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        _view: &wgpu::TextureView, // We don't write to the screen yet! We write to ssao_texture
        gbuffer: &GBuffer,
        _viewer: &ModelViewer,
        _resources: &ResourceManager,
        context: &GraphicsContext,
        renderer: &Renderer,
        settings: &crate::settings::RenderSettings,
    ) {
        if settings.changed {
            println!("recreating ssao pipeline");
            let ssao_samples = generate_ssao_samples(settings.ssao_kernel_size);

            context.queue.write_buffer(
                &self.samples_buffer,
                0,
                bytemuck::cast_slice(&ssao_samples),
            );
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SSAO Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &renderer.ssao_target.texture.view, // Writing to Renderer's target
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), // 1.0 means no occlusion by default
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        // 1. Set the pipeline
        pass.set_pipeline(&self.pipeline);

        // 2. Bind the data
        pass.set_bind_group(0, &gbuffer.bind_group, &[]);
        pass.set_bind_group(1, &renderer.camera_bind_group, &[]);
        pass.set_bind_group(2, &self.ssao_bind_group, &[]);
        pass.set_bind_group(3, &renderer.settings_bind_group, &[]);

        // 3. Draw a fullscreen triangle!
        pass.draw(0..3, 0..1);
    }
}
