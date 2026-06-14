use std::collections::HashMap;
pub struct RenderSettings {
    // Maps a pass name to its enabled/disabled state
    pub pass_states: HashMap<String, bool>,
    pub debug_mode: u32,
}
impl RenderSettings {
    pub fn new() -> Self {
        Self {
            pass_states: HashMap::new(),
            debug_mode: 0,
        }
    }
    pub fn is_pass_enabled(&self, name: &str) -> bool {
        // Default to true if not found
        *self.pass_states.get(name).unwrap_or(&true)
    }
}
