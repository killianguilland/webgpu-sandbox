struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// No vertex buffers needed! We use the built-in @builtin(vertex_index)
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

// Group 0: GBuffer (We only need Normal and Depth for SSAO)
@group(0) @binding(1) var t_normal: texture_2d<f32>;
@group(0) @binding(3) var t_depth: texture_depth_2d;
@group(0) @binding(4) var s_gbuffer: sampler;

// Group 1: Camera Uniform
struct CameraUniform {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}
@group(1) @binding(0) var<uniform> camera: CameraUniform;

// Group 2: Noise Texture
@group(2) @binding(0) var t_noise: texture_2d<f32>;
@group(2) @binding(1) var s_noise: sampler;

// Our Hardcoded Settings!
const RADIUS: f32 = 0.5;
const KERNEL_SIZE: u32 = 16u;
const SSAO_KERNEL = array<vec4<f32>, 16>(
    vec4<f32>( 0.052,  0.014,  0.084, 0.0), vec4<f32>(-0.065,  0.111,  0.158, 0.0),
    vec4<f32>( 0.176, -0.125,  0.222, 0.0), vec4<f32>(-0.188, -0.203,  0.108, 0.0),
    vec4<f32>( 0.287,  0.198,  0.154, 0.0), vec4<f32>(-0.324,  0.187,  0.267, 0.0),
    vec4<f32>( 0.254, -0.388,  0.312, 0.0), vec4<f32>(-0.267, -0.422,  0.188, 0.0),
    vec4<f32>( 0.485,  0.334,  0.288, 0.0), vec4<f32>(-0.556,  0.278,  0.311, 0.0),
    vec4<f32>( 0.444, -0.589,  0.366, 0.0), vec4<f32>(-0.478, -0.512,  0.422, 0.0),
    vec4<f32>( 0.765,  0.432,  0.211, 0.0), vec4<f32>(-0.688,  0.511,  0.455, 0.0),
    vec4<f32>( 0.655, -0.711,  0.388, 0.0), vec4<f32>(-0.722, -0.655,  0.299, 0.0)
);
const ACNE_BIAS: f32 = 0.025;
const POWER: f32 = 1.0;

fn reconstruct_view_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    // 1. Convert UV to Normalized Device Coordinates (-1 to 1)
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    
    // 2. Un-project using the camera's inverse projection matrix
    let view_pos = camera.inv_proj * ndc;
    
    // 3. Perform perspective divide
    return (view_pos / view_pos.w).xyz;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let depth = textureLoad(t_depth, vec2<i32>(in.clip_position.xy), 0);
    if depth >= 1.0 { discard; }

    let view_pos = reconstruct_view_position(in.uv, depth);

    let world_normal = normalize(textureSample(t_normal, s_gbuffer, in.uv).xyz);
    let view_normal = normalize((camera.view * vec4<f32>(world_normal, 0.0)).xyz);

    // 1. Get the screen dimensions to tile our 4x4 noise texture
    let screen_dim = vec2<f32>(textureDimensions(t_depth));
    let noise_scale = screen_dim / 4.0; // 4 is the noise texture size here
    
    // 2. Sample the random vector (we expand it from 0..1 to -1..1)
    let random_vec = normalize(textureSample(t_noise, s_noise, in.uv * noise_scale).xyz * 2.0 - 1.0);
    
    // 3. Build the TBN Matrix to orient our kernel along the normal
    let tangent = normalize(random_vec - view_normal * dot(random_vec, view_normal));
    let bitangent = cross(view_normal, tangent);
    let TBN = mat3x3<f32>(tangent, bitangent, view_normal);

    var occlusion = 0.0;

    for (var i = 0u; i < KERNEL_SIZE; i++) {
        let sample_vector = SSAO_KERNEL[i].xyz;
        let rotated_vector = TBN * sample_vector * RADIUS;
        let view_space_location = view_pos + rotated_vector;

        // 1. Project the sample back to the 2D screen
        let world_pos = camera.inv_view * vec4<f32>(view_space_location, 1.0);
        var offset = camera.view_proj * world_pos;
        offset = offset / offset.w; // Perspective Divide to get NDC (-1 to 1)
        
        // 2. Convert NDC to UV coordinates (0 to 1)
        var offset_uv = offset.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
        
        // 3. Read the actual depth of the geometry at this pixel
        let sampled_depth = textureSample(t_depth, s_gbuffer, offset_uv);
        
        // 4. Use our helper function to find the 3D view-space location of the geometry!
        let geometry_view_pos = reconstruct_view_position(offset_uv, sampled_depth);

        // Check 1: Is the sample behind the geometry?
        let is_behind = view_space_location.z + ACNE_BIAS < geometry_view_pos.z;
        
        // Check 2: Is the geometry actually close enough to the sample to cast a shadow?
        let range_check = smoothstep(0.0, 1.0, RADIUS / abs(view_pos.z - geometry_view_pos.z));
        
        if is_behind {
            occlusion += 1.0 * range_check;
        }
    }
    // Convert from [0, 16] to a [0.0, 1.0] ambient light multiplier
    let ambient_light_factor = pow(1.0 - (occlusion / f32(KERNEL_SIZE)), POWER);
    
    return vec4<f32>(vec3<f32>(ambient_light_factor), 1.0);
}