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
    pub model_name: String,
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
            model_name: "models/OBJ/camera/camera.obj".to_string(),
            position: Vector3::zero(),
            rotation: Quaternion::from_axis_angle(Vector3::unit_z(), cgmath::Deg(0.0)),
        }];

        let lights = vec![Light {
            position: Vector3::new(2.0, 2.0, 2.0),
            color: Vector3::new(300.0, 300.0, 300.0),
        }];

        Self {
            camera,
            camera_controller,
            instances,
            lights,
            skybox_path: "pure-sky.hdr".to_string(),
            time: 0.0,
        }
    }

    pub fn update(&mut self, dt: Duration, input: &mut Input, animate_light: bool) {
        self.camera_controller
            .update_camera(&mut self.camera, dt, input);

        self.time += dt.as_secs_f32();

        // Animate light
        if animate_light && !self.lights.is_empty() {
            let old_position = self.lights[0].position;
            self.lights[0].position = cgmath::Quaternion::from_axis_angle(
                Vector3::new(0.0, 1.0, 0.0),
                cgmath::Deg(60.0 * dt.as_secs_f32()),
            ) * old_position;
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
