use crate::camera::{Camera, CameraController};
use crate::input::Input;
use cgmath::{Quaternion, Rotation3, Vector3, Zero};
use std::time::Duration;

pub trait RenderDebug {
    fn position(&self) -> cgmath::Vector3<f32>;

    fn rotation(&self) -> Option<cgmath::Quaternion<f32>> {
        None
    }

    fn get_transform(&self) -> cgmath::Matrix4<f32> {
        let translation = cgmath::Matrix4::from_translation(self.position());

        if let Some(rot) = self.rotation() {
            translation * cgmath::Matrix4::from(rot)
        } else {
            translation
        }
    }
}

pub struct Instance {
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
}

impl RenderDebug for Instance {
    fn position(&self) -> cgmath::Vector3<f32> {
        self.position
    }

    fn rotation(&self) -> Option<cgmath::Quaternion<f32>> {
        Some(self.rotation)
    }
}

pub struct Light {
    pub position: Vector3<f32>,
    pub color: Vector3<f32>,
}

impl RenderDebug for Light {
    fn position(&self) -> cgmath::Vector3<f32> {
        self.position
    }
}

pub struct ModelViewer {
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub model_name: String,
    pub instances: Vec<Instance>,
    pub lights: Vec<Light>,
    pub skybox_path: String,
    pub time: f32,
}

impl ModelViewer {
    pub fn new() -> Self {
        let camera = Camera::new((0.0, 1.0, 2.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let camera_controller = CameraController::new(4.0, 0.4);

        let instances = vec![Instance {
            position: Vector3::zero(),
            rotation: Quaternion::from_axis_angle(Vector3::unit_z(), cgmath::Deg(0.0)),
        }];

        let lights = vec![Light {
            position: Vector3::new(2.0, 2.0, 2.0),
            color: Vector3::new(75.0, 75.0, 75.0),
        }];

        Self {
            camera,
            camera_controller,
            model_name: "models/OBJ/camera/camera.obj".to_string(),
            instances,
            lights,
            skybox_path: "pure-sky.hdr".to_string(),
            time: 0.0,
        }
    }

    pub fn update(&mut self, dt: Duration, input: &mut Input, settings: &crate::settings::RenderSettings) {
        self.camera_controller
            .update_camera(&mut self.camera, dt, input);

        self.time += dt.as_secs_f32();

        // Sync light from settings
        if !self.lights.is_empty() {
            self.lights[0].position = cgmath::Vector3::new(
                settings.light_position[0],
                settings.light_position[1],
                settings.light_position[2],
            );
            self.lights[0].color = cgmath::Vector3::new(
                settings.light_color[0] * settings.light_intensity,
                settings.light_color[1] * settings.light_intensity,
                settings.light_color[2] * settings.light_intensity,
            );
        }
    }

    pub fn gizmo_nodes(&self) -> Vec<&dyn RenderDebug> {
        let mut nodes: Vec<&dyn RenderDebug> = Vec::new();

        for instance in &self.instances {
            nodes.push(instance as &dyn RenderDebug);
        }

        for light in &self.lights {
            nodes.push(light as &dyn RenderDebug);
        }

        nodes
    }
}
