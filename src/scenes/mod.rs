use crate::camera::Camera;
use crate::input::Input;
use crate::model::Model;
use cgmath::{Quaternion, Vector3};
use std::time::Duration;

pub mod default_scene;

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

pub trait Scene {
    fn update(&mut self, dt: Duration, input: &mut Input);
    fn camera(&self) -> &Camera;
    fn required_models(&self) -> Vec<&str>;
    fn debug_nodes(&self) -> Vec<&dyn RenderDebug> {
        vec![]
    }
    fn instances(&self) -> &[Instance];
    fn lights(&self) -> &[Light];
    fn skybox_path(&self) -> Option<&str> {
        None
    }
}
