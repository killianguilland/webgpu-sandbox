struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    out.clip_position = vec4<f32>(out.uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv.y = 1.0 - out.uv.y;
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

// Group 0: G-Buffer
@group(0) @binding(0) var t_albedo: texture_2d<f32>;
@group(0) @binding(1) var t_normal: texture_2d<f32>;
@group(0) @binding(2) var t_pbr: texture_2d<f32>;
@group(0) @binding(3) var t_depth: texture_depth_2d;
@group(0) @binding(4) var s_sampler: sampler;

// Group 1: Camera
struct CameraUniform {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> camera: CameraUniform;

// Group 2: Lights
struct Light {
    position: vec3<f32>,
    // There's a 4-byte padding here due to WGSL alignment rules!
    color: vec3<f32>,
};
@group(2) @binding(0) var<uniform> light: Light;

// Group 3: Skybox
@group(3) @binding(0) var t_env: texture_cube<f32>;
@group(3) @binding(1) var s_env: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Get the depth
    let depth = textureLoad(t_depth, vec2<i32>(in.clip_position.xy), 0);
    if depth >= 1.0 { discard; } // Don't calculate lighting for the sky!
    
    // 2. Recreate the squashed Clip Space coordinate
    // UV is [0, 1], Clip Space is [-1, 1]. Note the inverted Y!
    let clip_space = vec4<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, depth, 1.0);
    
    // 3. Un-squash the perspective
    let view_space = camera.inv_proj * clip_space;
    
    // 4. Move it back into the 3D world (divide by W to complete the perspective un-squash)
    let world_position = camera.inv_view * (view_space / view_space.w);
    
    // // Debug: Let's output the World Position as a color! 
    // // We scale it down so it fits in the [0, 1] color range.
    // return vec4<f32>(world_position.xyz * 0.1, 1.0);

    // 2. Read G-Buffer Data
    let albedo = textureSample(t_albedo, s_sampler, in.uv).rgb;
    let normal = normalize(textureSample(t_normal, s_sampler, in.uv).xyz);
    let pbr = textureSample(t_pbr, s_sampler, in.uv).rg;
    let metallic = pbr.r;
    let roughness = pbr.g;
    // 3. PBR Vectors
    let N = normal; // Normal Vector
    let V = normalize(camera.view_pos.xyz - world_position.xyz); // View Vector
    let L = normalize(light.position - world_position.xyz); // Light Vector
    let H = normalize(V + L); // Halfway Vector
    // 4. Base Reflectivity (F0)
    // Non-metals use 0.04, metals use their Albedo color
    var F0 = vec3<f32>(0.04);
    F0 = mix(F0, albedo, metallic);
    // 5. Light Radiance (Inverse Square Law)
    let distance = length(light.position - world_position.xyz);
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
    let Lo = (kD * albedo / PI + specular) * radiance * NdotL;
    
    // 9. IBL (Image Based Lighting)
    // Calculate where the camera ray bounces off the surface
    let R = reflect(-V, N);

    // Sample the skybox! The rougher the surface, the higher the Mipmap level (blurrier)
    let max_reflection_lod = 8.0; 
    let reflection = textureSampleLevel(t_env, s_env, R, roughness * max_reflection_lod).rgb;

    // How much light bounces off like a mirror? (F0 is our base reflectivity)
    let kS_ibl = F0;

    // How much light gets absorbed and becomes diffuse color?
    let kD_ibl = (vec3<f32>(1.0) - kS_ibl) * (1.0 - metallic);

    // Combine the diffuse ambient (darkened heavily) and the shiny skybox reflection!
    let ambient = (kD_ibl * albedo * 0.03) + (reflection * kS_ibl);

    return vec4<f32>(ambient + Lo, 1.0);
}
