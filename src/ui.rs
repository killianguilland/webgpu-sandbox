use crate::model::Model;
use crate::{context::GraphicsContext, viewer::ModelViewer};
use std::collections::HashMap;

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
    is_view_panel_open: bool,
    is_model_panel_open: bool,
    example_models: Vec<String>,
}

impl AppUi {
    pub fn new() -> Self {
        let mut example_models = Vec::new();
        if let Ok(entries) = std::fs::read_dir("res/gltf-models") {
            for entry in entries.filter_map(Result::ok) {
                if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let gltf_path = format!("gltf-models/{}/glTF/{}.gltf", name, name);
                    let glb_path = format!("gltf-models/{}/glTF-Binary/{}.glb", name, name);

                    if std::path::Path::new(&format!("res/{}", gltf_path)).exists() {
                        example_models.push(gltf_path);
                    } else if std::path::Path::new(&format!("res/{}", glb_path)).exists() {
                        example_models.push(glb_path);
                    }
                }
            }
        }
        example_models.sort();

        Self {
            is_model_panel_open: false,
            is_view_panel_open: false,
            example_models,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        viewer: &ModelViewer,
        models: &HashMap<String, Model>,
        settings: &mut crate::settings::RenderSettings,
    ) -> Option<String> {
        let mut load_path = None;
        egui::Panel::top("wrap_app_top_bar")
            // .frame(egui::Frame::new().inner_margin(4))
            .show_inside(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.visuals_mut().button_frame = false;

                    ui.menu_button("File", |ui| {
                        ui.menu_button("Open Example", |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(400.0)
                                .show(ui, |ui| {
                                    for path in &self.example_models {
                                        let label = std::path::Path::new(path)
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("Unknown Model");
                                        if ui.button(label).clicked() {
                                            load_path = Some(path.clone());
                                            ui.close();
                                        }
                                    }
                                });
                        });
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });

                    if ui
                        .selectable_label(self.is_view_panel_open, "View")
                        .clicked()
                    {
                        self.is_view_panel_open = !self.is_view_panel_open;
                    }

                    if ui
                        .selectable_label(self.is_model_panel_open, "Model")
                        .clicked()
                    {
                        self.is_model_panel_open = !self.is_model_panel_open;
                    }
                });
            });

        egui::Panel::bottom("Bottom data").show_inside(ui, |window_ui| {
            window_ui.horizontal_wrapped(|hz_ui| {
                hz_ui.add_space(6.0);
                egui::widgets::global_theme_preference_switch(hz_ui);
                hz_ui.separator();
                hz_ui.label(format!(
                    "Camera position: ({}, {}, {})",
                    viewer.camera.position.x.round(),
                    viewer.camera.position.y.round(),
                    viewer.camera.position.z.round(),
                ));
            });
        });
        if self.is_view_panel_open {
            egui::Panel::left("View").show_inside(ui, |window_ui| {
                egui::ScrollArea::vertical().show(window_ui, |window_ui| {
                    window_ui.heading("Render options");
                    for (pass_name, is_enabled) in settings.pass_states.iter_mut() {
                        window_ui.checkbox(is_enabled, format!("{} pass", pass_name));
                    }

                    window_ui.separator();

                    window_ui.heading("Lighting");
                    window_ui.horizontal(|ui| {
                        ui.label("Position");
                        ui.add(
                            egui::DragValue::new(&mut settings.light_position[0])
                                .speed(0.1)
                                .prefix("X: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut settings.light_position[1])
                                .speed(0.1)
                                .prefix("Y: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut settings.light_position[2])
                                .speed(0.1)
                                .prefix("Z: "),
                        );
                    });
                    window_ui.horizontal(|ui| {
                        ui.add(
                            egui::Slider::new(&mut settings.light_intensity, 0.0..=500.0)
                                .text("Intensity"),
                        );
                        ui.color_edit_button_rgb(&mut settings.light_color);
                    });
                    window_ui.add(
                        egui::Slider::new(&mut settings.ambient_intensity, 0.0..=0.2)
                            .text("Ambient"),
                    );

                    window_ui.separator();

                    window_ui.heading("Camera");
                    window_ui
                        .add(egui::Slider::new(&mut settings.camera_fov, 30.0..=120.0).text("FOV"));
                    window_ui.add(
                        egui::Slider::new(&mut settings.hdr_exposure, 0.1..=10.0).text("Exposure"),
                    );

                    window_ui.separator();

                    window_ui.heading("SSAO Settings");
                    window_ui.horizontal(|ui| {
                        ui.label("Samples");
                        ui.radio_value(&mut settings.ssao_kernel_size, 8, "8");
                        ui.radio_value(&mut settings.ssao_kernel_size, 16, "16");
                        ui.radio_value(&mut settings.ssao_kernel_size, 32, "32");
                        ui.radio_value(&mut settings.ssao_kernel_size, 64, "64");
                    });
                    window_ui.add(
                        egui::Slider::new(&mut settings.ssao_radius, 0.01..=1.0).text("Radius"),
                    );
                    window_ui.add(
                        egui::Slider::new(&mut settings.ssao_bias, 0.0..=0.1).text("Acne Bias"),
                    );
                    window_ui
                        .add(egui::Slider::new(&mut settings.ssao_power, 0.5..=5.0).text("Power"));

                    window_ui.separator();

                    window_ui.heading("G-Buffer visualizer");
                    window_ui.radio_value(&mut settings.debug_mode, 0, "None");
                    window_ui.radio_value(&mut settings.debug_mode, 1, "Albedo");
                    window_ui.radio_value(&mut settings.debug_mode, 2, "Normal");
                    window_ui.radio_value(&mut settings.debug_mode, 3, "Metal/Rough");
                    window_ui.radio_value(&mut settings.debug_mode, 4, "Depth");
                    window_ui.radio_value(&mut settings.debug_mode, 5, "SSAO");
                });
            });
        }

        if self.is_model_panel_open {
            egui::Panel::right("Model settings").show_inside(ui, |window_ui| {
                egui::ScrollArea::vertical().show(window_ui, |window_ui| {
                    if let Some(model) = models.get(&viewer.model_name) {
                        egui::CollapsingHeader::new(format!("Meshes ({})", model.meshes.len()))
                            .default_open(true)
                            .show(window_ui, |ui| {
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

                        window_ui.separator();

                        egui::CollapsingHeader::new(format!(
                            "Materials ({})",
                            model.materials.len()
                        ))
                        .default_open(true)
                        .show(window_ui, |ui| {
                            for mat in model.materials.iter() {
                                egui::CollapsingHeader::new(&mat.name).show(ui, |ui| {
                                    if mat.is_transparent {
                                        ui.label("Transparent");
                                    } else {
                                        ui.label("Opaque");
                                    }
                                    ui.label(format!(
                                        "Base color ({}, {}, {}, {})",
                                        mat.uniforms.base_color_factor[0],
                                        mat.uniforms.base_color_factor[1],
                                        mat.uniforms.base_color_factor[2],
                                        mat.uniforms.base_color_factor[3]
                                    ));
                                });
                            }
                        });
                    } else {
                        window_ui.label("Loading model...");
                    }
                });
            });
        }

        load_path
    }
}
