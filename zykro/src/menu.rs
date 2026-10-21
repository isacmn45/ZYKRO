//! Menú rápido de los controles del juego, como el de BlueStacks 5: se abre con el botón del
//! mapeador y deja cambiar la sensibilidad de la puntería sin abrir el editor.

use eframe::egui::{self, Align, Align2, Id, Layout, Order, Pos2, Rect, RichText, Stroke, Vec2};
use egui_phosphor::regular as ic;

use crate::mapeo::Mapa;
use crate::{ACENTO, FONDO, FONDO_CONTROL, FONDO_HOVER, TENUE, TEXTO};

pub enum Salida {
    Nada,
    /// Abrir el editor completo.
    Editor,
    Cerrar,
}

#[derive(Default)]
pub struct Menu {
    /// Botón que lo abrió (`None` = cerrado) y si el menú sale hacia abajo o hacia la izquierda.
    boton: Option<(Rect, bool)>,
    /// Hubo cambios que guardar al cerrar.
    cambiado: bool,
}

impl Menu {
    pub fn abierto(&self) -> bool {
        self.boton.is_some()
    }

    /// `abajo`: el botón está en la barra de título; si no, en la barra lateral.
    pub fn abrir(&mut self, boton: Rect, abajo: bool) {
        self.boton = Some((boton, abajo));
        self.cambiado = false;
    }

    /// Cierra el menú y devuelve si hay cambios que guardar.
    pub fn cerrar(&mut self) -> bool {
        self.boton = None;
        std::mem::take(&mut self.cambiado)
    }

    /// Dibuja el menú. `mapa` es el del motor: los cambios se aplican al momento.
    pub fn mostrar(&mut self, ctx: &egui::Context, mapa: Option<&mut Mapa>) -> Salida {
        let Some((boton, abajo)) = self.boton else { return Salida::Nada };
        let mut salida = Salida::Nada;
        let (pivote, pos) = if abajo {
            (Align2::RIGHT_TOP, boton.right_bottom() + Vec2::new(0.0, 4.0))
        } else {
            (Align2::RIGHT_TOP, boton.left_top() - Vec2::new(4.0, 0.0))
        };
        let marco = egui::Frame::new()
            .fill(FONDO)
            .corner_radius(8.0)
            .stroke(Stroke::new(1.0, FONDO_HOVER))
            .shadow(ctx.global_style().visuals.popup_shadow)
            .inner_margin(egui::Margin::same(12));
        let area = egui::Area::new(Id::new("menu_controles")).order(Order::Foreground).pivot(pivote).fixed_pos(pos).show(ctx, |ui| {
            marco.show(ui, |ui| {
                ui.set_width(250.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ic::GAME_CONTROLLER).size(16.0).color(ACENTO));
                    ui.label(RichText::new("Controles del juego").size(14.0).strong().color(TEXTO));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if crate::icono(ui, ic::X, "Cerrar", Vec2::new(26.0, 22.0), crate::CERRAR_HOVER).clicked() {
                            salida = Salida::Cerrar;
                        }
                    });
                });
                match mapa {
                    Some(Mapa { raton: Some(r), .. }) => {
                        ui.label(RichText::new("SENSIBILIDAD DE LA PUNTERÍA").size(10.5).strong().color(TENUE));
                        self.cambiado |= crate::editor::sensibilidad(ui, r);
                    }
                    Some(_) => {
                        ui.label(RichText::new("Este juego no tiene control de puntería.").color(TENUE));
                    }
                    None => {
                        ui.label(RichText::new("Este juego todavía no tiene controles.").color(TENUE));
                    }
                }
                let editar = egui::Button::new(RichText::new(format!("{}  Editar controles", ic::PENCIL_SIMPLE)).color(TEXTO))
                    .fill(FONDO_CONTROL)
                    .min_size(Vec2::new(ui.available_width(), 30.0));
                if ui.add(editar).clicked() {
                    salida = Salida::Editor;
                }
            });
        });
        // Se cierra con Esc o con un clic fuera (el clic en su botón lo maneja el botón).
        let dentro = area.response.rect;
        let clic_fuera = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p: Pos2| !dentro.contains(p) && !boton.contains(p))
        });
        if matches!(salida, Salida::Nada) && (clic_fuera || ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
            salida = Salida::Cerrar;
        }
        salida
    }
}
