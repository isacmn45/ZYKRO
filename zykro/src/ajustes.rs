//! Ventana de configuración, al estilo de LDPlayer: pestañas a la izquierda y botones abajo.

use eframe::egui::{self, Align, Align2, FontId, Layout, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as ic;

use crate::config::Config;
use crate::{ACENTO, FONDO, FONDO_HOVER, ICONO, NOMBRE, TENUE, TEXTO, VERSION};

#[derive(Clone, Copy, PartialEq)]
enum Pestana {
    General,
    Pantalla,
    Rendimiento,
    Acerca,
}

const PESTANAS: [(Pestana, &str, &str); 4] = [
    (Pestana::General, ic::SLIDERS_HORIZONTAL, "General"),
    (Pestana::Pantalla, ic::MONITOR, "Pantalla"),
    (Pestana::Rendimiento, ic::LIGHTNING, "Rendimiento"),
    (Pestana::Acerca, ic::INFO, "Acerca de"),
];

const RESOLUCIONES: [(u32, u32, &str); 6] = [
    (1280, 720, "1280 × 720 (HD)"),
    (1600, 900, "1600 × 900"),
    (1920, 1080, "1920 × 1080 (Full HD)"),
    (2560, 1440, "2560 × 1440 (2K)"),
    (720, 1280, "720 × 1280 (teléfono)"),
    (1080, 1920, "1080 × 1920 (teléfono)"),
];
const DPIS: [(u32, &str); 4] = [(160, "160 (pequeño)"), (240, "240 (normal)"), (320, "320 (grande)"), (480, "480 (muy grande)")];
const FPS: [u32; 4] = [30, 60, 90, 120];

pub struct Ajustes {
    pub abierto: bool,
    pestana: Pestana,
    borrador: Config,
    original: Config,
}

impl Ajustes {
    pub fn new() -> Self {
        Ajustes { abierto: false, pestana: Pestana::General, borrador: Config::default(), original: Config::default() }
    }

    pub fn abrir(&mut self, actual: &Config) {
        self.borrador = actual.clone();
        self.original = actual.clone();
        self.abierto = true;
    }

    /// Dibuja la ventana si está abierta. Devuelve la configuración nueva al presionar Guardar.
    pub fn mostrar(&mut self, ctx: &egui::Context) -> Option<Config> {
        if !self.abierto {
            return None;
        }
        let mut guardar = None;
        let mut cerrar = false;
        let marco = egui::Frame::new().fill(FONDO).corner_radius(8.0).stroke(Stroke::new(1.0, FONDO_HOVER));
        let respuesta = egui::Modal::new(egui::Id::new("ajustes")).frame(marco).show(ctx, |ui| {
            ui.set_width(660.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

            // Cabecera
            ui.horizontal(|ui| {
                ui.set_height(40.0);
                ui.add_space(16.0);
                ui.label(RichText::new(ic::GEAR).size(16.0).color(ACENTO));
                ui.label(RichText::new("Configuración").size(15.0).strong().color(TEXTO));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(6.0);
                    if crate::icono(ui, ic::X, "Cerrar", Vec2::new(34.0, 30.0), crate::CERRAR_HOVER).clicked() {
                        cerrar = true;
                    }
                });
            });
            linea(ui);

            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // Pestañas
                ui.vertical(|ui| {
                    ui.set_width(160.0);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (p, glifo, nombre) in PESTANAS {
                        if pestana(ui, self.pestana == p, glifo, nombre).clicked() {
                            self.pestana = p;
                        }
                    }
                });
                // Contenido
                ui.vertical(|ui| {
                    ui.set_width(480.0);
                    ui.set_min_height(300.0);
                    ui.spacing_mut().item_spacing.y = 12.0;
                    ui.add_space(6.0);
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 4)).show(ui, |ui| match self.pestana {
                        Pestana::General => self.general(ui),
                        Pestana::Pantalla => self.pantalla(ui),
                        Pestana::Rendimiento => self.rendimiento(ui),
                        Pestana::Acerca => acerca(ui),
                    });
                });
            });

            linea(ui);
            ui.horizontal(|ui| {
                ui.set_height(44.0);
                ui.add_space(16.0);
                if self.borrador.android != self.original.android {
                    ui.label(RichText::new(format!("{}  Al guardar se reiniciará Android", ic::ARROWS_CLOCKWISE)).size(12.5).color(ACENTO));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(16.0);
                    let boton = egui::Button::new(RichText::new("Guardar").color(FONDO).strong())
                        .fill(ACENTO)
                        .min_size(Vec2::new(96.0, 30.0));
                    if ui.add(boton).clicked() {
                        guardar = Some(self.borrador.clone());
                        cerrar = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(Vec2::new(96.0, 30.0))).clicked() {
                        cerrar = true;
                    }
                });
            });
        });
        if respuesta.should_close() {
            cerrar = true;
        }
        if cerrar {
            self.abierto = false;
        }
        guardar
    }

    fn general(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "General");
        ui.checkbox(&mut self.borrador.apagar_al_cerrar, "Apagar Android al cerrar ZYKRO");
        ui.checkbox(&mut self.borrador.mostrar_fps, "Mostrar FPS en la barra de título");
        ui.checkbox(&mut self.borrador.ocultar_teclado, "Ocultar el teclado en pantalla de Android");
        nota(ui, "Con el teclado en pantalla oculto, escribe con el teclado de la laptop.");
    }

    fn pantalla(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Pantalla");
        let a = &mut self.borrador.android;
        fila(ui, "Resolución", |ui| {
            let actual = RESOLUCIONES
                .iter()
                .find(|r| r.0 == a.ancho && r.1 == a.alto)
                .map(|r| r.2.to_string())
                .unwrap_or_else(|| format!("{} × {}", a.ancho, a.alto));
            egui::ComboBox::from_id_salt("resolucion").selected_text(actual).width(220.0).show_ui(ui, |ui| {
                for (w, h, nombre) in RESOLUCIONES {
                    if ui.selectable_label(a.ancho == w && a.alto == h, nombre).clicked() {
                        a.ancho = w;
                        a.alto = h;
                    }
                }
            });
        });
        fila(ui, "DPI", |ui| {
            let actual = DPIS.iter().find(|d| d.0 == a.dpi).map(|d| d.1.to_string()).unwrap_or(a.dpi.to_string());
            egui::ComboBox::from_id_salt("dpi").selected_text(actual).width(220.0).show_ui(ui, |ui| {
                for (dpi, nombre) in DPIS {
                    ui.selectable_value(&mut a.dpi, dpi, nombre);
                }
            });
        });
        fila(ui, "FPS máximos", |ui| {
            egui::ComboBox::from_id_salt("fps").selected_text(format!("{} FPS", a.fps)).width(220.0).show_ui(ui, |ui| {
                for fps in FPS {
                    ui.selectable_value(&mut a.fps, fps, format!("{fps} FPS"));
                }
            });
        });
        nota(ui, "Se aplica reiniciando Android. Tus apps y datos no se borran.");
    }

    fn rendimiento(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Rendimiento");
        let a = &mut self.borrador.android;
        ui.checkbox(&mut a.lod_activado, "Simplificar texturas (LOD bias)");
        ui.add_enabled_ui(a.lod_activado, |ui| {
            fila(ui, "Nivel", |ui| {
                ui.add(egui::Slider::new(&mut a.lod, 0.0..=4.0).step_by(0.25).fixed_decimals(2));
            });
        });
        nota(ui, "Más alto = texturas más simples y más FPS en juegos pesados. 0.5 es un buen punto medio.");
    }
}

fn acerca(ui: &mut egui::Ui) {
    titulo(ui, "Acerca de");
    ui.label(RichText::new(format!("{NOMBRE} {VERSION}")).size(18.0).strong().color(TEXTO));
    ui.label(RichText::new("Emulador de Android para Linux.").color(ICONO));
    nota(ui, "Usa el Android de SwagPlayer (redroid con Houdini): video sin comprimir y control al estilo scrcpy.");
}

fn titulo(ui: &mut egui::Ui, texto: &str) {
    ui.label(RichText::new(texto).size(16.0).strong().color(TEXTO));
}

fn nota(ui: &mut egui::Ui, texto: &str) {
    ui.label(RichText::new(texto).size(12.0).color(TENUE));
}

fn fila(ui: &mut egui::Ui, etiqueta: &str, contenido: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(120.0, 24.0), egui::Label::new(RichText::new(etiqueta).color(ICONO)));
        contenido(ui);
    });
}

fn linea(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, FONDO_HOVER);
}

fn pestana(ui: &mut egui::Ui, elegida: bool, glifo: &str, nombre: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(160.0, 38.0), Sense::click());
    if elegida || resp.hovered() {
        ui.painter().rect_filled(rect.shrink2(Vec2::new(8.0, 2.0)), 4.0, FONDO_HOVER);
    }
    if elegida {
        let barra = egui::Rect::from_min_size(rect.min + Vec2::new(8.0, 9.0), Vec2::new(3.0, 20.0));
        ui.painter().rect_filled(barra, 1.5, ACENTO);
    }
    let color = if elegida { TEXTO } else { ICONO };
    let y = rect.center().y;
    ui.painter().text(egui::pos2(rect.min.x + 24.0, y), Align2::LEFT_CENTER, glifo, FontId::proportional(15.0), color);
    ui.painter().text(egui::pos2(rect.min.x + 48.0, y), Align2::LEFT_CENTER, nombre, FontId::proportional(13.5), color);
    resp
}
