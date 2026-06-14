use crate::renderer::create_render_pipeline;
use crate::texture;
use wgpu::Operations;

pub struct Depth {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Depth {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        depth_texture: &texture::Texture,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth::layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
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
            ],
        });

        let bind_group = Self::create_bind_group(device, &layout, depth_texture, &sampler);

        let shader = wgpu::include_wgsl!("shaders/depth.wgsl");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = create_render_pipeline(
            device,
            &pipeline_layout,
            config.format.add_srgb_suffix(),
            None,
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            shader,
        );

        Self {
            pipeline,
            bind_group,
            layout,
            sampler,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        depth_texture: &texture::Texture,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Depth::bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, depth_texture: &texture::Texture) {
        self.bind_group =
            Self::create_bind_group(device, &self.layout, depth_texture, &self.sampler);
    }

    pub fn process(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Depth::process"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                ops: Operations {
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
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
