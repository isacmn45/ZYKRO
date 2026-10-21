//! Confirmación al cerrar ZYKRO (con la X, Alt+F4 o desde la barra de tareas).

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use egui_phosphor::regular as ic;

use crate::{CERRAR_HOVER, FONDO, FONDO_HOVER, TENUE, TEXTO};

/// Lo que eligió el usuario en el diálogo.
pub enum Respuesta {
    Cerrar,
    Cancelar,
}

/// Dibuja el diálogo. `apaga_android`: la configuración apaga Android al cerrar (se avisa).
pub fn mostrar(ctx: &egui::Context, apaga_android: bool) -> Option<Respuesta> {
    let mut respuesta = None;
    let marco = egui::Frame::new()
        .fill(FONDO)
        .corner_radius(8.0)
        .stroke(Stroke::new(1.0, FONDO_HOVER))
        .inner_margin(egui::Margin::same(20));
    let modal = egui::Modal::new(egui::Id::new("confirmar_cierre")).frame(marco).show(ctx, |ui| {
        ui.set_width(340.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.horizontal(|ui| {
            ui.label(RichText::new(ic::POWER).size(18.0).color(CERRAR_HOVER));
            ui.label(RichText::new("¿Cerrar ZYKRO?").size(15.5).strong().color(TEXTO));
        });
        ui.label(RichText::new("¿Seguro que quieres cerrar ZYKRO?").size(13.5).color(TEXTO));
        if apaga_android {
            ui.label(RichText::new("Android también se apagará.").size(12.5).color(TENUE));
        }
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let cerrar = egui::Button::new(RichText::new("Cerrar").color(Color32::WHITE).strong())
                .fill(CERRAR_HOVER)
                .min_size(Vec2::new(96.0, 30.0));
            if ui.add(cerrar).clicked() {
                respuesta = Some(Respuesta::Cerrar);
            }
            if ui.add(egui::Button::new("Cancelar").min_size(Vec2::new(96.0, 30.0))).clicked() {
                respuesta = Some(Respuesta::Cancelar);
            }
        });
    });
    // Esc o un clic fuera cancelan; Enter confirma.
    if respuesta.is_none() && modal.should_close() {
        respuesta = Some(Respuesta::Cancelar);
    }
    if respuesta.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        respuesta = Some(Respuesta::Cerrar);
    }
    respuesta
}
