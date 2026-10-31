//! Confirmación al cerrar ZYKRO (con la X, Alt+F4 o desde la barra de tareas).

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use egui_phosphor::regular as ic;

use crate::{ACENTO, CERRAR_HOVER, FONDO, FONDO_HOVER, TENUE, TEXTO};

/// Lo que eligió el usuario en el diálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Respuesta {
    Cerrar,
    Reiniciar,
    Cancelar,
}

/// Dibuja el diálogo. Al cerrar ZYKRO también se apaga Android, y se avisa.
pub fn mostrar(ctx: &egui::Context) -> Option<Respuesta> {
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
        ui.label(RichText::new("Android también se apagará.").size(12.5).color(TENUE));
        ui.label(RichText::new("Reiniciar vuelve a abrir ZYKRO y Android.").size(12.5).color(TENUE));
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let cerrar = egui::Button::new(RichText::new("Cerrar").color(Color32::WHITE).strong())
                .fill(CERRAR_HOVER)
                .min_size(Vec2::new(96.0, 30.0));
            if ui.add(cerrar).clicked() {
                respuesta = Some(Respuesta::Cerrar);
            }
            let reiniciar = egui::Button::new(RichText::new("Reiniciar").color(Color32::WHITE).strong())
                .fill(ACENTO)
                .min_size(Vec2::new(96.0, 30.0));
            if ui.add(reiniciar).clicked() {
                respuesta = Some(Respuesta::Reiniciar);
            }
            if ui.add(egui::Button::new("Cancelar").min_size(Vec2::new(96.0, 30.0))).clicked() {
                respuesta = Some(Respuesta::Cancelar);
            }
        });
    });
    // Esc o un clic fuera cancelan; Enter confirma.
    resolver(respuesta, modal.should_close(), ctx.input(|i| i.key_pressed(egui::Key::Enter)))
}

fn resolver(elegida: Option<Respuesta>, cancelar: bool, enter: bool) -> Option<Respuesta> {
    if elegida.is_some() {
        elegida
    } else if cancelar {
        Some(Respuesta::Cancelar)
    } else if enter {
        Some(Respuesta::Cerrar)
    } else {
        None
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn respeta_botones_y_cancelacion() {
        for accion in [Respuesta::Cerrar, Respuesta::Reiniciar, Respuesta::Cancelar] {
            assert_eq!(resolver(Some(accion), true, true), Some(accion));
        }
        assert_eq!(resolver(None, true, true), Some(Respuesta::Cancelar));
        assert_eq!(resolver(None, false, true), Some(Respuesta::Cerrar));
        assert_eq!(resolver(None, false, false), None);
    }

    #[test]
    fn botones_del_dialogo_se_pueden_pulsar() {
        for (etiqueta, esperada) in [("Cancelar", Respuesta::Cancelar), ("Reiniciar", Respuesta::Reiniciar), ("Cerrar", Respuesta::Cerrar)] {
            let ctx = egui::Context::default();
            let entrada = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 600.0))),
                ..Default::default()
            };
            ctx.run_ui(entrada(), |ui| { mostrar(ui.ctx()); }).drop_without_applying_deltas();
            let salida = ctx.run_ui(entrada(), |ui| { mostrar(ui.ctx()); });
            let centro = salida.shapes.iter().find_map(|forma| match &forma.shape {
                egui::Shape::Text(texto) if texto.galley.job.text == etiqueta => Some(texto.pos + texto.galley.size() * 0.5),
                _ => None,
            }).unwrap();
            salida.drop_without_applying_deltas();
            let mut clic = entrada();
            clic.events.push(egui::Event::PointerMoved(centro));
            for pressed in [true, false] {
                clic.events.push(egui::Event::PointerButton { pos: centro, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            }
            let mut respuesta = None;
            ctx.run_ui(clic, |ui| { respuesta = mostrar(ui.ctx()); }).drop_without_applying_deltas();
            assert_eq!(respuesta, Some(esperada));
        }
    }
}
