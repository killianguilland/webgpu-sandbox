use std::io::Cursor;

use wgpu::util::DeviceExt;

use image::codecs::hdr::HdrDecoder;

use crate::{model, texture};
use asset_importer::{Importer, TextureType, postprocess::PostProcessSteps, texture::TextureData};

use std::collections::HashMap;

pub struct ResourceManager {
    pub models: HashMap<String, model::Model>,
    pub cube_textures: HashMap<String, texture::CubeTexture>,
    pub bind_groups: HashMap<String, wgpu::BindGroup>,

    equirect_layout: wgpu::BindGroupLayout,
    equirect_to_cubemap: wgpu::ComputePipeline,
    texture_format: wgpu::TextureFormat,
}

impl ResourceManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let module =
            device.create_shader_module(wgpu::include_wgsl!("shaders/equirectangular.wgsl"));
        let texture_format = wgpu::TextureFormat::Rgba32Float;
        let equirect_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ResourceManager::equirect_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: texture_format,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&equirect_layout)],
            immediate_size: 0,
        });

        let equirect_to_cubemap =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("equirect_to_cubemap"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("compute_equirect_to_cubemap"),
                compilation_options: Default::default(),
                cache: None,
            });

        Self {
            models: HashMap::new(),
            cube_textures: HashMap::new(),
            bind_groups: HashMap::new(),
            equirect_to_cubemap,
            texture_format,
            equirect_layout,
        }
    }

    pub fn get_model(&self, name: &str) -> Option<&model::Model> {
        self.models.get(name)
    }

    pub fn get_bind_group(&self, name: &str) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(name)
    }

    pub fn get_cube_texture(&self, name: &str) -> Option<&texture::CubeTexture> {
        self.cube_textures.get(name)
    }

    pub async fn load_binary(&self, file_name: &str) -> anyhow::Result<Vec<u8>> {
        let data = {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(file_name);
            std::fs::read(path)?
        };
        Ok(data)
    }

    pub async fn load_texture(
        &self,
        file_name: &str,
        is_linear: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<texture::Texture> {
        let data = self.load_binary(file_name).await?;
        texture::Texture::from_bytes(device, queue, &data, file_name, is_linear)
    }

    pub fn load_embedded_texture(
        &self,
        tex: &asset_importer::texture::Texture,
        is_linear: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<texture::Texture> {
        match tex.data() {
            Ok(asset_importer::texture::TextureData::Compressed(bytes)) => {
                texture::Texture::from_bytes(device, queue, &bytes, "embedded", is_linear)
            }
            Ok(asset_importer::texture::TextureData::Texels(texels)) => {
                let (width, height) = tex.dimensions();
                let vec: Vec<u8> = texels
                    .iter()
                    .flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b, pixel.a])
                    .collect();
                let image = image::RgbaImage::from_raw(width, height, vec).unwrap();
                let dynamic_image = image::DynamicImage::ImageRgba8(image);
                texture::Texture::from_image(
                    device,
                    queue,
                    &dynamic_image,
                    Some("embedded texture"),
                    is_linear,
                )
            }
            Err(e) => Err(anyhow::anyhow!("Embedded texture error: {:?}", e)),
        }
    }

    pub async fn load_material_texture(
        &self,
        material: &asset_importer::material::Material,
        scene: &asset_importer::scene::Scene,
        types_to_try: &[TextureType],
        is_linear: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<Option<texture::Texture>> {
        for &ty in types_to_try {
            if material.texture_count(ty) > 0 {
                let tex = material.texture(ty, 0).unwrap();
                if tex.path.starts_with('*') {
                    if let Ok(Some(embedded)) = scene.embedded_texture_by_name(&tex.path) {
                        return Ok(Some(
                            self.load_embedded_texture(&embedded, is_linear, device, queue)?,
                        ));
                    }
                } else {
                    return Ok(Some(
                        self.load_texture(&tex.path, is_linear, device, queue)
                            .await?,
                    ));
                }
            }
        }
        Ok(None)
    }

    pub async fn load_model(
        &mut self,
        file_name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<()> {
        if self.models.contains_key(file_name) {
            return Ok(());
        }

        let model_path = std::path::Path::new(env!("OUT_DIR"))
            .join("res")
            .join(file_name);

        let path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Le chemin du modèle est invalide"))?;

        let scene = Importer::new()
            .import_file_with(path_str, |b| {
                b.with_post_process(
                    PostProcessSteps::TRIANGULATE
                        | PostProcessSteps::CALC_TANGENT_SPACE
                        | PostProcessSteps::JOIN_IDENTICAL_VERTICES
                        | PostProcessSteps::FLIP_UVS,
                )
            })
            .map_err(|e| anyhow::anyhow!("Assimp error during loading : {:?}", e))?;

        // 3. Chargement des matériaux
        let mut materials = Vec::new();
        for m in scene.materials() {
            let name = m.name();

            // Recherche de la texture diffuse
            let diffuse_texture = self
                .load_material_texture(
                    &m,
                    &scene,
                    &[TextureType::BaseColor, TextureType::Diffuse],
                    false,
                    device,
                    queue,
                )
                .await?
                .unwrap_or_else(|| {
                    texture::Texture::fallback_diffuse(
                        device,
                        queue,
                        Some(&format!("{}::diffuse_fallback", name)),
                    )
                    .unwrap()
                });

            // Recherche de la texture de normales ou de bump
            let normal_texture = self
                .load_material_texture(
                    &m,
                    &scene,
                    &[TextureType::Normals, TextureType::Height],
                    true,
                    device,
                    queue,
                )
                .await?
                .unwrap_or_else(|| {
                    texture::Texture::fallback_normal(
                        device,
                        queue,
                        Some(&format!("{}::normal_fallback", name)),
                    )
                    .unwrap()
                });

            // Recherche de la texture metalness
            let metalness_texture = self
                .load_material_texture(
                    &m,
                    &scene,
                    &[TextureType::Metalness, TextureType::GltfMetallicRoughness],
                    true,
                    device,
                    queue,
                )
                .await?
                .unwrap_or_else(|| {
                    texture::Texture::fallback_metalness(
                        device,
                        queue,
                        Some(&format!("{}::metalness_fallback", name)),
                    )
                    .unwrap()
                });

            // Recherche de la texture roughness
            let roughness_texture = self
                .load_material_texture(
                    &m,
                    &scene,
                    &[
                        TextureType::DiffuseRoughness,
                        TextureType::GltfMetallicRoughness,
                    ],
                    true,
                    device,
                    queue,
                )
                .await?
                .unwrap_or_else(|| {
                    texture::Texture::fallback_roughness(
                        device,
                        queue,
                        Some(&format!("{}::roughness_fallback", name)),
                    )
                    .unwrap()
                });

            materials.push(model::Material::new(
                device,
                &name,
                diffuse_texture,
                normal_texture,
                roughness_texture,
                metalness_texture,
                layout,
            ));
        }

        // 4. Chargement et formatage de la géométrie (Meshes)
        let mut meshes = Vec::new();
        for m in scene.meshes() {
            let positions = m.vertices();
            let normals = m.normals();
            let texcoords = m.texture_coords(0); // Premier canal UV
            let texcoords = texcoords.as_ref();
            let tangents = m.tangents();
            let tangents = tangents.as_ref();
            let bitangents = m.bitangents();
            let bitangents = bitangents.as_ref();

            let mut vertices = Vec::with_capacity(positions.len());

            for i in 0..positions.len() {
                let pos = positions[i];

                // Assimp garantit ces tableaux s'ils ont été demandés,
                // mais c'est toujours bien de sécuriser si le fichier d'origine est corrompu.
                let normal = if let Some(normals) = normals.as_ref() {
                    [normals[i].x, normals[i].y, normals[i].z]
                } else {
                    [0.0, 0.0, 0.0]
                };
                let tc = if let Some(uvs) = texcoords {
                    [uvs[i].x, uvs[i].y]
                } else {
                    [0.0, 0.0]
                };

                let tangent = if let Some(t) = tangents {
                    [t[i].x, t[i].y, t[i].z]
                } else {
                    [0.0, 0.0, 0.0]
                };

                let bitangent = if let Some(b) = bitangents {
                    [b[i].x, b[i].y, b[i].z]
                } else {
                    [0.0, 0.0, 0.0]
                };

                vertices.push(model::ModelVertex {
                    position: [pos.x, pos.y, pos.z],
                    tex_coords: tc,
                    normal,
                    tangent,
                    bitangent,
                });
            }

            let mut indices = Vec::new();
            for face in m.faces() {
                indices.extend_from_slice(&face.indices());
            }

            let mesh_name = m.name();

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Vertex Buffer", mesh_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Index Buffer", mesh_name)),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(model::Mesh {
                name: mesh_name,
                vertex_buffer,
                index_buffer,
                num_elements: indices.len() as u32,
                material: m.material_index() as usize,
            });
        }

        let model = model::Model { meshes, materials };
        self.models.insert(file_name.to_string(), model);

        Ok(())
    }

    pub async fn load_hdr_environment(
        &mut self,
        file_name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        dst_size: u32,
        label: Option<&str>,
    ) -> anyhow::Result<()> {
        if self.cube_textures.contains_key(file_name) {
            return Ok(());
        }

        let data = self.load_binary(file_name).await?;

        let hdr_decoder = HdrDecoder::new(Cursor::new(&data))?;
        let meta = hdr_decoder.metadata();

        let pixels = {
            let mut pixels = vec![[0.0, 0.0, 0.0, 0.0]; meta.width as usize * meta.height as usize];
            hdr_decoder.read_image_transform(
                |pix| {
                    let rgb = pix.to_hdr();
                    [rgb.0[0], rgb.0[1], rgb.0[2], 1.0f32]
                },
                &mut pixels[..],
            )?;
            pixels
        };

        let src = texture::Texture::create_2d_texture(
            device,
            meta.width,
            meta.height,
            self.texture_format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            wgpu::FilterMode::Linear,
            None,
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &src.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytemuck::cast_slice(&pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(src.size.width * std::mem::size_of::<[f32; 4]>() as u32),
                rows_per_image: Some(src.size.height),
            },
            src.size,
        );

        let dst = texture::CubeTexture::create_2d(
            device,
            dst_size,
            dst_size,
            self.texture_format,
            1,
            // We are going to write to `dst` texture so we
            // need to use a `STORAGE_BINDING`.
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            wgpu::FilterMode::Nearest,
            label,
        );

        let dst_view = dst.texture().create_view(&wgpu::TextureViewDescriptor {
            label,
            // Normally, you'd use `TextureViewDimension::Cube`
            // for a cube texture, but we can't use that
            // view dimension with a `STORAGE_BINDING`.
            // We need to access the cube texture layers
            // directly.
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &self.equirect_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&dst_view),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            timestamp_writes: None,
        });

        let num_workgroups = (dst_size + 15) / 16;
        pass.set_pipeline(&self.equirect_to_cubemap);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(num_workgroups, num_workgroups, 6);

        drop(pass);

        queue.submit([encoder.finish()]);

        let environment_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&dst.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&dst.sampler()),
                },
            ],
        });

        self.bind_groups
            .insert(file_name.to_string(), environment_bind_group);
        self.cube_textures.insert(file_name.to_string(), dst);

        Ok(())
    }
}
