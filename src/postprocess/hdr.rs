use wgpu::Operations;

use crate::graphics::renderer::create_render_pipeline;
use crate::graphics::texture;

/// Owns the render texture and controls tonemapping
pub struct Hdr {
    pipeline: wgpu::RenderPipeline,
    // bind_group: wgpu::BindGroup,
    // texture: texture::Texture,

    // Arrays to hold our ping-pong state
    textures: [texture::Texture; 2],
    bind_groups: [wgpu::BindGroup; 2],
    frame_index: usize,

    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl Hdr {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        settings_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let width = config.width;
        let height = config.height;

        let format = wgpu::TextureFormat::Rgba16Float;

        let textures: [texture::Texture; 2] = std::array::from_fn(|_| {
            texture::Texture::create_2d_texture(
                device,
                width,
                height,
                format,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                wgpu::FilterMode::Nearest,
                Some("Hdr::texture"),
            )
        });

        let bind_groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Hdr::bind_group"),
                layout: texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&textures[i].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&textures[i].sampler),
                    },
                ],
            })
        });

        let shader = wgpu::include_wgsl!("../shaders/hdr.wgsl");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(texture_layout), Some(settings_layout)],
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
            bind_groups,
            textures,
            frame_index: 0,
            width,
            height,
            format,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, texture_layout: &wgpu::BindGroupLayout) {
        self.textures = std::array::from_fn(|_| {
            texture::Texture::create_2d_texture(
                device,
                width,
                height,
                self.format,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                wgpu::FilterMode::Nearest,
                Some("Hdr::texture"),
            )
        });

        self.bind_groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Hdr::bind_group"),
                layout: texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.textures[i].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.textures[i].sampler),
                    },
                ],
            })
        });
        self.width = width;
        self.height = height;
    }

    pub fn current_view(&self) -> &wgpu::TextureView {
        &self.textures[self.frame_index % 2].view
    }

    pub fn history_view(&self) -> &wgpu::TextureView {
        &self.textures[(self.frame_index + 1) % 2].view
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn advance_frame(&mut self) {
        self.frame_index += 1;
    }

    pub fn process(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        settings_bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Hdr::process"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output,
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
        pass.set_bind_group(0, &self.bind_groups[self.frame_index % 2], &[]);
        pass.set_bind_group(1, settings_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
