//! Selector de apps abiertas. Sin Quickstep (se desactivó para usar Nova), Android no tiene vista
//! de recientes, así que ZYKRO dibuja la suya: una tarjeta por app con la última imagen que vio de
//! ella. Clic para volver a la app, X para cerrarla.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Align2, Color32, ColorImage, FontId, Id, Order, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use egui_phosphor::regular as ic;

use crate::android::{self, Tarea};
use crate::video::Cuadro;
use crate::{ACENTO, CERRAR_HOVER, FONDO, FONDO_CONTROL, FONDO_HOVER, ICONO, TENUE, TEXTO};

/// Ancho de las miniaturas guardadas (el alto sale de la proporción de la pantalla).
const ANCHO_MINIATURA: usize = 384;

#[derive(Default)]
pub struct Recientes {
    abierto: bool,
    /// `None` mientras se pregunta a Android.
    lista: Arc<Mutex<Option<Vec<Tarea>>>>,
    /// Última imagen de cada app (por paquete) y si falta subirla a la GPU.
    miniaturas: HashMap<String, (ColorImage, bool)>,
    texturas: HashMap<String, TextureHandle>,
}

impl Recientes {
    pub fn abierto(&self) -> bool {
        self.abierto
    }

    pub fn alternar(&mut self, ctx: &egui::Context) {
        if self.abierto {
            self.abierto = false;
            return;
        }
        self.abierto = true;
        self.actualizar_lista(ctx);
    }

    fn actualizar_lista(&mut self, ctx: &egui::Context) {
        *self.lista.lock().unwrap() = None;
        let (lista, ctx) = (self.lista.clone(), ctx.clone());
        thread::spawn(move || {
            let tareas = android::tareas_abiertas();
            *lista.lock().unwrap() = Some(tareas);
            ctx.request_repaint();
        });
    }

    /// Guarda una miniatura del cuadro actual para `paquete` (la app que está al frente).
    pub fn guardar_miniatura(&mut self, paquete: &str, cuadro: &Cuadro) {
        if cuadro.ancho() == 0 || cuadro.alto() == 0 {
            return;
        }
        self.miniaturas.insert(paquete.to_string(), (cuadro.miniatura(ANCHO_MINIATURA), true));
    }

    /// Dibuja el selector sobre `area` (la zona del video).
    pub fn mostrar(&mut self, ctx: &egui::Context, area: Rect) {
        if !self.abierto {
            return;
        }
        // Subir a la GPU las miniaturas nuevas.
        for (paquete, (imagen, nueva)) in &mut self.miniaturas {
            if std::mem::take(nueva) {
                match self.texturas.get_mut(paquete) {
                    Some(t) => t.set(imagen.clone(), TextureOptions::LINEAR),
                    None => {
                        let t = ctx.load_texture(format!("miniatura-{paquete}"), imagen.clone(), TextureOptions::LINEAR);
                        self.texturas.insert(paquete.clone(), t);
                    }
                }
            }
        }
        let lista = self.lista.lock().unwrap().clone();
        let mut abrir = None;
        let mut cerrar: Vec<Tarea> = Vec::new();
        let mut salir = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        egui::Area::new(Id::new("recientes")).order(Order::Foreground).fixed_pos(area.min).show(ctx, |ui| {
            let fondo = ui.allocate_rect(Rect::from_min_size(area.min, area.size()), Sense::click());
            ui.painter().rect_filled(area, 0.0, Color32::from_black_alpha(235));
            if fondo.clicked() {
                salir = true;
            }
            let p = ui.painter().clone();
            let titulo = area.min + Vec2::new(28.0, 26.0);
            p.text(titulo, Align2::LEFT_TOP, "Apps abiertas", FontId::proportional(20.0), TEXTO);

            let Some(tareas) = lista else {
                p.text(area.center(), Align2::CENTER_CENTER, "Buscando apps abiertas…", FontId::proportional(14.0), TENUE);
                return;
            };
            if tareas.is_empty() {
                p.text(area.center(), Align2::CENTER_CENTER, "No hay apps abiertas", FontId::proportional(15.0), TENUE);
                return;
            }
            // «Cerrar todas», arriba a la derecha.
            let boton = Rect::from_min_size(Pos2::new(area.max.x - 160.0, area.min.y + 20.0), Vec2::new(132.0, 32.0));
            let todas = ui.allocate_rect(boton, Sense::click());
            let color = if todas.hovered() { CERRAR_HOVER } else { FONDO_CONTROL };
            p.rect_filled(boton, 6.0, color);
            p.text(boton.center(), Align2::CENTER_CENTER, format!("{}  Cerrar todas", ic::X), FontId::proportional(13.0), TEXTO);
            if todas.clicked() {
                cerrar = tareas.clone();
            }

            // Tarjetas en fila, centradas; si no caben, se achican.
            let n = tareas.len() as f32;
            let separacion = 22.0;
            let ancho = ((area.width() - 56.0 - separacion * (n - 1.0)) / n).clamp(120.0, 340.0);
            let alto_imagen = ancho * 9.0 / 16.0;
            let total = ancho * n + separacion * (n - 1.0);
            let x0 = area.center().x - total / 2.0;
            let y0 = area.center().y - (alto_imagen + 40.0) / 2.0;
            for (i, tarea) in tareas.iter().enumerate() {
                let tarjeta = Rect::from_min_size(Pos2::new(x0 + i as f32 * (ancho + separacion), y0), Vec2::new(ancho, alto_imagen + 40.0));
                let imagen = Rect::from_min_size(tarjeta.min + Vec2::new(0.0, 40.0), Vec2::new(ancho, alto_imagen));
                let resp = ui.allocate_rect(tarjeta, Sense::click());
                let encima = resp.hovered();
                // Nombre arriba de la imagen, como en Android.
                let nombre = nombre_app(&tarea.paquete);
                p.text(Pos2::new(tarjeta.min.x + 4.0, tarjeta.min.y + 20.0), Align2::LEFT_CENTER, &nombre, FontId::proportional(14.0), TEXTO);
                p.rect_filled(imagen, 10.0, FONDO);
                match self.texturas.get(&tarea.paquete) {
                    Some(t) => {
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        egui::Image::from_texture(t).corner_radius(10.0).uv(uv).paint_at(ui, imagen);
                    }
                    None => {
                        let inicial = nombre.chars().next().unwrap_or('?').to_uppercase().to_string();
                        p.circle_filled(imagen.center(), alto_imagen * 0.2, FONDO_HOVER);
                        p.text(imagen.center(), Align2::CENTER_CENTER, inicial, FontId::proportional(alto_imagen * 0.2), ICONO);
                    }
                }
                let borde = if encima { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.0, FONDO_HOVER) };
                p.rect_stroke(imagen, 10.0, borde, egui::StrokeKind::Outside);
                // X para cerrar, a la derecha del nombre.
                let x = Rect::from_center_size(Pos2::new(tarjeta.max.x - 14.0, tarjeta.min.y + 20.0), Vec2::splat(28.0));
                let resp_x = ui.allocate_rect(x, Sense::click());
                let color_x = if resp_x.hovered() { Color32::WHITE } else { TENUE };
                if resp_x.hovered() {
                    p.circle_filled(x.center(), 13.0, CERRAR_HOVER);
                }
                p.text(x.center(), Align2::CENTER_CENTER, ic::X, FontId::proportional(15.0), color_x);
                if resp_x.clicked() {
                    cerrar.push(tarea.clone());
                } else if resp.clicked() {
                    abrir = Some(tarea.paquete.clone());
                }
            }
        });

        if !cerrar.is_empty() {
            if let Some(tareas) = self.lista.lock().unwrap().as_mut() {
                tareas.retain(|t| !cerrar.iter().any(|c| c.raiz == t.raiz));
                // Sin apps que mostrar (cerrar todas o la última), el selector se cierra solo.
                salir |= tareas.is_empty();
            }
            thread::spawn(move || cerrar.iter().for_each(|t| android::cerrar_tarea(t.raiz)));
        }
        if let Some(paquete) = abrir {
            thread::spawn(move || android::abrir_app(&paquete));
            salir = true;
        }
        if salir {
            self.abierto = false;
        }
    }
}

/// Nombre para mostrar. Android no da las etiquetas de las apps por la terminal, así que se usa una
/// tabla con las conocidas y, si no, la última parte del paquete.
fn nombre_app(paquete: &str) -> String {
    let conocidas = [
        ("com.dts.freefireth", "Free Fire"),
        ("com.dts.freefiremax", "Free Fire MAX"),
        ("org.cromite.cromite", "Cromite"),
        ("com.android.chrome", "Chrome"),
        ("com.android.settings", "Ajustes"),
        ("com.aurora.store", "Aurora Store"),
        ("com.teslacoilsw.launcher", "Nova Launcher"),
        ("com.android.documentsui", "Archivos"),
        ("com.teksavia.climint", "Climint"),
        ("com.google.android.youtube", "YouTube"),
        ("com.whatsapp", "WhatsApp"),
    ];
    if let Some((_, nombre)) = conocidas.iter().find(|(p, _)| *p == paquete) {
        return nombre.to_string();
    }
    let parte = paquete.rsplit('.').find(|p| !["android", "app", "apps", "mobile", "client"].contains(p)).unwrap_or(paquete);
    let mut letras = parte.chars();
    letras.next().map(|c| c.to_uppercase().chain(letras).collect()).unwrap_or_default()
}
