use crate::camera::{Camera, CameraController};
use crate::input::Input;
use crate::model::Model;
use crate::resources;
use crate::scenes::{Instance, Light, RenderDebug, Scene};
use cgmath::{Quaternion, Rotation3, Vector3, Zero};
use std::time::Duration;

pub struct DefaultScene {
    camera: Camera,
    camera_controller: CameraController,
    instances: Vec<Instance>,
    lights: Vec<Light>,
    time: f32,
}

impl DefaultScene {
    pub fn new() -> Self {
        let camera = Camera::new((0.0, 2.0, 5.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));

        let camera_controller = CameraController::new(4.0, 0.4);

        let instances = vec![Instance {
            model_name: "scene.gltf".to_string(),
            position: Vector3::zero(),
            rotation: Quaternion::from_axis_angle(Vector3::unit_z(), cgmath::Deg(0.0)),
        }];

        let lights = vec![Light {
            position: Vector3::new(2.0, 2.0, 2.0),
            color: Vector3::new(1.0, 1.0, 1.0),
        }];

        Self {
            camera,
            camera_controller,
            instances,
            lights,
            time: 0.0,
        }
    }
}

impl Scene for DefaultScene {
    fn update(&mut self, dt: Duration, input: &mut Input) {
        self.camera_controller
            .update_camera(&mut self.camera, dt, input);

        self.time += dt.as_secs_f32();

        // Animate light
        if !self.lights.is_empty() {
            let old_position = self.lights[0].position;
            self.lights[0].position = cgmath::Quaternion::from_axis_angle(
                Vector3::new(0.0, 1.0, 0.0),
                cgmath::Deg(60.0 * dt.as_secs_f32()),
            ) * old_position;
        }
    }

    fn debug_nodes(&self) -> Vec<&dyn RenderDebug> {
        let mut nodes: Vec<&dyn RenderDebug> = Vec::new();

        // Add all instances
        for instance in &self.instances {
            nodes.push(instance as &dyn RenderDebug);
        }

        // Add all lights
        for light in &self.lights {
            nodes.push(light as &dyn RenderDebug);
        }

        nodes
    }

    fn camera(&self) -> &Camera {
        &self.camera
    }

    fn required_models(&self) -> Vec<&str> {
        vec!["scene.gltf"]
    }

    fn instances(&self) -> &[Instance] {
        &self.instances
    }

    fn lights(&self) -> &[Light] {
        &self.lights
    }

    fn skybox_path(&self) -> Option<&str> {
        Some("pure-sky.hdr")
    }
}
