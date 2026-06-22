use std::ops::Range;

use crate::texture;
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFilter {
    //All, // Will be useful for shadow maps
    TransparentsOnly,
    OpaqueOnly,
}

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct Node {
    pub name: String,
    pub local_transform: [[f32; 4]; 4],
    pub global_transform: [[f32; 4]; 4],
    pub mesh_indices: Vec<usize>, // Which meshes this node renders
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub nodes: Vec<Node>,
    pub node_buffer: wgpu::Buffer,
    pub hierarchy_bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub base_color_factor: [f32; 4],
    // [Emissive R, Emissive G, Emissive B, Occlusion Strength]
    pub emissive_occlusion: [f32; 4],
    // [Metallic Factor, Roughness Factor, Normal Scale, Alpha Cutoff]
    pub mr_factors: [f32; 4],
}

pub struct Material {
    #[allow(unused)]
    pub name: String,
    #[allow(unused)]
    pub diffuse_texture: texture::Texture,
    #[allow(unused)]
    pub normal_texture: texture::Texture,
    #[allow(unused)]
    pub metalness_texture: texture::Texture,
    #[allow(unused)]
    pub roughness_texture: texture::Texture,
    #[allow(unused)]
    pub emissive_texture: texture::Texture,
    #[allow(unused)]
    pub occlusion_texture: texture::Texture,
    pub uniforms: MaterialUniform,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub is_transparent: bool,
}

impl Material {
    pub fn new(
        device: &wgpu::Device,
        name: &str,
        diffuse_texture: texture::Texture,
        normal_texture: texture::Texture,
        metalness_texture: texture::Texture,
        roughness_texture: texture::Texture,
        emissive_texture: texture::Texture,
        occlusion_texture: texture::Texture,
        layout: &wgpu::BindGroupLayout,
        is_transparent: bool,
        uniforms: MaterialUniform,
    ) -> Self {
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Uniforms", name)),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&metalness_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&metalness_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&roughness_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&roughness_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&emissive_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&emissive_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&occlusion_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Sampler(&occlusion_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
            label: Some(name),
        });

        Self {
            name: String::from(name),
            diffuse_texture,
            normal_texture,
            roughness_texture,
            metalness_texture,
            occlusion_texture,
            emissive_texture,
            bind_group,
            is_transparent,
            uniforms,
            uniform_buffer,
        }
    }
}

pub struct Mesh {
    #[allow(unused)]
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

pub trait DrawModel<'a> {
    #[allow(unused)]
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    // fn draw_model(
    //     &mut self,
    //     model: &'a Model,
    //     camera_bind_group: &'a wgpu::BindGroup,
    //     light_bind_group: &'a wgpu::BindGroup,
    // );
    // fn draw_model_instanced(
    //     &mut self,
    //     model: &'a Model,
    //     instances: Range<u32>,
    //     camera_bind_group: &'a wgpu::BindGroup,
    //     light_bind_group: &'a wgpu::BindGroup,
    // );

    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        mesh_filter: MeshFilter,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    // fn draw_model(
    //     &mut self,
    //     model: &'b Model,
    //     camera_bind_group: &'b wgpu::BindGroup,
    //     light_bind_group: &'b wgpu::BindGroup,
    // ) {
    //     self.draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    // }

    // fn draw_model_instanced(
    //     &mut self,
    //     model: &'b Model,
    //     instances: Range<u32>,
    //     camera_bind_group: &'b wgpu::BindGroup,
    //     light_bind_group: &'b wgpu::BindGroup,
    // ) {
    //     for mesh in &model.meshes {
    //         let material = &model.materials[mesh.material];
    //         self.draw_mesh_instanced(
    //             mesh,
    //             material,
    //             instances.clone(),
    //             camera_bind_group,
    //             light_bind_group,
    //         );
    //     }
    // }

    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        mesh_filter: MeshFilter,
    ) {
        for (i, node) in model.nodes.iter().enumerate() {
            // 1. Calculate dynamic offset for this specific node
            let dynamic_offset = i as u32 * 256;

            // 2. Bind the hierarchy group at index 4 with the offset!
            self.set_bind_group(4, &model.hierarchy_bind_group, &[dynamic_offset]);

            // 3. Draw all meshes attached to this node
            for &mesh_idx in &node.mesh_indices {
                let mesh = &model.meshes[mesh_idx];
                let material = &model.materials[mesh.material];

                let should_draw = match mesh_filter {
                    MeshFilter::TransparentsOnly => material.is_transparent,
                    MeshFilter::OpaqueOnly => !material.is_transparent,
                };

                if should_draw {
                    self.draw_mesh_instanced(
                        mesh,
                        material,
                        instances.clone(),
                        camera_bind_group,
                        light_bind_group,
                    );
                }
            }
        }
    }
}
