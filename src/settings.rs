use std::collections::HashMap;
pub struct RenderSettings {
    // Maps a pass name to its enabled/disabled state
    pub pass_states: HashMap<String, bool>,
    pub show_depthmap: bool,
}
impl RenderSettings {
    pub fn new() -> Self {
        Self {
            pass_states: HashMap::new(),
            show_depthmap: false,
        }
    }
    pub fn is_pass_enabled(&self, name: &str) -> bool {
        // Default to true if not found
        *self.pass_states.get(name).unwrap_or(&true)
    }
}
