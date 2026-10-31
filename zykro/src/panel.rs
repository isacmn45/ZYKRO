//! Panel de notificaciones de ZYKRO, compacto como el de Android 4 (KitKat) y con el tema de ZYKRO:
//! una fila de interruptores con su nombre, el volumen y las notificaciones en tarjetas, cubriendo toda
//! la imagen. Lo dibuja ZYKRO en vez del de Android, que gasta mucho espacio; se baja arrastrando
//! desde el borde de arriba de la imagen y se sube arrastrándolo hacia arriba.
//! También muestra, con el mismo tema, las notificaciones emergentes (las de Android están apagadas).
//! Las notificaciones las exporta la pantalla de inicio de ZYKRO (`Escucha.java`), y ZYKRO le pide
//! abrirlas o quitarlas con `am broadcast` (`Acciones.java`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, emath::easing, scroll_area::ScrollSource, text::LayoutJob, text::TextWrapping, Align2, Color32, ColorImage, FontId, Id,
    Order, Pos2, Rect, Sense, Shadow, Stroke, StrokeKind, TextFormat, TextureHandle, TextureOptions, Vec2,
};
use egui_phosphor::regular as ic;

use crate::android;
use crate::{ACENTO, FONDO, FONDO_CONTROL, FONDO_HOVER, FONDO_OSCURO, ICONO, TENUE, TEXTO};

/// Carpeta de Android donde la pantalla de inicio deja las notificaciones y sus íconos.
const EXPORTADO: &str = "/data/media/0/Android/data/com.zykro.inicio/files";

/// Separación entre tarjetas y botones, y margen del panel.
const SEP: f32 = 10.0;
const MARGEN: f32 = 20.0;
/// Ancho mínimo de una tarjeta de notificación (con más ancho, van en columnas).
const ANCHO_TARJETA: f32 = 340.0;
/// Ancho máximo de la notificación emergente.
const ANCHO_EMERGENTE: f32 = 440.0;
/// Lo que tarda en bajar o subir el panel o la notificación emergente.
const DURACION: f64 = 0.2;
/// Cuánto se queda a la vista una notificación emergente, como en Android.
const SEGUNDOS_EMERGENTE: f64 = 5.0;
/// Al soltar un arrastre hacia arriba más rápido que esto (puntos por segundo), se cierra.
const VELOCIDAD_CIERRE: f32 = 600.0;

const DIAS: [&str; 7] = ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"];
const MESES: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

/// Lo que la ventana tiene que hacer al tocar algunos interruptores.
pub enum Accion {
    Captura,
    AjustesAndroid,
    /// Abrir el panel de Android de siempre (música, responder mensajes).
    PanelAndroid,
}

#[derive(Clone)]
struct Notificacion {
    clave: String,
    app: String,
    titulo: String,
    texto: String,
    /// Milisegundos desde 1970.
    hora: i64,
    descartable: bool,
    acciones: Vec<String>,
    icono: String,
    /// Última vez que avisó (ms desde 1970); 0 si no avisó desde que la pantalla de inicio la vio.
    alerta: i64,
}

#[derive(Clone, Default)]
struct Datos {
    notificaciones: Vec<Notificacion>,
    avion: bool,
    no_molestar: bool,
    girar: bool,
    hora24: bool,
    volumen: u32,
    volumen_max: u32,
}

/// Un deslizamiento entre la vista (0) y escondido arriba (1), con la misma curva que egui.
#[derive(Clone, Copy)]
struct Deslizar {
    desde: f32,
    hasta: f32,
    inicio: f64,
}

impl Default for Deslizar {
    fn default() -> Self {
        Deslizar::quieto(1.0)
    }
}

impl Deslizar {
    fn quieto(valor: f32) -> Self {
        Deslizar { desde: valor, hasta: valor, inicio: f64::NEG_INFINITY }
    }

    fn valor(&self, ahora: f64) -> f32 {
        let t = ((ahora - self.inicio) / DURACION).clamp(0.0, 1.0) as f32;
        self.desde + (self.hasta - self.desde) * easing::cubic_out(t)
    }

    fn ir(&mut self, hasta: f32, ahora: f64) {
        *self = Deslizar { desde: self.valor(ahora), hasta, inicio: ahora };
    }

    fn terminado(&self, ahora: f64) -> bool {
        ahora - self.inicio >= DURACION
    }
}

/// Lo que comparten la ventana y el hilo que lee de Android.
#[derive(Clone, Default)]
struct Lector {
    datos: Arc<Mutex<Option<Datos>>>,
    iconos_nuevos: Arc<Mutex<Vec<(String, ColorImage)>>>,
    /// Llegaron datos nuevos (para buscar notificaciones que avisen).
    recien_leidos: Arc<AtomicBool>,
    lectura: Arc<Mutex<Lectura>>,
}

#[derive(Default)]
struct Lectura {
    leyendo: bool,
    /// Algo cambió mientras se leía: hay que leer otra vez al terminar.
    otra_vez: bool,
}

impl Lector {
    /// Vuelve a leer las notificaciones y los interruptores (en otro hilo).
    fn recargar(&self, ctx: &egui::Context) {
        {
            let mut lectura = self.lectura.lock().unwrap();
            if lectura.leyendo {
                lectura.otra_vez = true;
                return;
            }
            lectura.leyendo = true;
        }
        let (lector, ctx) = (self.clone(), ctx.clone());
        thread::spawn(move || loop {
            let (leidos, nuevos) = leer();
            *lector.datos.lock().unwrap() = Some(leidos);
            *lector.iconos_nuevos.lock().unwrap() = nuevos;
            lector.recien_leidos.store(true, Ordering::Relaxed);
            ctx.request_repaint();
            let mut lectura = lector.lectura.lock().unwrap();
            if !std::mem::take(&mut lectura.otra_vez) {
                lectura.leyendo = false;
                break;
            }
        });
    }
}

/// Una notificación que acaba de llegar, arriba de la imagen.
struct Emergente {
    n: Notificacion,
    /// `false` = subiendo para irse.
    visible: bool,
    posicion: Deslizar,
    /// Cuándo se va sola (se alarga mientras el mouse está encima).
    vence: f64,
    /// Subiéndola con el mouse: lo escondido al empezar y lo último calculado.
    arrastre: Option<(f32, f32)>,
    /// Dónde se dibujó, para que los toques ahí no le lleguen a Android.
    rect: Rect,
}

/// Lo que se tocó en una tarjeta (ya se le pidió a la pantalla de inicio).
enum Toque {
    /// La notificación o uno de sus botones.
    Abrir,
    Quitar,
}

#[derive(Default)]
pub struct Panel {
    abierto: bool,
    /// Bajando (o abierto); `false` = subiendo para cerrarse.
    visible: bool,
    posicion: Deslizar,
    /// Subiéndolo con el mouse: lo escondido al empezar y lo último calculado.
    arrastre: Option<(f32, f32)>,
    lector: Lector,
    texturas: HashMap<String, TextureHandle>,
    /// Volumen mientras se arrastra la barra (se aplica al soltar).
    volumen: Option<f32>,
    emergente: Option<Emergente>,
    /// El aviso más nuevo ya visto (ms); al empezar, la hora en que se abrió ZYKRO, para mostrar
    /// solo las notificaciones que lleguen después.
    ultima_alerta: Option<i64>,
}

impl Panel {
    pub fn abierto(&self) -> bool {
        self.abierto
    }

    pub fn abrir(&mut self, ctx: &egui::Context) {
        if !self.abierto {
            self.abierto = true;
            self.posicion = Deslizar::quieto(1.0);
            self.arrastre = None;
        }
        self.visible = true;
        self.recargar(ctx);
    }

    /// Lo abre siguiendo al mouse, que ya viene bajando desde el borde de arriba de la imagen: al
    /// soltar, se queda abierto si bajó un cuarto (o con un tirón), si no, se vuelve a esconder.
    pub fn abrir_arrastrando(&mut self, ctx: &egui::Context) {
        self.abrir(ctx);
        self.arrastre = Some((1.0, 1.0));
    }

    pub fn cerrar(&mut self) {
        self.visible = false;
    }

    /// Vuelve a leer las notificaciones y los interruptores (en otro hilo).
    pub fn recargar(&self, ctx: &egui::Context) {
        self.lector.recargar(ctx);
    }

    /// Corre `orden` en Android y vuelve a leer.
    fn aplicar(&self, ctx: &egui::Context, orden: String) {
        let (lector, ctx) = (self.lector.clone(), ctx.clone());
        thread::spawn(move || {
            android::shell(&orden);
            lector.recargar(&ctx);
        });
    }

    /// Le pide algo a la pantalla de inicio (abrir o quitar notificaciones).
    fn pedir(accion: &str, clave: Option<&str>, indice: Option<usize>) {
        let mut orden = format!("am broadcast -n com.zykro.inicio/.Acciones -a com.zykro.inicio.{accion}");
        if let Some(clave) = clave {
            orden += &format!(" --es clave '{}'", clave.replace('\'', "'\\''"));
        }
        if let Some(i) = indice {
            orden += &format!(" --ei indice {i}");
        }
        thread::spawn(move || android::shell(&orden));
    }

    fn cargar_iconos(&mut self, ctx: &egui::Context) {
        for (archivo, imagen) in self.lector.iconos_nuevos.lock().unwrap().drain(..) {
            self.texturas.insert(archivo.clone(), ctx.load_texture(format!("notificacion-{archivo}"), imagen, TextureOptions::LINEAR));
        }
    }

    /// Dibuja el panel sobre `area` (la zona del video), cubriéndola entera.
    pub fn mostrar(&mut self, ctx: &egui::Context, area: Rect) -> Option<Accion> {
        self.cargar_iconos(ctx);
        if !self.abierto {
            return None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cerrar();
        }
        let tiempo = ctx.input(|i| i.time);
        let escondido = deslizar(ctx, &mut self.posicion, &mut self.arrastre, &mut self.visible, area.height(), tiempo);
        if !self.visible && escondido >= 1.0 {
            self.abierto = false;
            return None;
        }
        let datos = self.lector.datos.lock().unwrap().clone();
        let d = datos.clone().unwrap_or_default();
        let mut accion = None;
        // Baja desde arriba cubriendo toda la imagen.
        let panel = area.translate(Vec2::new(0.0, -escondido * area.height()));

        // Sin el fundido de egui: ya tiene su deslizamiento.
        egui::Area::new(Id::new("panel")).order(Order::Foreground).fixed_pos(area.min).fade_in(false).show(ctx, |ui| {
            // Que la capa mida toda la imagen: si no, egui cree que el mouse está sobre lo de atrás
            // (sin la X de las tarjetas ni la rueda en la lista).
            ui.set_min_size(area.size());
            let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(panel));
            let p = ui.painter_at(area);
            p.rect_filled(panel, 0.0, FONDO_OSCURO);
            // Arrastrando hacia arriba (desde cualquier parte que no sea un botón) se sube, como en
            // Android: si se suelta pasado un cuarto o con un tirón rápido, se cierra.
            if ui.allocate_rect(panel, Sense::click_and_drag()).drag_started() {
                self.arrastre = Some((escondido, escondido));
            }
            let (x0, x1) = (panel.min.x + MARGEN, panel.max.x - MARGEN);
            let ancho = x1 - x0;
            let mut y = panel.min.y + 14.0;

            // Hora y fecha; a la derecha, «Borrar todo» y cerrar.
            let (ahora, dia, mes, num) = ahora();
            let hora = p.layout_no_wrap(formato_hora(ahora, d.hora24), FontId::proportional(26.0), TEXTO);
            let alto = 40.0;
            p.galley(Pos2::new(x0, y + (alto - hora.size().y) / 2.0), hora.clone(), TEXTO);
            let fecha = format!("{dia}, {num} de {mes}");
            p.text(Pos2::new(x0 + hora.size().x + 12.0, y + alto / 2.0 + 3.0), Align2::LEFT_CENTER, fecha, FontId::proportional(13.0), TENUE);
            let cerrar = Rect::from_min_size(Pos2::new(x1 - 34.0, y + 3.0), Vec2::splat(34.0));
            if boton_icono(ui, &p, cerrar, ic::CARET_UP, "Cerrar") {
                self.cerrar();
            }
            let borrar = Rect::from_min_size(Pos2::new(cerrar.min.x - 8.0 - 130.0, y + 3.0), Vec2::new(130.0, 34.0));
            if d.notificaciones.iter().any(|n| n.descartable) && boton_texto(ui, &p, borrar, ic::BROOM, "Borrar todo") {
                Panel::pedir("BORRAR_TODO", None, None);
            }
            y += alto + 14.0;

            // Interruptores a todo lo ancho (encendido = color de ZYKRO).
            let interruptores: [(&str, &str, Option<bool>); 6] = [
                (ic::AIRPLANE, "Avión", Some(d.avion)),
                (ic::MOON, "No molestar", Some(d.no_molestar)),
                (ic::ARROWS_CLOCKWISE, "Girar", Some(d.girar)),
                (ic::CAMERA, "Captura", None),
                (ic::GEAR, "Ajustes", None),
                (ic::ANDROID_LOGO, "Android", None),
            ];
            let n = interruptores.len() as f32;
            let w = (ancho - SEP * (n - 1.0)) / n;
            for (i, (glifo, nombre, estado)) in interruptores.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * (w + SEP), y), Vec2::new(w, 64.0));
                let resp = ui.allocate_rect(r, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let encendido = estado == &Some(true);
                let fondo = if encendido { ACENTO } else if resp.hovered() { FONDO_HOVER } else { FONDO_CONTROL };
                p.rect_filled(r, 12.0, fondo);
                let color = if encendido { Color32::WHITE } else { ICONO };
                p.text(Pos2::new(r.center().x, r.min.y + 24.0), Align2::CENTER_CENTER, *glifo, FontId::proportional(20.0), color);
                p.text(Pos2::new(r.center().x, r.min.y + 47.0), Align2::CENTER_CENTER, *nombre, FontId::proportional(11.5), color);
                if resp.clicked() {
                    match i {
                        0 => self.aplicar(ctx, format!("cmd connectivity airplane-mode {}", if d.avion { "disable" } else { "enable" })),
                        1 => self.aplicar(ctx, format!("cmd notification set_dnd {}", if d.no_molestar { "off" } else { "on" })),
                        2 => self.aplicar(ctx, format!("settings put system accelerometer_rotation {}", (!d.girar) as u8)),
                        3 => accion = Some(Accion::Captura),
                        4 => accion = Some(Accion::AjustesAndroid),
                        _ => accion = Some(Accion::PanelAndroid),
                    }
                    if i >= 3 {
                        self.cerrar();
                    }
                }
            }
            y += 64.0 + 12.0;

            // Volumen de la música y los juegos (el brillo no hace nada en un emulador).
            let fila = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(ancho, 44.0));
            p.rect_filled(fila, 12.0, FONDO);
            p.text(Pos2::new(x0 + 22.0, fila.center().y), Align2::CENTER_CENTER, ic::SPEAKER_HIGH, FontId::proportional(18.0), ICONO);
            let barra = Rect::from_min_max(Pos2::new(x0 + 46.0, fila.center().y - 12.0), Pos2::new(x1 - 62.0, fila.center().y + 12.0));
            let resp = ui.allocate_rect(barra, Sense::click_and_drag());
            let maximo = d.volumen_max.max(1) as f32;
            if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked() || resp.drag_started()) {
                self.volumen = Some(((pos.x - barra.min.x) / barra.width()).clamp(0.0, 1.0) * maximo);
            }
            let valor = self.volumen.unwrap_or(d.volumen as f32) / maximo;
            let pista = Rect::from_center_size(barra.center(), Vec2::new(barra.width(), 4.0));
            p.rect_filled(pista, 2.0, FONDO_HOVER);
            p.rect_filled(Rect::from_min_size(pista.min, Vec2::new(pista.width() * valor, 4.0)), 2.0, ACENTO);
            let pulgar = Pos2::new(pista.min.x + pista.width() * valor, pista.center().y);
            p.circle(pulgar, 7.0, Color32::WHITE, Stroke::new(2.0, ACENTO));
            let porcentaje = format!("{:.0} %", valor * 100.0);
            p.text(Pos2::new(x1 - 14.0, fila.center().y), Align2::RIGHT_CENTER, porcentaje, FontId::proportional(12.0), TENUE);
            if resp.drag_stopped() || resp.clicked() {
                if let Some(v) = self.volumen.take() {
                    self.aplicar(ctx, format!("cmd media_session volume --stream 3 --set {}", v.round() as u32));
                }
            }
            y += 44.0 + 16.0;

            // Notificaciones en tarjetas; con ancho, en columnas.
            let titulo = match d.notificaciones.len() {
                0 => "Notificaciones".to_string(),
                n => format!("Notificaciones · {n}"),
            };
            p.text(Pos2::new(x0 + 2.0, y + 8.0), Align2::LEFT_CENTER, titulo, FontId::proportional(12.5), TENUE);
            y += 24.0;
            let lista = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, panel.max.y - 22.0));
            match &datos {
                None => {
                    p.text(lista.center_top() + Vec2::new(0.0, 50.0), Align2::CENTER_CENTER, "…", FontId::proportional(16.0), TENUE);
                }
                Some(d) if d.notificaciones.is_empty() => {
                    let centro = lista.center_top() + Vec2::new(0.0, 60.0);
                    p.text(centro, Align2::CENTER_CENTER, ic::BELL_SLASH, FontId::proportional(30.0), FONDO_HOVER);
                    p.text(centro + Vec2::new(0.0, 32.0), Align2::CENTER_CENTER, "No hay notificaciones", FontId::proportional(13.0), TENUE);
                }
                Some(d) => {
                    let columnas = ((ancho + SEP) / (ANCHO_TARJETA + SEP)).floor().clamp(1.0, 3.0) as usize;
                    let w = (ancho - SEP * (columnas as f32 - 1.0)) / columnas as f32;
                    let mut hijo = ui.new_child(egui::UiBuilder::new().max_rect(lista));
                    // Sin arrastrar para desplazar: arrastrar es para subir el panel (la rueda sí desplaza).
                    let desplazar = egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height(lista.height())
                        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL);
                    desplazar.show(&mut hijo, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        for fila in d.notificaciones.chunks(columnas) {
                            let medidas: Vec<Medida> = fila.iter().map(|n| medir(ui.painter(), n, w)).collect();
                            let alto = medidas.iter().map(|m| m.alto).fold(0.0, f32::max);
                            let (r, _) = ui.allocate_exact_size(Vec2::new(ancho, alto + SEP), Sense::hover());
                            for (j, (n, m)) in fila.iter().zip(medidas).enumerate() {
                                let (_, toque) = tarjeta(ui, &self.texturas, tarjeta_r(r, j, w, alto), n, m, d.hora24, Sense::click());
                                if let Some(Toque::Abrir) = toque {
                                    self.cerrar();
                                }
                            }
                        }
                    });
                }
            }

            // Asa de abajo: tocarla (o arrastrarla hacia arriba) sube el panel.
            let asa = Rect::from_min_max(Pos2::new(panel.min.x, panel.max.y - 22.0), panel.max);
            p.rect_filled(Rect::from_center_size(asa.center(), Vec2::new(44.0, 4.0)), 2.0, FONDO_HOVER);
            if ui.allocate_rect(asa, Sense::click()).clicked() {
                self.cerrar();
            }
        });
        accion
    }

    /// Muestra la notificación que acaba de llegar arriba de la imagen, unos segundos. Con
    /// `interactivo` en `false` (puntería activa) solo se ve, un poco transparente.
    pub fn mostrar_emergente(&mut self, ctx: &egui::Context, area: Rect, interactivo: bool) {
        let ahora = ctx.input(|i| i.time);
        self.ultima_alerta.get_or_insert_with(|| ahora_ms());
        if self.lector.recien_leidos.swap(false, Ordering::Relaxed) {
            let datos = self.lector.datos.lock().unwrap().clone();
            if let Some(d) = datos {
                self.revisar_alertas(&d, ahora);
            }
        }
        let hora24 = self.lector.datos.lock().unwrap().as_ref().is_some_and(|d| d.hora24);
        let Some(e) = self.emergente.as_mut() else { return };
        // Con el panel abierto ya se ven todas.
        if self.abierto || ahora >= e.vence {
            e.visible = false;
        }
        let ancho = (area.width() - 2.0 * MARGEN).min(ANCHO_EMERGENTE);
        let capa = egui::LayerId::new(Order::Foreground, Id::new("emergente"));
        let m = medir(&ctx.layer_painter(capa), &e.n, ancho);
        let recorrido = m.alto + 36.0;
        let escondido = deslizar(ctx, &mut e.posicion, &mut e.arrastre, &mut e.visible, recorrido, ahora);
        if !e.visible && escondido >= 1.0 {
            self.emergente = None;
            return;
        }
        if e.visible {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((e.vence - ahora).max(0.0)));
        }
        let y = area.min.y + 12.0 - escondido * recorrido;
        let r = Rect::from_min_size(Pos2::new(area.center().x - ancho / 2.0, y), Vec2::new(ancho, m.alto));
        e.rect = r;
        egui::Area::new(capa.id).order(Order::Foreground).fixed_pos(r.min).constrain(false).fade_in(false).show(ctx, |ui| {
            if !interactivo {
                ui.disable();
            }
            let p = ui.painter_at(area);
            p.add(Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(120) }.as_shape(r, 12.0));
            let sentido = if interactivo { Sense::click_and_drag() } else { Sense::hover() };
            let (resp, toque) = tarjeta(ui, &self.texturas, r, &e.n, m, hora24, sentido);
            p.rect_stroke(r, 12.0, Stroke::new(1.0, FONDO_HOVER), StrokeKind::Inside);
            if toque.is_some() {
                e.visible = false;
            }
            // Mientras el mouse está encima (o la arrastra) no se va; arrastrándola hacia arriba, sí.
            if interactivo && (ui.rect_contains_pointer(r) || e.arrastre.is_some()) {
                e.vence = e.vence.max(ahora + 1.5);
            }
            if resp.drag_started() {
                e.arrastre = Some((escondido, escondido));
            }
        });
    }

    /// Si encima de `pos` está la notificación emergente (ese toque es para ella, no para Android).
    pub fn sobre_emergente(&self, pos: Pos2) -> bool {
        self.emergente.as_ref().is_some_and(|e| e.visible && e.rect.contains(pos))
    }

    /// Busca, en lo recién leído, una notificación que haya avisado después de la última mostrada.
    fn revisar_alertas(&mut self, d: &Datos, ahora: f64) {
        let ultima = self.ultima_alerta.unwrap_or(0);
        if let Some(n) = d.notificaciones.iter().filter(|n| n.alerta > ultima).max_by_key(|n| n.alerta) {
            self.ultima_alerta = Some(n.alerta);
            if !self.abierto {
                match &mut self.emergente {
                    // Si ya hay una a la vista, la nueva toma su lugar sin volver a bajar.
                    Some(e) if e.visible => {
                        e.n = n.clone();
                        e.vence = ahora + SEGUNDOS_EMERGENTE;
                    }
                    _ => {
                        self.emergente = Some(Emergente {
                            n: n.clone(),
                            visible: true,
                            posicion: Deslizar::quieto(1.0),
                            vence: ahora + SEGUNDOS_EMERGENTE,
                            arrastre: None,
                            rect: Rect::NOTHING,
                        });
                    }
                }
                return;
            }
        }
        // La que está a la vista: con su texto al día, o se va si ya la quitaron.
        if let Some(e) = &mut self.emergente {
            match d.notificaciones.iter().find(|n| n.clave == e.n.clave) {
                Some(n) => e.n = n.clone(),
                None => e.visible = false,
            }
        }
    }
}

/// Lo escondido (0 a 1) de algo que se desliza hacia arriba. Mientras se arrastra (`arrastre` = lo
/// escondido al empezar y lo último calculado) sigue al mouse, con `recorrido` = puntos para
/// esconderlo del todo. Al soltar decide un tirón rápido o, si no, haberlo movido un cuarto desde
/// donde empezó; después va solo hacia la vista o hacia arriba, según `visible`.
fn deslizar(ctx: &egui::Context, posicion: &mut Deslizar, arrastre: &mut Option<(f32, f32)>, visible: &mut bool, recorrido: f32, ahora: f64) -> f32 {
    if let Some((base, ultimo)) = *arrastre {
        if ctx.input(|i| i.pointer.primary_down()) {
            if let Some(d) = ctx.input(|i| i.pointer.total_drag_delta()) {
                let valor = (base - d.y / recorrido).clamp(0.0, 1.0);
                *arrastre = Some((base, valor));
                return valor;
            }
            return ultimo;
        }
        *arrastre = None;
        let velocidad = ctx.input(|i| i.pointer.velocity().y);
        let esconder = if velocidad.abs() > VELOCIDAD_CIERRE {
            velocidad < 0.0
        } else if base < 0.5 {
            ultimo > 0.25
        } else {
            ultimo > 0.75
        };
        *visible = !esconder;
        *posicion = Deslizar::quieto(ultimo);
    }
    let meta = if *visible { 0.0 } else { 1.0 };
    if posicion.hasta != meta {
        posicion.ir(meta, ahora);
    }
    if !posicion.terminado(ahora) {
        ctx.request_repaint();
    }
    posicion.valor(ahora)
}

/// Dónde va la tarjeta `j` de una fila de tarjetas de ancho `w`.
fn tarjeta_r(fila: Rect, j: usize, w: f32, alto: f32) -> Rect {
    Rect::from_min_size(Pos2::new(fila.min.x + j as f32 * (w + SEP), fila.min.y), Vec2::new(w, alto))
}

/// Una notificación en su tarjeta: tocarla la abre; con el mouse encima, la X la quita.
fn tarjeta(
    ui: &mut egui::Ui,
    texturas: &HashMap<String, TextureHandle>,
    r: Rect,
    n: &Notificacion,
    m: Medida,
    hora24: bool,
    sentido: Sense,
) -> (egui::Response, Option<Toque>) {
    let resp = ui.allocate_rect(r, sentido);
    let p = ui.painter().clone();
    let encima = ui.rect_contains_pointer(r);
    p.rect_filled(r, 12.0, if encima { FONDO_CONTROL } else { FONDO });
    if let Some(t) = texturas.get(&n.icono) {
        egui::Image::from_texture(t).corner_radius(8.0).paint_at(ui, Rect::from_min_size(r.min + Vec2::new(14.0, 12.0), Vec2::splat(36.0)));
    }
    let texto_x = r.min.x + 62.0;
    let arriba = r.min.y + 12.0;
    p.galley(Pos2::new(texto_x, arriba), m.titulo.clone(), TEXTO);
    p.galley(Pos2::new(texto_x, arriba + m.titulo.size().y + 3.0), m.texto, TENUE);
    let mut toque = None;
    // App y hora a la derecha; con el mouse encima, la X para quitarla.
    if n.descartable && encima {
        let x = Rect::from_center_size(Pos2::new(r.max.x - 20.0, arriba + 9.0), Vec2::splat(28.0));
        if boton_icono(ui, &p, x, ic::X, "Quitar") {
            Panel::pedir("DESCARTAR", Some(&n.clave), None);
            toque = Some(Toque::Quitar);
        }
    } else {
        let detalle = galeria(&p, &format!("{} · {}", n.app, formato_hora(n.hora, hora24)), 11.0, TENUE, 150.0, 1);
        p.galley(Pos2::new(r.max.x - 14.0 - detalle.size().x, arriba + 1.0), detalle, TENUE);
    }
    if !n.acciones.is_empty() {
        let mut x = texto_x;
        for (i, nombre) in n.acciones.iter().enumerate().take(3) {
            let etiqueta = galeria(&p, nombre, 12.0, ACENTO, 140.0, 1);
            let boton = Rect::from_min_size(Pos2::new(x, r.max.y - 12.0 - 28.0), Vec2::new(etiqueta.size().x + 24.0, 28.0));
            let b = ui.allocate_rect(boton, Sense::click());
            p.rect_filled(boton, 14.0, if b.hovered() { FONDO_HOVER } else { FONDO_OSCURO });
            p.galley(boton.center() - etiqueta.size() / 2.0, etiqueta, ACENTO);
            if b.clicked() {
                Panel::pedir("ACCION", Some(&n.clave), Some(i));
                toque = Some(Toque::Abrir);
            }
            x = boton.max.x + 8.0;
        }
    }
    if toque.is_none() && resp.clicked() {
        Panel::pedir("ABRIR", Some(&n.clave), None);
        toque = Some(Toque::Abrir);
    }
    (resp, toque)
}

/// Textos de una tarjeta ya acomodados, y su alto.
struct Medida {
    titulo: Arc<egui::Galley>,
    texto: Arc<egui::Galley>,
    alto: f32,
}

fn medir(p: &egui::Painter, n: &Notificacion, ancho: f32) -> Medida {
    let ancho_texto = ancho - 62.0 - 14.0;
    let titulo = galeria(p, &n.titulo, 13.5, TEXTO, ancho_texto - 90.0, 1);
    let texto = galeria(p, &n.texto, 12.0, TENUE, ancho_texto, 3);
    let acciones = if n.acciones.is_empty() { 0.0 } else { 38.0 };
    let alto = (12.0 + titulo.size().y + 3.0 + texto.size().y + acciones + 12.0).max(60.0);
    Medida { titulo, texto, alto }
}

fn galeria(p: &egui::Painter, texto: &str, tam: f32, color: Color32, ancho: f32, filas: usize) -> Arc<egui::Galley> {
    let mut trabajo = LayoutJob::single_section(texto.to_string(), TextFormat { font_id: FontId::proportional(tam), color, ..Default::default() });
    trabajo.wrap = TextWrapping { max_width: ancho.max(10.0), max_rows: filas, break_anywhere: false, overflow_character: Some('…') };
    p.layout_job(trabajo)
}

fn boton_icono(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, glifo: &str, ayuda: &str) -> bool {
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_text(ayuda);
    if resp.hovered() {
        p.circle_filled(r.center(), r.width() / 2.0, FONDO_HOVER);
    }
    p.text(r.center(), Align2::CENTER_CENTER, glifo, FontId::proportional(17.0), ICONO);
    resp.clicked()
}

fn boton_texto(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, glifo: &str, texto: &str) -> bool {
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    p.rect_filled(r, 17.0, if resp.hovered() { FONDO_HOVER } else { FONDO_CONTROL });
    p.text(r.center(), Align2::CENTER_CENTER, format!("{glifo}  {texto}"), FontId::proportional(13.0), TEXTO);
    resp.clicked()
}

fn ahora_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Ahora (ms) y el día de la semana, el mes y el día del mes.
fn ahora() -> (i64, &'static str, &'static str, i32) {
    let ms = ahora_ms();
    let tm = local(ms);
    (ms, DIAS[tm.tm_wday.clamp(0, 6) as usize], MESES[tm.tm_mon.clamp(0, 11) as usize], tm.tm_mday)
}

fn local(ms: i64) -> libc::tm {
    let segundos = (ms / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&segundos, &mut tm) };
    tm
}

/// «23:05» o «11:05 p. m.», como lo tenga Android.
fn formato_hora(ms: i64, hora24: bool) -> String {
    let tm = local(ms);
    if hora24 {
        format!("{}:{:02}", tm.tm_hour, tm.tm_min)
    } else {
        let h = if tm.tm_hour % 12 == 0 { 12 } else { tm.tm_hour % 12 };
        format!("{h}:{:02} {}", tm.tm_min, if tm.tm_hour < 12 { "a. m." } else { "p. m." })
    }
}

/// Lee los interruptores, el volumen y las notificaciones con sus íconos.
fn leer() -> (Datos, Vec<(String, ColorImage)>) {
    let mut d = Datos::default();
    let estado = android::shell(
        "settings get global airplane_mode_on; settings get global zen_mode; settings get system accelerometer_rotation; \
         settings get system time_12_24; cmd media_session volume --stream 3 --get 2>/dev/null | tail -1",
    )
    .unwrap_or_default();
    let campos: Vec<&str> = estado.lines().map(str::trim).collect();
    let campo = |i: usize| campos.get(i).copied().unwrap_or("");
    d.avion = campo(0) == "1";
    d.no_molestar = !matches!(campo(1), "0" | "" | "null");
    d.girar = campo(2) == "1";
    d.hora24 = campo(3) == "24";
    // «volume is 7 in range [0..15]»
    let numeros: Vec<u32> = campo(4).split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    if let [actual, _, maximo, ..] = numeros[..] {
        d.volumen = actual;
        d.volumen_max = maximo;
    }

    let json = android::shell(&format!("cat {EXPORTADO}/notificaciones.json")).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    for n in json.as_ref().and_then(|v| v.as_array()).into_iter().flatten() {
        let texto = |k: &str| n[k].as_str().unwrap_or("").to_string();
        d.notificaciones.push(Notificacion {
            clave: texto("clave"),
            app: texto("app"),
            titulo: texto("titulo"),
            texto: texto("texto"),
            hora: n["hora"].as_i64().unwrap_or(0),
            descartable: n["descartable"].as_bool().unwrap_or(false),
            acciones: n["acciones"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(String::from)).collect(),
            icono: texto("icono"),
            alerta: n["alerta"].as_i64().unwrap_or(0),
        });
    }
    // Las más nuevas arriba.
    d.notificaciones.sort_by(|a, b| b.hora.cmp(&a.hora));

    let mut iconos = Vec::new();
    let carpeta = carpeta_iconos();
    let _ = std::fs::remove_dir_all(&carpeta);
    if let Some(padre) = carpeta.parent() {
        let _ = std::fs::create_dir_all(padre);
    }
    if !d.notificaciones.is_empty() && android::copiar_desde(&format!("{EXPORTADO}/notificaciones"), &carpeta) {
        for n in &d.notificaciones {
            if let Some(imagen) = crate::ajustes_android::leer_png(&carpeta.join(&n.icono)) {
                iconos.push((n.icono.clone(), imagen));
            }
        }
    }
    (d, iconos)
}

fn carpeta_iconos() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"))
        .join("zykro/notificaciones")
}
