use crate::model::Model;
use crate::{context::GraphicsContext, scenes::Scene};
use std::collections::{HashMap, HashSet};

pub struct UiState {
    pub ctx: egui::Context,
    pub winit_state: egui_winit::State,
    pub renderer: egui_wgpu::Renderer,
}

impl UiState {
    pub fn new(context: &GraphicsContext) -> Self {
        let ctx = egui::Context::default();

        let winit_state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &context.window,
            Some(context.window.scale_factor() as f32),
            None,
            None,
        );

        let renderer = egui_wgpu::Renderer::new(
            &context.device,
            context.config.format,
            egui_wgpu::RendererOptions::default(),
        );

        Self {
            ctx,
            winit_state,
            renderer,
        }
    }

    // Add this inside `impl UiState`
    pub fn draw(
        &mut self,
        context: &crate::context::GraphicsContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        run_ui: impl FnMut(&mut egui::Ui),
    ) {
        // Gather input and generate UI
        let raw_input = self.winit_state.take_egui_input(&context.window);
        let full_output = self.ctx.run_ui(raw_input, run_ui);

        // Handle OS actions (like copying to clipboard) and tessellate geometry
        self.winit_state
            .handle_platform_output(&context.window, full_output.platform_output);
        let clipped_primitives = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        // Upload new textures to the GPU
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&context.device, &context.queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [context.config.width, context.config.height],
            pixels_per_point: context.window.scale_factor() as f32,
        };

        self.renderer.update_buffers(
            &context.device,
            &context.queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        // Draw over the existing scene
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            self.renderer.render(
                &mut render_pass.forget_lifetime(),
                &clipped_primitives,
                &screen_descriptor,
            );
        }

        // 5. Cleanup old textures
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}

pub struct AppUi {
    pub open_windows: HashSet<String>,
}

impl AppUi {
    pub fn new() -> Self {
        Self {
            open_windows: HashSet::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, scene: &dyn Scene, models: &HashMap<String, Model>) {
        egui::Window::new("Viewer settings").show(ui.ctx(), |window_ui| {
            window_ui.heading("Render options");
            window_ui.label(format!(
                "Camera position: x {} y {} z {}",
                scene.camera().position.x.round(),
                scene.camera().position.y.round(),
                scene.camera().position.z.round(),
            ));
            window_ui.checkbox(&mut false, "Display depthmap");
            window_ui.heading("Models inspector");
            for model_name in models.keys() {
                if window_ui.button(model_name).clicked() {
                    if self.open_windows.contains(model_name) {
                        self.open_windows.remove(model_name);
                    } else {
                        self.open_windows.insert(model_name.clone());
                    }
                }
            }
        });

        let mut closed_windows = Vec::new();

        for model_name in self.open_windows.iter() {
            let mut is_open = true;

            egui::Window::new(model_name)
                .open(&mut is_open)
                .show(ui.ctx(), |_window_ui| {
                    let model = models.get(model_name).expect("Model not found");

                    _window_ui.separator();

                    egui::CollapsingHeader::new(format!("Meshes ({})", model.meshes.len()))
                        .default_open(true)
                        .show(_window_ui, |ui| {
                            for mesh in model.meshes.iter() {
                                egui::CollapsingHeader::new(&mesh.name).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("Elements:");
                                        ui.label(mesh.num_elements.to_string());
                                    });
                                    ui.horizontal(|ui| {
                                        ui.strong("Material index:");
                                        ui.label(mesh.material.to_string());
                                    });
                                });
                            }
                        });

                    _window_ui.separator();

                    egui::CollapsingHeader::new(format!("Materials ({})", model.materials.len()))
                        .default_open(true)
                        .show(_window_ui, |ui| {
                            for mat in model.materials.iter() {
                                egui::CollapsingHeader::new(&mat.name).show(ui, |ui| {});
                            }
                        });
                });

            if !is_open {
                closed_windows.push(model_name.clone());
            }
        }

        for closed in closed_windows {
            self.open_windows.remove(&closed);
        }
    }
}
