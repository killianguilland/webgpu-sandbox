use wgpu::util::DeviceExt;

use crate::camera::{Camera, Projection};
use crate::context::GraphicsContext;
use crate::model::Model;
use crate::scenes::{Instance, Scene};
use cgmath::Matrix;
use std::collections::HashMap;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    position: [f32; 3],
    _padding: u32,
    color: [f32; 3],
    _padding2: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    inv_proj: [[f32; 4]; 4],
    inv_view: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_position: [0.0; 4],
            view: cgmath::Matrix4::identity().into(),
            view_proj: cgmath::Matrix4::identity().into(),
            inv_proj: cgmath::Matrix4::identity().into(),
            inv_view: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera, projection: &Projection) {
        use cgmath::SquareMatrix;
        self.view_position = camera.position.to_homogeneous().into();
        let proj = projection.calc_matrix();
        let view = camera.calc_matrix();
        let view_proj = proj * view;
        self.view = view.into();
        self.view_proj = view_proj.into();
        self.inv_proj = proj.invert().unwrap().into();
        self.inv_view = view.transpose().into();
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    pub model: [[f32; 4]; 4],
    pub normal: [[f32; 3]; 3],
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        let model =
            cgmath::Matrix4::from_translation(self.position) * cgmath::Matrix4::from(self.rotation);
        InstanceRaw {
            model: model.into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
        }
    }
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    topology: wgpu::PrimitiveTopology,
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: vertex_layouts,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: topology,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

// --------------------------------------------------------------------------
// Traits
// --------------------------------------------------------------------------

pub trait RenderPass {
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene: &dyn Scene,
        context: &GraphicsContext,
        renderer: &Renderer,
    );
}

// --------------------------------------------------------------------------
// Renderer Core
// --------------------------------------------------------------------------

pub struct Renderer {
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,

    pub light_buffer: wgpu::Buffer,
    pub light_bind_group_layout: wgpu::BindGroupLayout,
    pub light_bind_group: wgpu::BindGroup,

    pub texture_bind_group_layout: wgpu::BindGroupLayout,

    pub models: HashMap<String, Model>,
    pub instance_buffers: HashMap<String, (wgpu::Buffer, u32)>,

    pub projection: Projection,

    passes: Vec<Box<dyn RenderPass>>,
}

impl Renderer {
    pub fn new(context: &GraphicsContext) -> Self {
        let device = &context.device;

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_uniform = CameraUniform::new();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let light_uniform = LightUniform {
            position: [2.0, 2.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        };

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("light_bind_group_layout"),
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: Some("light_bind_group"),
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let projection = Projection::new(
            context.config.width,
            context.config.height,
            cgmath::Deg(45.0),
            0.1,
            100.0,
        );

        Self {
            camera_buffer,
            camera_bind_group_layout,
            camera_bind_group,
            light_buffer,
            light_bind_group_layout,
            light_bind_group,
            texture_bind_group_layout,
            models: HashMap::new(),
            instance_buffers: HashMap::new(),
            projection,
            passes: Vec::new(),
        }
    }

    pub fn add_pass(&mut self, pass: Box<dyn RenderPass>) {
        self.passes.push(pass);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.projection.resize(width, height);
    }

    pub fn get_model(&self, name: &str) -> Option<&Model> {
        self.models.get(name)
    }

    pub async fn load_model(
        &mut self,
        name: &str,
        context: &GraphicsContext,
    ) -> anyhow::Result<()> {
        if self.models.contains_key(name) {
            return Ok(());
        }
        let model = crate::resources::load_model(
            name,
            &context.device,
            &context.queue,
            &self.texture_bind_group_layout,
        )
        .await?;
        self.models.insert(name.to_string(), model);
        Ok(())
    }

    pub fn update(&mut self, context: &GraphicsContext, scene: &dyn Scene) {
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(scene.camera(), &self.projection);
        context.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        let lights = scene.lights();
        if !lights.is_empty() {
            let light = &lights[0];
            let light_uniform = LightUniform {
                position: light.position.into(),
                _padding: 0,
                color: light.color.into(),
                _padding2: 0,
            };
            context.queue.write_buffer(
                &self.light_buffer,
                0,
                bytemuck::cast_slice(&[light_uniform]),
            );
        }

        let mut instances_by_model: HashMap<String, Vec<InstanceRaw>> = HashMap::new();
        for instance in scene.instances() {
            instances_by_model
                .entry(instance.model_name.clone())
                .or_default()
                .push(instance.to_raw());
        }

        for (model_name, raw_instances) in instances_by_model {
            let instance_bytes = bytemuck::cast_slice(&raw_instances);
            // 2. Check if we already created a buffer for this model
            if let Some((buffer, _count)) = self.instance_buffers.get(&model_name) {
                // Buffer exists! Just overwrite the data. This is super fast.
                context.queue.write_buffer(buffer, 0, instance_bytes);
            } else {
                // Buffer doesn't exist yet (first frame). Create it!
                let buffer = context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Instance Buffer for {}", model_name)),
                        contents: instance_bytes,
                        // IMPORTANT: Make sure COPY_DST is here so we can write to it later
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
                self.instance_buffers
                    .insert(model_name, (buffer, raw_instances.len() as u32));
            }
        }
    }

    pub fn render(
        &self,
        context: &GraphicsContext,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene: &dyn Scene,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        for pass in &self.passes {
            pass.render(encoder, view, depth_view, scene, context, self);
        }
    }
}
