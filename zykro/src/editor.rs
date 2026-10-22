//! Controles del juego al estilo de BlueStacks: insignias semitransparentes sobre el juego y un
//! editor con panel lateral para agregar controles arrastrándolos a la pantalla.

use eframe::egui::{self, Align, Align2, Color32, FontId, Id, LayerId, Layout, Order, Painter, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as ic;
use serde_json::Map;

use crate::mapeo::{etiqueta, Arrastre, Clic, Dpad, Mapa, Nodo, Punto, Raton};
use crate::{ACENTO, FONDO, FONDO_CONTROL, FONDO_HOVER, ICONO, TENUE, TEXTO};

/// Parte de un control que se puede tocar en el editor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Asa {
    Nodo(usize),
    DpadCentro(usize),
    DpadBorde(usize),
    DpadTecla(usize, Dir),
    ArrastreFin(usize),
    MultiPaso(usize, usize),
    Apuntar,
    Disparo,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dir {
    Arriba,
    Abajo,
    Izquierda,
    Derecha,
}

/// A qué se le asigna la próxima tecla.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Objetivo {
    Nodo(usize),
    Dpad(usize, Dir),
    Cambio,
    Pausa,
}

#[derive(Clone, Copy, PartialEq)]
enum Tipo {
    Toque,
    Doble,
    Dpad,
    Apuntar,
    Deslizar,
}

const TIPOS: [(Tipo, &str, &str); 5] = [
    (Tipo::Toque, ic::HAND_TAP, "Toque"),
    (Tipo::Doble, ic::CURSOR_CLICK, "Doble toque"),
    (Tipo::Dpad, ic::JOYSTICK, "D-pad"),
    (Tipo::Apuntar, ic::CROSSHAIR, "Apuntar"),
    (Tipo::Deslizar, ic::HAND_SWIPE_RIGHT, "Deslizar"),
];

pub struct Zona {
    asa: Asa,
    centro: Pos2,
    radio: f32,
}

pub fn a_pantalla(rect: Rect, p: Punto) -> Pos2 {
    rect.min + Vec2::new(p.x * rect.width(), p.y * rect.height())
}

pub fn a_punto(rect: Rect, pos: Pos2) -> Punto {
    Punto::nuevo((pos.x - rect.min.x) / rect.width(), (pos.y - rect.min.y) / rect.height())
}

// ---------------------------------------------------------------------------------------------
// Dibujo de los controles
// ---------------------------------------------------------------------------------------------

struct Estilo {
    radio: f32,
    opacidad: f32,
}

impl Estilo {
    fn alfa(&self, c: Color32, a: f32) -> Color32 {
        c.gamma_multiply(a * self.opacidad)
    }
}

/// Insignia redonda con la tecla, como las de BlueStacks. Los textos largos se vuelven píldora.
fn insignia(p: &Painter, e: &Estilo, centro: Pos2, texto: &str, radio: f32, elegida: bool) {
    let largo = texto.chars().count();
    let tam = if largo <= 2 { radio * 0.9 } else { radio * 0.62 };
    let galeria = p.layout_no_wrap(texto.to_string(), FontId::proportional(tam), e.alfa(Color32::WHITE, 1.0));
    let ancho = (galeria.size().x + radio * 0.9).max(radio * 2.0);
    let caja = Rect::from_center_size(centro, Vec2::new(ancho, radio * 2.0));
    let borde = if elegida { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.2, e.alfa(Color32::WHITE, 0.75)) };
    p.rect(caja, radio, e.alfa(Color32::from_black_alpha(165), 1.0), borde, egui::StrokeKind::Middle);
    p.galley(centro - galeria.size() / 2.0, galeria, Color32::WHITE);
}

fn icono_insignia(p: &Painter, e: &Estilo, centro: Pos2, glifo: &str, sub: Option<&str>, elegida: bool) {
    let r = e.radio * 1.15;
    let borde = if elegida { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.2, e.alfa(Color32::WHITE, 0.75)) };
    p.circle(centro, r, e.alfa(Color32::from_black_alpha(165), 1.0), borde);
    p.text(centro, Align2::CENTER_CENTER, glifo, FontId::proportional(r * 1.05), e.alfa(Color32::WHITE, 1.0));
    if let Some(s) = sub {
        insignia(p, e, centro + Vec2::new(0.0, r + e.radio * 0.75), s, e.radio * 0.62, false);
    }
}

/// Dibuja los controles del mapa. Devuelve las zonas que se pueden tocar en el editor.
pub fn dibujar(p: &Painter, rect: Rect, mapa: &Mapa, opacidad: f32, elegida: Option<Asa>, editando: bool) -> Vec<Zona> {
    let e = Estilo { radio: (rect.height() / 720.0 * 15.0).clamp(10.0, 24.0), opacidad };
    let mut zonas = Vec::new();
    let r = e.radio;

    for (i, nodo) in mapa.nodos.iter().enumerate() {
        match nodo {
            Nodo::Clic(c, doble) => {
                let pos = a_pantalla(rect, c.pos);
                let texto = if *doble { format!("{}×2", etiqueta(&c.key)) } else { etiqueta(&c.key) };
                insignia(p, &e, pos, &texto, r, elegida == Some(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r });
            }
            Nodo::Multi(m) => {
                if let Some(primero) = m.pasos.first() {
                    let pos = a_pantalla(rect, primero.pos);
                    insignia(p, &e, pos, &etiqueta(&m.key), r, elegida == Some(Asa::Nodo(i)));
                    zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r });
                }
                if editando {
                    for (j, paso) in m.pasos.iter().enumerate().skip(1) {
                        let pos = a_pantalla(rect, paso.pos);
                        insignia(p, &e, pos, &(j + 1).to_string(), r * 0.6, elegida == Some(Asa::MultiPaso(i, j)));
                        zonas.push(Zona { asa: Asa::MultiPaso(i, j), centro: pos, radio: r * 0.6 });
                    }
                }
            }
            Nodo::Dpad(d) => {
                let c = a_pantalla(rect, d.centro);
                let (rx, ry) = (d.radio() * rect.width(), d.radio() * rect.height());
                let rad = rx.max(ry).max(r * 2.6);
                let sel = matches!(elegida, Some(Asa::DpadCentro(k) | Asa::DpadBorde(k) | Asa::DpadTecla(k, _)) if k == i);
                let borde = if sel { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.5, e.alfa(Color32::WHITE, 0.6)) };
                p.circle(c, rad, e.alfa(Color32::from_black_alpha(90), 1.0), borde);
                p.circle_filled(c, r * 0.9, e.alfa(Color32::WHITE, 0.25));
                let teclas = [
                    (Dir::Arriba, &d.arriba, Vec2::new(0.0, -rad)),
                    (Dir::Abajo, &d.abajo, Vec2::new(0.0, rad)),
                    (Dir::Izquierda, &d.izquierda, Vec2::new(-rad, 0.0)),
                    (Dir::Derecha, &d.derecha, Vec2::new(rad, 0.0)),
                ];
                for (dir, tecla, desp) in teclas {
                    let pos = c + desp;
                    insignia(p, &e, pos, &etiqueta(tecla), r * 0.85, elegida == Some(Asa::DpadTecla(i, dir)));
                    zonas.push(Zona { asa: Asa::DpadTecla(i, dir), centro: pos, radio: r * 0.85 });
                }
                zonas.push(Zona { asa: Asa::DpadCentro(i), centro: c, radio: r * 1.4 });
                if editando {
                    let asa = c + Vec2::new(rad, rad).normalized() * rad;
                    p.circle(asa, 5.0, ACENTO, Stroke::new(1.0, FONDO));
                    zonas.push(Zona { asa: Asa::DpadBorde(i), centro: asa, radio: 8.0 });
                }
            }
            Nodo::Arrastre(a) => {
                let (ini, fin) = (a_pantalla(rect, a.inicio), a_pantalla(rect, a.fin));
                let color = e.alfa(Color32::WHITE, 0.8);
                p.arrow(ini, fin - ini, Stroke::new(2.0, color));
                if editando {
                    p.circle(fin, r * 0.45, e.alfa(Color32::from_black_alpha(165), 1.0), Stroke::new(1.2, color));
                    zonas.push(Zona { asa: Asa::ArrastreFin(i), centro: fin, radio: r * 0.6 });
                }
                insignia(p, &e, ini, &etiqueta(&a.key), r, elegida == Some(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: ini, radio: r });
            }
            Nodo::Otro(_) => {}
        }
    }

    if let Some(raton) = &mapa.raton {
        let pos = a_pantalla(rect, raton.inicio);
        icono_insignia(p, &e, pos, ic::CROSSHAIR, Some(&etiqueta(&mapa.cambio)), elegida == Some(Asa::Apuntar));
        zonas.push(Zona { asa: Asa::Apuntar, centro: pos, radio: r * 1.2 });
        if let Some(d) = raton.disparo {
            let pos = a_pantalla(rect, d);
            icono_insignia(p, &e, pos, ic::MOUSE_LEFT_CLICK, None, elegida == Some(Asa::Disparo));
            zonas.push(Zona { asa: Asa::Disparo, centro: pos, radio: r * 1.2 });
        }
    }
    zonas
}

// ---------------------------------------------------------------------------------------------
// Editor
// ---------------------------------------------------------------------------------------------

pub enum Salida {
    Nada,
    Guardar(Mapa),
    Cerrar,
}

pub struct Editor {
    pub abierto: bool,
    pub mapa: Mapa,
    original: Mapa,
    paquete: String,
    elegida: Option<Asa>,
    esperando: Option<Objetivo>,
    arrastrando: Option<Asa>,
    nuevo: Option<Tipo>,
    ultimo_rect: Rect,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            abierto: false,
            mapa: Mapa::default(),
            original: Mapa::default(),
            paquete: String::new(),
            elegida: None,
            esperando: None,
            arrastrando: None,
            nuevo: None,
            ultimo_rect: Rect::NOTHING,
        }
    }

    pub fn abrir(&mut self, paquete: &str, mapa: Option<Mapa>) {
        self.mapa = mapa.unwrap_or_default();
        self.original = self.mapa.clone();
        self.paquete = paquete.to_string();
        self.elegida = None;
        self.esperando = None;
        self.abierto = true;
    }

    pub fn esperando_tecla(&self) -> bool {
        self.esperando.is_some()
    }

    /// Tecla presionada mientras el editor está abierto.
    pub fn tecla(&mut self, nombre: &str) {
        let Some(obj) = self.esperando.take() else {
            if nombre == "Key_Delete" {
                self.eliminar();
            }
            return;
        };
        if nombre == "Key_Escape" {
            return;
        }
        let n = nombre.to_string();
        match obj {
            Objetivo::Nodo(i) => match self.mapa.nodos.get_mut(i) {
                Some(Nodo::Clic(c, _)) => c.key = n,
                Some(Nodo::Multi(m)) => m.key = n,
                Some(Nodo::Arrastre(a)) => a.key = n,
                _ => {}
            },
            Objetivo::Dpad(i, dir) => {
                if let Some(Nodo::Dpad(d)) = self.mapa.nodos.get_mut(i) {
                    *match dir {
                        Dir::Arriba => &mut d.arriba,
                        Dir::Abajo => &mut d.abajo,
                        Dir::Izquierda => &mut d.izquierda,
                        Dir::Derecha => &mut d.derecha,
                    } = n;
                }
            }
            Objetivo::Cambio => self.mapa.cambio = n,
            Objetivo::Pausa => {
                if let Some(r) = &mut self.mapa.raton {
                    r.tecla_pausa = Some(n);
                }
            }
        }
    }

    fn eliminar(&mut self) {
        match self.elegida.take() {
            Some(Asa::Nodo(i) | Asa::DpadCentro(i) | Asa::DpadBorde(i) | Asa::DpadTecla(i, _) | Asa::ArrastreFin(i) | Asa::MultiPaso(i, _)) => {
                if i < self.mapa.nodos.len() {
                    self.mapa.nodos.remove(i);
                }
            }
            Some(Asa::Apuntar | Asa::Disparo) => self.mapa.raton = None,
            None => {}
        }
        self.esperando = None;
    }

    fn crear(&mut self, tipo: Tipo, p: Punto) {
        let extra = Map::new;
        let (asa, objetivo) = match tipo {
            Tipo::Toque | Tipo::Doble => {
                let clic = Clic { key: String::new(), pos: p, cambia_modo: false, extra: extra() };
                self.mapa.nodos.push(Nodo::Clic(clic, tipo == Tipo::Doble));
                let i = self.mapa.nodos.len() - 1;
                (Asa::Nodo(i), Some(Objetivo::Nodo(i)))
            }
            Tipo::Dpad => {
                let d = Dpad {
                    centro: p,
                    arriba: "W".into(),
                    abajo: "S".into(),
                    izquierda: "A".into(),
                    derecha: "D".into(),
                    d_arriba: 0.04,
                    d_abajo: 0.04,
                    d_izquierda: 0.04,
                    d_derecha: 0.04,
                    extra: extra(),
                };
                self.mapa.nodos.push(Nodo::Dpad(d));
                (Asa::DpadCentro(self.mapa.nodos.len() - 1), None)
            }
            Tipo::Apuntar => {
                self.mapa.raton = Some(Raton::nuevo(p));
                (Asa::Apuntar, None)
            }
            Tipo::Deslizar => {
                let a = Arrastre { key: String::new(), inicio: p, fin: Punto::nuevo(p.x + 0.12, p.y), velocidad: 1.0, extra: extra() };
                self.mapa.nodos.push(Nodo::Arrastre(a));
                let i = self.mapa.nodos.len() - 1;
                (Asa::Nodo(i), Some(Objetivo::Nodo(i)))
            }
        };
        self.elegida = Some(asa);
        self.esperando = objetivo;
    }

    fn mover(&mut self, asa: Asa, p: Punto, pos: Pos2, rect: Rect) {
        match asa {
            Asa::Nodo(i) => match self.mapa.nodos.get_mut(i) {
                Some(Nodo::Clic(c, _)) => c.pos = p,
                Some(Nodo::Multi(m)) => {
                    if let Some(paso) = m.pasos.first_mut() {
                        paso.pos = p;
                    }
                }
                Some(Nodo::Arrastre(a)) => {
                    let (dx, dy) = (a.fin.x - a.inicio.x, a.fin.y - a.inicio.y);
                    a.inicio = p;
                    a.fin = Punto::nuevo(p.x + dx, p.y + dy);
                }
                _ => {}
            },
            Asa::DpadCentro(i) | Asa::DpadTecla(i, _) => {
                if let Some(Nodo::Dpad(d)) = self.mapa.nodos.get_mut(i) {
                    d.centro = p;
                }
            }
            Asa::DpadBorde(i) => {
                if let Some(Nodo::Dpad(d)) = self.mapa.nodos.get_mut(i) {
                    let c = a_pantalla(rect, d.centro);
                    d.fijar_radio(c.distance(pos) / rect.width());
                }
            }
            Asa::ArrastreFin(i) => {
                if let Some(Nodo::Arrastre(a)) = self.mapa.nodos.get_mut(i) {
                    a.fin = p;
                }
            }
            Asa::MultiPaso(i, j) => {
                if let Some(Nodo::Multi(m)) = self.mapa.nodos.get_mut(i) {
                    if let Some(paso) = m.pasos.get_mut(j) {
                        paso.pos = p;
                    }
                }
            }
            Asa::Apuntar => {
                if let Some(r) = &mut self.mapa.raton {
                    r.inicio = p;
                }
            }
            Asa::Disparo => {
                if let Some(r) = &mut self.mapa.raton {
                    r.disparo = Some(p);
                }
            }
        }
    }

    /// Interacción sobre la pantalla del juego: elegir, mover y soltar controles nuevos.
    pub fn pantalla(&mut self, ui: &mut egui::Ui, rect: Rect) {
        self.ultimo_rect = rect;
        let painter = ui.painter_at(rect);
        // Velo ligero para que se note que se está editando.
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(60));
        let zonas = dibujar(&painter, rect, &self.mapa, 1.0, self.elegida, true);

        let resp = ui.interact(rect, Id::new("editor-pantalla"), Sense::click_and_drag());
        let tocada = |pos: Pos2| zonas.iter().rev().find(|z| z.centro.distance(pos) <= z.radio + 3.0).map(|z| z.asa);
        if let Some(pos) = resp.interact_pointer_pos() {
            if resp.drag_started() {
                self.arrastrando = tocada(pos);
                if self.arrastrando.is_some() {
                    self.elegida = self.arrastrando;
                    self.esperando = None;
                }
            }
            if resp.dragged() {
                if let Some(asa) = self.arrastrando {
                    self.mover(asa, a_punto(rect, pos), pos, rect);
                }
            }
            if resp.clicked() {
                self.elegida = tocada(pos);
                // Al hacer clic en una tecla, la próxima tecla que presiones se asigna.
                self.esperando = match self.elegida {
                    Some(Asa::Nodo(i)) => Some(Objetivo::Nodo(i)),
                    Some(Asa::DpadTecla(i, d)) => Some(Objetivo::Dpad(i, d)),
                    Some(Asa::Apuntar) => Some(Objetivo::Cambio),
                    _ => None,
                };
            }
        }
        if resp.drag_stopped() {
            self.arrastrando = None;
        }
        if let Some(Objetivo::Nodo(_) | Objetivo::Dpad(..) | Objetivo::Cambio) = self.esperando {
            let aviso = "Presiona una tecla o botón del mouse  ·  Esc para cancelar";
            let pos = rect.center_top() + Vec2::new(0.0, 28.0);
            let galeria = painter.layout_no_wrap(aviso.into(), FontId::proportional(14.0), Color32::WHITE);
            let caja = Rect::from_center_size(pos, galeria.size() + Vec2::new(24.0, 14.0));
            painter.rect_filled(caja, 8.0, ACENTO);
            painter.galley(caja.center() - galeria.size() / 2.0, galeria, Color32::WHITE);
        }

        // Control nuevo arrastrado desde el panel.
        if let Some(tipo) = self.nuevo {
            let ctx = ui.ctx();
            if let Some(pos) = ctx.pointer_latest_pos() {
                let capa = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("fantasma")));
                let glifo = TIPOS.iter().find(|t| t.0 == tipo).map(|t| t.1).unwrap_or(ic::PLUS);
                capa.circle(pos, 18.0, ACENTO.gamma_multiply(0.85), Stroke::new(1.5, Color32::WHITE));
                capa.text(pos, Align2::CENTER_CENTER, glifo, FontId::proportional(18.0), FONDO);
                if ctx.input(|i| i.pointer.any_released()) {
                    if rect.contains(pos) {
                        self.crear(tipo, a_punto(rect, pos));
                    }
                    self.nuevo = None;
                }
            }
        }
    }

    /// Panel lateral del editor.
    pub fn panel(&mut self, ui: &mut egui::Ui) -> Salida {
        let mut salida = Salida::Nada;
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new(ic::GAME_CONTROLLER).size(18.0).color(ACENTO));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("Controles del juego").size(14.5).strong().color(TEXTO));
                let app = if self.paquete.is_empty() { "Sin app".to_string() } else { self.paquete.clone() };
                ui.label(RichText::new(app).size(11.0).color(TENUE));
            });
        });

        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            seccion(ui, "Agregar control");
            ui.label(RichText::new("Arrástralo a la pantalla").size(11.5).color(TENUE));
            egui::Grid::new("tipos").num_columns(2).spacing(Vec2::splat(6.0)).show(ui, |ui| {
                for (k, (tipo, glifo, nombre)) in TIPOS.iter().enumerate() {
                    let habilitado = *tipo != Tipo::Apuntar || self.mapa.raton.is_none();
                    let resp = mosaico(ui, glifo, nombre, habilitado);
                    if habilitado && resp.drag_started() {
                        self.nuevo = Some(*tipo);
                    }
                    if habilitado && resp.clicked() && self.ultimo_rect.is_positive() {
                        self.crear(*tipo, a_punto(self.ultimo_rect, self.ultimo_rect.center()));
                    }
                    if k % 2 == 1 {
                        ui.end_row();
                    }
                }
            });

            seccion(ui, "Control elegido");
            self.propiedades(ui);

            seccion(ui, "Vista");
            ui.horizontal(|ui| {
                ui.label(RichText::new("Opacidad").color(ICONO));
                ui.add(egui::Slider::new(&mut self.mapa.overlay.opacity, 0.1..=1.0).show_value(false));
            });
            ui.checkbox(&mut self.mapa.overlay.enabled, "Mostrar controles al jugar");
        });

        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                let ancho = (ui.available_width() - 32.0) / 2.0;
                if ui.add(egui::Button::new("Cancelar").min_size(Vec2::new(ancho, 32.0))).clicked() {
                    salida = Salida::Cerrar;
                }
                let guardar = egui::Button::new(RichText::new(format!("{}  Guardar", ic::FLOPPY_DISK)).color(Color32::WHITE).strong())
                    .fill(ACENTO)
                    .min_size(Vec2::new(ancho, 32.0));
                if ui.add(guardar).clicked() {
                    salida = Salida::Guardar(self.mapa.clone());
                }
            });
            if ui.add(egui::Button::new(RichText::new(format!("{}  Restablecer", ic::ARROW_COUNTER_CLOCKWISE)).color(ICONO)).frame(false)).clicked() {
                self.mapa = self.original.clone();
                self.elegida = None;
                self.esperando = None;
            }
        });
        if matches!(salida, Salida::Guardar(_) | Salida::Cerrar) {
            self.abierto = false;
        }
        salida
    }

    fn boton_tecla(&mut self, ui: &mut egui::Ui, texto: &str, tecla: &str, objetivo: Objetivo) {
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(78.0, 24.0), egui::Label::new(RichText::new(texto).color(ICONO)));
            let esperando = self.esperando == Some(objetivo);
            let etiqueta = if esperando { "Presiona…".to_string() } else { etiqueta(tecla) };
            let mut b = egui::Button::new(RichText::new(etiqueta).strong()).min_size(Vec2::new(110.0, 24.0));
            if esperando {
                b = b.fill(ACENTO.gamma_multiply(0.35)).stroke(Stroke::new(1.0, ACENTO));
            }
            if ui.add(b).clicked() {
                self.esperando = Some(objetivo);
            }
        });
    }

    fn propiedades(&mut self, ui: &mut egui::Ui) {
        let Some(asa) = self.elegida else {
            ui.label(RichText::new("Haz clic en un control y presiona una tecla para asignarla. Arrástralo para moverlo; Supr lo borra.").size(11.5).color(TENUE));
            return;
        };
        let mut borrar = false;
        match asa {
            Asa::Nodo(i) | Asa::DpadCentro(i) | Asa::DpadBorde(i) | Asa::DpadTecla(i, _) | Asa::ArrastreFin(i) | Asa::MultiPaso(i, _) => {
                let Some(nodo) = self.mapa.nodos.get(i).cloned() else { return };
                match nodo {
                    Nodo::Clic(c, doble) => {
                        titulo(ui, ic::HAND_TAP, if doble { "Doble toque" } else { "Toque" });
                        self.boton_tecla(ui, "Tecla", &c.key, Objetivo::Nodo(i));
                        if let Some(Nodo::Clic(c, doble)) = self.mapa.nodos.get_mut(i) {
                            ui.checkbox(doble, "Doble toque");
                            ui.checkbox(&mut c.cambia_modo, "También activa/desactiva la puntería");
                        }
                    }
                    Nodo::Multi(m) => {
                        titulo(ui, ic::CURSOR_CLICK, "Varios toques");
                        self.boton_tecla(ui, "Tecla", &m.key, Objetivo::Nodo(i));
                    }
                    Nodo::Dpad(d) => {
                        titulo(ui, ic::JOYSTICK, "D-pad");
                        self.boton_tecla(ui, "Arriba", &d.arriba, Objetivo::Dpad(i, Dir::Arriba));
                        self.boton_tecla(ui, "Izquierda", &d.izquierda, Objetivo::Dpad(i, Dir::Izquierda));
                        self.boton_tecla(ui, "Abajo", &d.abajo, Objetivo::Dpad(i, Dir::Abajo));
                        self.boton_tecla(ui, "Derecha", &d.derecha, Objetivo::Dpad(i, Dir::Derecha));
                        if let Some(Nodo::Dpad(d)) = self.mapa.nodos.get_mut(i) {
                            let mut radio = d.radio();
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Tamaño").color(ICONO));
                                if ui.add(egui::Slider::new(&mut radio, 0.015..=0.25).show_value(false)).changed() {
                                    d.fijar_radio(radio);
                                }
                            });
                        }
                    }
                    Nodo::Arrastre(a) => {
                        titulo(ui, ic::HAND_SWIPE_RIGHT, "Deslizar");
                        self.boton_tecla(ui, "Tecla", &a.key, Objetivo::Nodo(i));
                        if let Some(Nodo::Arrastre(a)) = self.mapa.nodos.get_mut(i) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Velocidad").color(ICONO));
                                ui.add(egui::Slider::new(&mut a.velocidad, 0.2..=3.0).fixed_decimals(1));
                            });
                        }
                    }
                    Nodo::Otro(_) => {
                        ui.label(RichText::new("Este tipo de control todavía no se puede editar.").color(TENUE));
                    }
                }
            }
            Asa::Apuntar | Asa::Disparo => {
                let (cambio, pausa) = (self.mapa.cambio.clone(), self.mapa.raton.as_ref().and_then(|r| r.tecla_pausa.clone()).unwrap_or_default());
                titulo(ui, ic::CROSSHAIR, "Apuntar y disparar");
                self.boton_tecla(ui, "Activar", &cambio, Objetivo::Cambio);
                self.boton_tecla(ui, "Pausar", &pausa, Objetivo::Pausa);
                if let Some(r) = &mut self.mapa.raton {
                    sensibilidad(ui, r);
                    ui.checkbox(&mut r.disparo_con_clic, "Disparar con clic izquierdo");
                    if r.disparo_con_clic && r.disparo.is_none() {
                        r.disparo = Some(Punto::nuevo(r.inicio.x + 0.05, r.inicio.y + 0.2));
                    }
                }
            }
        }
        ui.add_space(4.0);
        if ui.add(egui::Button::new(RichText::new(format!("{}  Eliminar", ic::TRASH)).color(crate::CERRAR_HOVER))).clicked() {
            borrar = true;
        }
        if borrar {
            self.eliminar();
        }
    }
}

fn seccion(ui: &mut egui::Ui, texto: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(texto.to_uppercase()).size(10.5).strong().color(TENUE));
}

fn titulo(ui: &mut egui::Ui, glifo: &str, texto: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(glifo).size(15.0).color(ACENTO));
        ui.label(RichText::new(texto).size(13.5).strong().color(TEXTO));
    });
}

fn deslizador(ui: &mut egui::Ui, texto: &str, valor: &mut f32, rango: std::ops::RangeInclusive<f32>) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(78.0, 20.0), egui::Label::new(RichText::new(texto).color(ICONO)));
        ui.add(egui::Slider::new(valor, rango).logarithmic(true).fixed_decimals(2)).changed()
    })
    .inner
}

/// Sensibilidad horizontal y vertical de la puntería (la usan el editor y el menú rápido).
/// En el archivo, vertical = sensitivityX × sensitivityRatioY; aquí se muestran por separado.
/// Devuelve `true` si cambió.
pub fn sensibilidad(ui: &mut egui::Ui, r: &mut Raton) -> bool {
    let mut vertical = r.sens * r.ratio_y;
    let cambio = deslizador(ui, "Horizontal", &mut r.sens, 0.1..=10.0) | deslizador(ui, "Vertical", &mut vertical, 0.1..=10.0);
    r.ratio_y = vertical / r.sens.max(0.01);
    cambio
}

/// Mosaico de un tipo de control en el panel (se arrastra a la pantalla o se hace clic).
fn mosaico(ui: &mut egui::Ui, glifo: &str, nombre: &str, habilitado: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(112.0, 62.0), Sense::click_and_drag());
    let encima = habilitado && resp.hovered();
    ui.painter().rect_filled(rect, 6.0, if encima { FONDO_HOVER } else { FONDO_CONTROL });
    let color = if !habilitado { TENUE.gamma_multiply(0.6) } else if encima { Color32::WHITE } else { ICONO };
    ui.painter().text(rect.center() - Vec2::new(0.0, 9.0), Align2::CENTER_CENTER, glifo, FontId::proportional(20.0), color);
    ui.painter().text(rect.center() + Vec2::new(0.0, 15.0), Align2::CENTER_CENTER, nombre, FontId::proportional(12.0), color);
    if habilitado {
        resp.on_hover_cursor(egui::CursorIcon::Grab)
    } else {
        resp.on_hover_text("Solo puede haber uno")
    }
}
