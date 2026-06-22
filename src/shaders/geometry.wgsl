// Vertex shader

struct Camera {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}
struct MeshUniforms {
    node_index: u32,
}
@group(1) @binding(0) var<uniform> camera: Camera;
@group(4) @binding(0) var<storage, read> node_transforms: array<mat4x4<f32>>;
@group(4) @binding(1) var<uniform> mesh_uniforms: MeshUniforms;

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
}
@group(2) @binding(0)
var<uniform> light: Light;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}
struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_view_position: vec3<f32>,
    @location(3) world_light_position: vec3<f32>,
    @location(4) world_normal: vec3<f32>,
    @location(5) world_tangent: vec3<f32>,
    @location(6) world_bitangent: vec3<f32>,
}


@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let instance_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let node_matrix = node_transforms[mesh_uniforms.node_index];

    let final_model_matrix = instance_matrix * node_matrix;
    let final_normal_matrix = mat3x3<f32>(
        final_model_matrix[0].xyz,
        final_model_matrix[1].xyz,
        final_model_matrix[2].xyz,
    );
    let world_position = final_model_matrix * vec4<f32>(model.position, 1.0);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_position;
    out.tex_coords = model.tex_coords;
    out.world_normal = normalize(final_normal_matrix * model.normal);
    out.world_tangent = normalize(final_normal_matrix * model.tangent);   
    out.world_bitangent = normalize(final_normal_matrix * model.bitangent);
    out.world_position = world_position.xyz;
    out.world_view_position = camera.view_pos.xyz;
    return out;
}

// Fragment shader

struct MaterialUniform {
    base_color_factor: vec4<f32>,
    emissive_occlusion: vec4<f32>,
    mr_factors: vec4<f32>,
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0)@binding(1) var s_diffuse: sampler;
@group(0)@binding(2) var t_normal: texture_2d<f32>;
@group(0) @binding(3) var s_normal: sampler;
@group(0) @binding(4) var t_metallic: texture_2d<f32>;
@group(0) @binding(5) var s_metallic: sampler;
@group(0) @binding(6) var t_roughness: texture_2d<f32>;
@group(0) @binding(7) var s_roughness: sampler;
@group(0) @binding(8) var t_emissive: texture_2d<f32>;
@group(0) @binding(9) var s_emissive: sampler;
@group(0) @binding(10) var t_occlusion: texture_2d<f32>;
@group(0) @binding(11) var s_occlusion: sampler;
@group(0) @binding(12) var<uniform> material: MaterialUniform;

@group(3)
@binding(0)
var env_map: texture_cube<f32>;
@group(3)
@binding(1)
var env_sampler: sampler;

struct GBufferOutput {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) pbr: vec4<f32>,
    @location(3) emissive: vec4<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    // ALBEDO
    let albedo = textureSample(t_diffuse, s_diffuse, in.tex_coords) * material.base_color_factor;

    if albedo.a < material.mr_factors.w {
        discard;
    }

    // NORMAL
    //let normal = vec4<f32>(normalize(in.world_normal), 1.0); // This is the triangle's normal (without the normal map texture)
    
    // Tangent space normal from the texture
    // Color space [0, 1]
    let object_normal = textureSample(t_normal, s_normal, in.tex_coords); 
    
    // Expand it from [0, 1] color space to [-1, 1] mathematical vector space
    let unpacked_normal = object_normal.xyz * 2.0 - 1.0;
    
    let scaled_normal = vec3<f32>(
        unpacked_normal.x * material.mr_factors.z,
        unpacked_normal.y * material.mr_factors.z,
        unpacked_normal.z
    );
    let tangent_normal = scaled_normal;
    
    // 3. Build the TBN arrows
    // We transform our three vectors into an orthogonal family of vectors (perpendicular)
    let world_tangent = normalize(in.world_tangent - dot(in.world_tangent, in.world_normal) * in.world_normal);
    let world_bitangent = cross(world_tangent, in.world_normal);
    
    // Pack them into the rotation matrix
    let TBN = mat3x3(
        world_tangent,
        world_bitangent,
        in.world_normal,
    );
    
    // Rotate the flat normal into 3D World Space!
    let final_world_normal = normalize(TBN * tangent_normal);
    
    // Output our final normal to the G-Buffer
    let normal_out = vec4<f32>(final_world_normal, 1.0);
    
    // PBR
    let metallic = textureSample(t_metallic, s_metallic, in.tex_coords).b * material.mr_factors.x;
    let roughness = textureSample(t_roughness, s_roughness, in.tex_coords).g * material.mr_factors.y;
    let occlusion = textureSample(t_occlusion, s_occlusion, in.tex_coords).r * material.emissive_occlusion.w;
    let pbr = vec4<f32>(metallic, roughness, occlusion, 1.0);

    let emissive = textureSample(t_emissive, s_emissive, in.tex_coords).rgb * material.emissive_occlusion.xyz;

    return GBufferOutput(albedo, normal_out, pbr, vec4<f32>(emissive, 1.0));
}
