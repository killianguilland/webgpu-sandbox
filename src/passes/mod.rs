pub mod blurpass;
pub mod clearpass;
pub mod geometrypass;
pub mod gizmopass;
pub mod lightingpass;
pub mod skyboxpass;
pub mod ssaopass;
pub mod transparentpass;

pub use blurpass::BlurPass;
pub use clearpass::ClearPass;
pub use geometrypass::GeometryPass;
pub use gizmopass::GizmoPass;
pub use lightingpass::LightingPass;
pub use skyboxpass::SkyboxPass;
pub use ssaopass::SsaoPass;
pub use transparentpass::TransparentPass;

use crate::context;
use crate::graphics;

pub trait RenderPass {
    fn name(&self) -> &str;
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        gbuffer: &graphics::gbuffer::GBuffer,
        viewer: &graphics::viewer::ModelViewer,
        resources: &graphics::resources::ResourceManager,
        context: &context::GraphicsContext,
        renderer: &graphics::renderer::Renderer,
        settings: &crate::settings::RenderSettings,
    );
}
