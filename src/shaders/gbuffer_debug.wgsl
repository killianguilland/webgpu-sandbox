// src/shaders/gbuffer_debug.wgsl

// Your existing fullscreen quad vertex shader
struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    var out: VertexOutput;
    // Generate a triangle that covers the whole screen
    out.uv = vec2<f32>(
        f32((vi << 1u) & 2u),
        f32(vi & 2u),
    );
    out.clip_position = vec4<f32>(out.uv * 2.0 - 1.0, 0.0, 1.0);
    // We need to invert the y coordinate so the image
    // is not upside down
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

// Bind our entire GBuffer at Group 0!
@group(0) @binding(0) var t_albedo: texture_2d<f32>;
@group(0) @binding(1) var t_normal: texture_2d<f32>;
@group(0) @binding(2) var t_pbr: texture_2d<f32>;
@group(0) @binding(3) var t_depth: texture_depth_2d;
@group(0) @binding(4) var s_sampler: sampler;

// We will use a uniform buffer to tell the shader which texture to draw
struct DebugSettings {
    mode: u32,
}
@group(1) @binding(0) var<uniform> settings: DebugSettings;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (settings.mode == 0u) {
        // Albedo
        return textureSample(t_albedo, s_sampler, in.uv);
    } else if (settings.mode == 1u) {
        // Normal [-1, 1] mapped to [0, 1] for easy viewing
        let normal = textureSample(t_normal, s_sampler, in.uv).rgb;
        return vec4<f32>(normal * 0.5 + 0.5, 1.0);
    } else if (settings.mode == 2u) {
        // PBR (R = Metallic, G = Roughness, B = 0)
        let pbr = textureSample(t_pbr, s_sampler, in.uv).rg;
        return vec4<f32>(pbr.r, pbr.g, 0.0, 1.0);
    } else {
        // Depth
        let depth_val = textureLoad(t_depth, vec2<i32>(in.clip_position.xy), 0);
        return vec4<f32>(depth_val, depth_val, depth_val, 1.0);
    }
}
