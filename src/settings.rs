use std::collections::HashMap;

pub struct RenderSettings {
    // Maps a pass name to its enabled/disabled state
    pub pass_states: HashMap<String, bool>,
    pub debug_mode: u32,

    // Lighting
    pub light_position: [f32; 3],
    pub light_color: [f32; 3],
    pub light_intensity: f32,
    pub ambient_intensity: f32,

    // SSAO
    pub ssao_radius: f32,
    pub ssao_bias: f32,
    pub ssao_power: f32,
    pub ssao_kernel_size: u32,

    // Camera / Post-processing
    pub hdr_exposure: f32,
    pub camera_fov: f32,

    pub resolution_scale: f32,

    pub changed: bool,
    pub light_follows_camera: bool,
}

impl RenderSettings {
    pub fn new() -> Self {
        Self {
            pass_states: HashMap::new(),
            debug_mode: 0,

            light_position: [2.0, 2.0, 2.0],
            light_color: [1.0, 1.0, 1.0],
            light_intensity: 150.0,
            ambient_intensity: 0.03,

            ssao_radius: 0.25,
            ssao_bias: 0.025,
            ssao_power: 2.0,
            ssao_kernel_size: 16,

            hdr_exposure: 1.0,
            camera_fov: 45.0,

            resolution_scale: 1.0,

            changed: true,
            light_follows_camera: false,
        }
    }

    pub fn is_pass_enabled(&self, name: &str) -> bool {
        // Default to true if not found
        *self.pass_states.get(name).unwrap_or(&true)
    }
}
