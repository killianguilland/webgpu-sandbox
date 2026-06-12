use std::io::Cursor;

use wgpu::util::DeviceExt;

use image::codecs::hdr::HdrDecoder;

use crate::{model, texture};

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let data = {
        let path = std::path::Path::new(env!("OUT_DIR"))
            .join("res")
            .join(file_name);
        std::fs::read(path)?
    };

    Ok(data)
}

pub async fn load_texture(
    file_name: &str,
    is_linear: bool,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name, is_linear)
}

use asset_importer::{Importer, TextureType, postprocess::PostProcessSteps};

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
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
        let diffuse_texture = if m.texture_count(TextureType::Diffuse) > 0 {
            let tex = m.texture(TextureType::Diffuse, 0).unwrap();
            // tex.file contient le chemin de la texture (ex: "textures/diffuse.png")
            // Ton helper load_texture va automatiquement le chercher dans "OUT_DIR/res/" !
            load_texture(&tex.path, false, device, queue).await?
        } else {
            texture::Texture::fallback_diffuse(
                device,
                queue,
                Some(&format!("{}::diffuse_fallback", name)),
            )?
        };

        // Recherche de la texture de normales ou de bump
        let normal_texture = if m.texture_count(TextureType::Normals) > 0 {
            let tex = m.texture(TextureType::Normals, 0).unwrap();
            load_texture(&tex.path, true, device, queue).await?
        } else if m.texture_count(TextureType::Height) > 0 {
            let tex = m.texture(TextureType::Height, 0).unwrap();
            load_texture(&tex.path, true, device, queue).await?
        } else {
            texture::Texture::fallback_normal(
                device,
                queue,
                Some(&format!("{}::normal_fallback", name)),
            )?
        };

        materials.push(model::Material::new(
            device,
            &name,
            diffuse_texture,
            normal_texture,
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

    Ok(model::Model { meshes, materials })
}

pub struct HdrLoader {
    texture_format: wgpu::TextureFormat,
    equirect_layout: wgpu::BindGroupLayout,
    equirect_to_cubemap: wgpu::ComputePipeline,
}

impl HdrLoader {
    pub fn new(device: &wgpu::Device) -> Self {
        let module =
            device.create_shader_module(wgpu::include_wgsl!("shaders/equirectangular.wgsl"));
        let texture_format = wgpu::TextureFormat::Rgba32Float;
        let equirect_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HdrLoader::equirect_layout"),
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
            equirect_to_cubemap,
            texture_format,
            equirect_layout,
        }
    }

    pub fn from_equirectangular_bytes(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        dst_size: u32,
        label: Option<&str>,
    ) -> anyhow::Result<texture::CubeTexture> {
        let hdr_decoder = HdrDecoder::new(Cursor::new(data))?;
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

        Ok(dst)
    }
}
