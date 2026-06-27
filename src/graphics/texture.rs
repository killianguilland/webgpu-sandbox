use anyhow::*;
use image::GenericImageView;

pub struct Texture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: wgpu::Extent3d,
}

impl Texture {
    pub fn from_solid_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rgba: [u8; 4],
        label: Option<&str>,
        is_linear: bool,
    ) -> Result<Self> {
        let img =
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)));
        Self::from_image(
            device,
            queue,
            &img,
            label,
            is_linear,
            [wgpu::AddressMode::Repeat; 3],
        )
    }

    pub fn from_raw_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        width: u32,
        height: u32,
        label: Option<&str>,
    ) -> Result<Self> {
        let img_buf = image::RgbaImage::from_raw(width, height, bytes.to_vec()).unwrap();
        let img = image::DynamicImage::ImageRgba8(img_buf);
        Self::from_image(
            device,
            queue,
            &img,
            label,
            true,
            [wgpu::AddressMode::Repeat; 3],
        )
    }

    // pub fn fallback_diffuse(
    //     device: &wgpu::Device,
    //     queue: &wgpu::Queue,
    //     label: Option<&str>,
    // ) -> Result<Self> {
    //     Self::from_solid_rgba(device, queue, [100, 100, 100, 255], label, false)
    // }

    // pub fn fallback_normal(
    //     device: &wgpu::Device,
    //     queue: &wgpu::Queue,
    //     label: Option<&str>,
    // ) -> Result<Self> {
    //     Self::from_solid_rgba(device, queue, [128, 128, 255, 255], label, true)
    // }

    // pub fn fallback_metalness(
    //     device: &wgpu::Device,
    //     queue: &wgpu::Queue,
    //     label: Option<&str>,
    // ) -> Result<Self> {
    //     Self::from_solid_rgba(device, queue, [0, 0, 0, 255], label, true)
    // }

    // pub fn fallback_roughness(
    //     device: &wgpu::Device,
    //     queue: &wgpu::Queue,
    //     label: Option<&str>,
    // ) -> Result<Self> {
    //     Self::from_solid_rgba(device, queue, [128, 128, 128, 255], label, true)
    // }

    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
        is_linear: bool,
        address_modes: [wgpu::AddressMode; 3],
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label), is_linear, address_modes)
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
        is_linear: bool,
        address_modes: [wgpu::AddressMode; 3],
    ) -> Result<Self> {
        let format = if is_linear {
            wgpu::TextureFormat::Rgba8Unorm
        } else {
            wgpu::TextureFormat::Rgba8UnormSrgb
        };

        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: address_modes[0],
            address_mode_v: address_modes[1],
            address_mode_w: address_modes[2],
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
            size,
        })
    }

    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float; // 1.

    pub fn create_render_target(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        format: wgpu::TextureFormat,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT // 3.
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let compare = if format == Self::DEPTH_FORMAT {
            Some(wgpu::CompareFunction::LessEqual)
        } else {
            None
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: compare,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            size,
        }
    }

    pub(crate) fn create_2d_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        mag_filter: wgpu::FilterMode,
        label: Option<&str>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        Self::create_texture(
            device,
            label,
            size,
            format,
            usage,
            wgpu::TextureDimension::D2,
            mag_filter,
        )
    }

    pub fn create_texture(
        device: &wgpu::Device,
        label: Option<&str>,
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        dimension: wgpu::TextureDimension,
        mag_filter: wgpu::FilterMode,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            size,
        }
    }
}

pub struct CubeTexture {
    texture: wgpu::Texture,
    sampler: wgpu::Sampler,
    view: wgpu::TextureView,
}

impl CubeTexture {
    pub fn create_2d(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        mip_level_count: u32,
        usage: wgpu::TextureUsages,
        mag_filter: wgpu::FilterMode,
        label: Option<&str>,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                // A cube has 6 sides, so we need 6 layers
                depth_or_array_layers: 6,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::Cube),
            array_layer_count: Some(6),
            ..Default::default()
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            sampler,
            view,
        }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

pub struct RenderTarget {
    pub texture: Texture,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    format: wgpu::TextureFormat,
    label: String,
}

impl RenderTarget {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        format: wgpu::TextureFormat,
        label: &str,
    ) -> Self {
        let texture = Texture::create_render_target(device, config, format, label);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{} Bind Group Layout", label)),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} Bind Group", label)),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });

        Self {
            texture,
            bind_group_layout,
            bind_group,
            format,
            label: label.to_string(),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        *self = Self::new(device, config, self.format, &self.label);
    }
}
