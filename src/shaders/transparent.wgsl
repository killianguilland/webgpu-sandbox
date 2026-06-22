// Vertex shader

struct Camera {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> camera: Camera;

struct MeshUniforms {
    node_index: u32,
}
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

const PI: f32 = 3.14159265359;

// D: Normal Distribution (GGX)
fn DistributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;
    let num = a2;
    let denom = (NdotH2 * (a2 - 1.0) + 1.0);
    return num / (PI * denom * denom);
}
// G: Geometry Shadowing (Schlick-GGX)
fn GeometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    let num = NdotV;
    let denom = NdotV * (1.0 - k) + k;
    return num / denom;
}
fn GeometrySmith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = GeometrySchlickGGX(NdotV, roughness);
    let ggx1 = GeometrySchlickGGX(NdotL, roughness);
    return ggx1 * ggx2;
}
// F: Fresnel Reflectance (Schlick's approximation)
fn fresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
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

// Group 5: SSAO
@group(5) @binding(0) var t_ssao: texture_2d<f32>;
@group(5) @binding(1) var s_ssao: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // ALBEDO
    let albedo = textureSample(t_diffuse, s_diffuse, in.tex_coords) * material.base_color_factor;

    if albedo.a < material.mr_factors.w {
        discard;
    }

    // Optional: If it's effectively invisible, don't waste GPU cycles
    //if albedo.a < 0.01 { discard; }

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

    // 3. PBR Vectors
    let N = final_world_normal; // Normal Vector
    let V = normalize(camera.view_pos.xyz - in.world_position.xyz); // View Vector
    let L = normalize(light.position - in.world_position.xyz); // Light Vector
    let H = normalize(V + L); // Halfway Vector
    // 4. Base Reflectivity (F0)
    // Non-metals use 0.04, metals use their Albedo color
    var F0 = vec3<f32>(0.04);
    F0 = mix(F0, albedo.rgb, metallic);
    // 5. Light Radiance (Inverse Square Law)
    let distance = length(light.position - in.world_position.xyz);
    let attenuation = 1.0 / (distance * distance);
    let radiance = light.color * attenuation;
    // 6. The Cook-Torrance BRDF Equation!
    let NDF = DistributionGGX(N, H, roughness);   
    let G   = GeometrySmith(N, V, L, roughness);      
    let F   = fresnelSchlick(max(dot(H, V), 0.0), F0);       
    let numerator    = NDF * G * F;
    let denominator  = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
    let specular     = numerator / denominator;
    // 7. Energy Conservation
    let kS = F;
    let kD = (vec3<f32>(1.0) - kS) * (1.0 - metallic);
    // 8. Final Lighting Math
    let NdotL = max(dot(N, L), 0.0);
    let diffuse_light = (kD * albedo.rgb / PI) * radiance * NdotL;
    let specular_light = specular * radiance * NdotL;
    // let Lo = (kD * albedo.rgb / PI + specular) * radiance * NdotL;
    
    // 9. IBL (Image Based Lighting)
    // Calculate where the camera ray bounces off the surface
    let R = reflect(-V, N);

    // Sample the skybox! The rougher the surface, the higher the Mipmap level (blurrier)
    let max_reflection_lod = 8.0; 
    let reflection = textureSampleLevel(env_map, env_sampler, R, roughness * max_reflection_lod).rgb;

    // How much light bounces off like a mirror? (F0 is our base reflectivity)
    let kS_ibl = F0;

    // How much light gets absorbed and becomes diffuse color?
    let kD_ibl = (vec3<f32>(1.0) - kS_ibl) * (1.0 - metallic);

    // Combine the diffuse ambient (darkened heavily) and the shiny skybox reflection!
    // let ambient = (kD_ibl * albedo.rgb * 0.03) + (reflection * kS_ibl);
    
    // Sample the exact pixel from the screen-space SSAO texture
    let ssao_factor = textureLoad(t_ssao, vec2<i32>(in.clip_position.xy), 0).r;
    let final_occlusion = ssao_factor * occlusion; // We merge baked occlusion and ssao

    let diffuse_ambient = kD_ibl * albedo.rgb * 0.03 * final_occlusion;
    let specular_ambient = reflection * kS_ibl * final_occlusion;

    // 10. PRE-MULTIPLY ALPHA!
    let final_diffuse = (diffuse_light + diffuse_ambient) * albedo.a;
    let final_specular = specular_light + specular_ambient;

    let emissive = textureSample(t_emissive, s_emissive, in.tex_coords).rgb * material.emissive_occlusion.xyz;

    let lit_color = final_diffuse + final_specular + (emissive * albedo.a);
    return vec4<f32>(lit_color, albedo.a);
}
