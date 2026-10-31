//! Panel de notificaciones al estilo de Android 4 (KitKat): oscuro y compacto, con una fila de
//! interruptores con su nombre, el volumen y la lista de notificaciones. Lo dibuja ZYKRO en vez del
//! de Android, que gasta mucho espacio; se baja arrastrando desde el borde de arriba de la imagen.
//! Las notificaciones las exporta la pantalla de inicio de ZYKRO (`Escucha.java`), y ZYKRO le pide
//! abrirlas o quitarlas con `am broadcast` (`Acciones.java`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, emath::easing, text::LayoutJob, text::TextWrapping, Align2, Color32, ColorImage, FontId, Id, Order, Pos2, Rect, Sense,
    Stroke, TextFormat, TextureHandle, TextureOptions, Vec2,
};
use egui_phosphor::regular as ic;

use crate::android;

/// Carpeta de Android donde la pantalla de inicio deja las notificaciones y sus íconos.
const EXPORTADO: &str = "/data/media/0/Android/data/com.zykro.inicio/files";

// Colores de Android 4 (Holo oscuro).
const FONDO: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);
const ENCIMA: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);
const LINEA: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
const HOLO: Color32 = Color32::from_rgb(0x33, 0xb5, 0xe5);
const BLANCO: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf2);
const GRIS: Color32 = Color32::from_rgb(0x9e, 0x9e, 0x9e);
const APAGADO: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);

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

#[derive(Default)]
pub struct Panel {
    abierto: bool,
    /// Bajando (o abierto); `false` = subiendo para cerrarse.
    visible: bool,
    datos: Arc<Mutex<Option<Datos>>>,
    iconos_nuevos: Arc<Mutex<Vec<(String, ColorImage)>>>,
    texturas: HashMap<String, TextureHandle>,
    leyendo: Arc<AtomicBool>,
    /// Volumen mientras se arrastra la barra (se aplica al soltar).
    volumen: Option<f32>,
}

impl Panel {
    pub fn abierto(&self) -> bool {
        self.abierto
    }

    pub fn abrir(&mut self, ctx: &egui::Context) {
        if !self.abierto {
            self.abierto = true;
            self.visible = true;
        }
        self.recargar(ctx);
    }

    pub fn cerrar(&mut self) {
        self.visible = false;
    }

    /// Vuelve a leer las notificaciones y los interruptores (en otro hilo).
    pub fn recargar(&self, ctx: &egui::Context) {
        if self.leyendo.swap(true, Ordering::Relaxed) {
            return;
        }
        let (datos, iconos, leyendo, ctx) = (self.datos.clone(), self.iconos_nuevos.clone(), self.leyendo.clone(), ctx.clone());
        thread::spawn(move || {
            let (leidos, nuevos) = leer();
            *datos.lock().unwrap() = Some(leidos);
            *iconos.lock().unwrap() = nuevos;
            leyendo.store(false, Ordering::Relaxed);
            ctx.request_repaint();
        });
    }

    /// Corre `orden` en Android y vuelve a leer.
    fn aplicar(&self, ctx: &egui::Context, orden: String) {
        let (datos, iconos, ctx) = (self.datos.clone(), self.iconos_nuevos.clone(), ctx.clone());
        thread::spawn(move || {
            android::shell(&orden);
            let (leidos, nuevos) = leer();
            *datos.lock().unwrap() = Some(leidos);
            *iconos.lock().unwrap() = nuevos;
            ctx.request_repaint();
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

    /// Dibuja el panel sobre `area` (la zona del video).
    pub fn mostrar(&mut self, ctx: &egui::Context, area: Rect) -> Option<Accion> {
        if !self.abierto {
            return None;
        }
        for (archivo, imagen) in self.iconos_nuevos.lock().unwrap().drain(..) {
            self.texturas.insert(archivo.clone(), ctx.load_texture(format!("notificacion-{archivo}"), imagen, TextureOptions::LINEAR));
        }
        let t = ctx.animate_bool_with_time_and_easing(Id::new("panel-t"), self.visible, 0.18, easing::cubic_out);
        if !self.visible && t <= 0.0 {
            self.abierto = false;
            return None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cerrar();
        }
        let datos = self.datos.lock().unwrap().clone();
        let mut accion = None;

        let ancho = (area.width() * 0.42).clamp(380.0, 560.0).min(area.width());
        let alto = area.height();
        let panel = Rect::from_min_size(Pos2::new(area.center().x - ancho / 2.0, area.min.y - (1.0 - t) * alto), Vec2::new(ancho, alto));

        egui::Area::new(Id::new("panel")).order(Order::Foreground).fixed_pos(area.min).show(ctx, |ui| {
            let p = ui.painter().clone();
            // Fuera del panel, oscurece y cierra al tocar.
            p.rect_filled(area, 0.0, Color32::from_black_alpha((110.0 * t) as u8));
            if ui.allocate_rect(area, Sense::click()).clicked() {
                self.cerrar();
            }
            let fondo = ui.allocate_rect(panel, Sense::click());
            p.rect_filled(panel, 0.0, FONDO);
            let _ = fondo;
            let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(panel));
            let x0 = panel.min.x;
            let mut y = panel.min.y;

            // Hora y fecha, y a la derecha los ajustes y «borrar todo».
            let (ahora, dia, mes, num) = ahora();
            let hora = formato_hora(ahora, datos.as_ref().is_some_and(|d| d.hora24));
            p.text(Pos2::new(x0 + 14.0, y + 26.0), Align2::LEFT_CENTER, &hora, FontId::proportional(24.0), BLANCO);
            let ancho_hora = p.layout_no_wrap(hora.clone(), FontId::proportional(24.0), BLANCO).size().x;
            let fecha = format!("{dia}, {num} de {mes}");
            p.text(Pos2::new(x0 + 24.0 + ancho_hora, y + 29.0), Align2::LEFT_CENTER, fecha, FontId::proportional(12.5), GRIS);
            let borrar = Rect::from_center_size(Pos2::new(panel.max.x - 22.0, y + 26.0), Vec2::splat(34.0));
            if boton_icono(ui, borrar, ic::BROOM, "Borrar todo") {
                Panel::pedir("BORRAR_TODO", None, None);
            }
            y += 52.0;
            linea(&p, panel, y);

            // Interruptores, con su nombre debajo (encendido = celeste).
            let d = datos.clone().unwrap_or_default();
            let interruptores: [(&str, &str, Option<bool>); 6] = [
                (ic::AIRPLANE, "Avión", Some(d.avion)),
                (ic::MOON, "No molestar", Some(d.no_molestar)),
                (ic::ARROWS_CLOCKWISE, "Girar", Some(d.girar)),
                (ic::CAMERA, "Captura", None),
                (ic::GEAR, "Ajustes", None),
                (ic::ANDROID_LOGO, "Android", None),
            ];
            let celda = ancho / interruptores.len() as f32;
            for (i, (glifo, nombre, estado)) in interruptores.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * celda, y), Vec2::new(celda, 62.0));
                let resp = ui.allocate_rect(r, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.hovered() {
                    p.rect_filled(r, 0.0, ENCIMA);
                }
                let color = if estado == &Some(true) { HOLO } else { APAGADO };
                p.text(Pos2::new(r.center().x, r.min.y + 22.0), Align2::CENTER_CENTER, *glifo, FontId::proportional(21.0), color);
                p.text(Pos2::new(r.center().x, r.min.y + 46.0), Align2::CENTER_CENTER, *nombre, FontId::proportional(11.0), color);
                if i > 0 {
                    p.line_segment([Pos2::new(r.min.x, r.min.y + 14.0), Pos2::new(r.min.x, r.max.y - 14.0)], Stroke::new(1.0, LINEA));
                }
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
            y += 62.0;
            linea(&p, panel, y);

            // Volumen de la música y los juegos (el brillo no hace nada en un emulador).
            let fila = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(ancho, 40.0));
            p.text(Pos2::new(x0 + 22.0, fila.center().y), Align2::CENTER_CENTER, ic::SPEAKER_HIGH, FontId::proportional(18.0), APAGADO);
            let barra = Rect::from_min_max(Pos2::new(x0 + 46.0, fila.center().y - 12.0), Pos2::new(panel.max.x - 18.0, fila.center().y + 12.0));
            let resp = ui.allocate_rect(barra, Sense::click_and_drag());
            let maximo = d.volumen_max.max(1) as f32;
            if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked() || resp.drag_started()) {
                self.volumen = Some(((pos.x - barra.min.x) / barra.width()).clamp(0.0, 1.0) * maximo);
            }
            let valor = self.volumen.unwrap_or(d.volumen as f32) / maximo;
            let pista = Rect::from_center_size(barra.center(), Vec2::new(barra.width(), 3.0));
            p.rect_filled(pista, 1.5, LINEA);
            p.rect_filled(Rect::from_min_size(pista.min, Vec2::new(pista.width() * valor, 3.0)), 1.5, HOLO);
            p.circle_filled(Pos2::new(pista.min.x + pista.width() * valor, pista.center().y), 7.0, HOLO);
            if resp.drag_stopped() || resp.clicked() {
                if let Some(v) = self.volumen.take() {
                    self.aplicar(ctx, format!("cmd media_session volume --stream 3 --set {}", v.round() as u32));
                }
            }
            y += 40.0;
            linea(&p, panel, y);

            // Notificaciones.
            let lista = Rect::from_min_max(Pos2::new(x0, y + 1.0), Pos2::new(panel.max.x, panel.max.y - 16.0));
            match &datos {
                None => {
                    p.text(lista.center_top() + Vec2::new(0.0, 40.0), Align2::CENTER_CENTER, "…", FontId::proportional(14.0), GRIS);
                }
                Some(d) if d.notificaciones.is_empty() => {
                    let centro = lista.center_top() + Vec2::new(0.0, 40.0);
                    p.text(centro, Align2::CENTER_CENTER, "Sin notificaciones", FontId::proportional(13.0), GRIS);
                }
                Some(d) => {
                    let mut hijo = ui.new_child(egui::UiBuilder::new().max_rect(lista));
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(lista.height()).show(&mut hijo, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        for n in &d.notificaciones {
                            if let Some(abrir) = self.fila(ui, n, d.hora24) {
                                if abrir {
                                    self.cerrar();
                                }
                            }
                        }
                    });
                }
            }

            // Asa de abajo: tocarla sube el panel.
            let asa = Rect::from_min_max(Pos2::new(x0, panel.max.y - 16.0), panel.max);
            p.rect_filled(Rect::from_center_size(asa.center(), Vec2::new(36.0, 3.0)), 1.5, GRIS);
            if ui.allocate_rect(asa, Sense::click()).clicked() {
                self.cerrar();
            }
        });
        accion
    }

    /// Una notificación. Devuelve `Some(true)` si se abrió (el panel se cierra).
    fn fila(&mut self, ui: &mut egui::Ui, n: &Notificacion, hora24: bool) -> Option<bool> {
        let ancho = ui.available_width();
        let texto_x = 58.0;
        let ancho_texto = ancho - texto_x - 14.0;
        let titulo = galeria(ui, &n.titulo, 13.5, BLANCO, ancho_texto - 70.0, 1);
        let texto = galeria(ui, &n.texto, 12.0, GRIS, ancho_texto, 2);
        let con_acciones = !n.acciones.is_empty();
        let alto = (12.0 + titulo.size().y + 2.0 + texto.size().y + if con_acciones { 32.0 } else { 0.0 } + 12.0).max(60.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ancho, alto), Sense::click());
        let p = ui.painter().clone();
        if resp.hovered() {
            p.rect_filled(r, 0.0, ENCIMA);
        }
        if let Some(t) = self.texturas.get(&n.icono) {
            let icono = Rect::from_min_size(r.min + Vec2::new(12.0, 12.0), Vec2::splat(36.0));
            egui::Image::from_texture(t).corner_radius(4.0).paint_at(ui, icono);
        }
        let arriba = r.min.y + 12.0;
        p.galley(Pos2::new(r.min.x + texto_x, arriba), titulo.clone(), BLANCO);
        p.galley(Pos2::new(r.min.x + texto_x, arriba + titulo.size().y + 2.0), texto.clone(), GRIS);
        // Hora y app a la derecha; con el mouse encima, la X para quitarla.
        let detalle = format!("{} · {}", n.app, formato_hora(n.hora, hora24));
        let x_der = Rect::from_center_size(Pos2::new(r.max.x - 18.0, arriba + 8.0), Vec2::splat(26.0));
        let mut resultado = None;
        if n.descartable && resp.hovered() {
            if boton_icono(ui, x_der, ic::X, "Quitar") {
                Panel::pedir("DESCARTAR", Some(&n.clave), None);
                resultado = Some(false);
            }
        } else {
            let detalle = galeria(ui, &detalle, 11.0, GRIS, 150.0, 1);
            p.galley(Pos2::new(r.max.x - 12.0 - detalle.size().x, arriba + 1.0), detalle, GRIS);
        }
        if con_acciones {
            let y = r.max.y - 34.0;
            let mut x = r.min.x + texto_x;
            for (i, nombre) in n.acciones.iter().enumerate().take(3) {
                let etiqueta = galeria(ui, &nombre.to_uppercase(), 11.5, BLANCO, 140.0, 1);
                let boton = Rect::from_min_size(Pos2::new(x - 8.0, y), Vec2::new(etiqueta.size().x + 16.0, 28.0));
                let b = ui.allocate_rect(boton, Sense::click());
                if b.hovered() {
                    ui.painter().rect_filled(boton, 2.0, LINEA);
                }
                ui.painter().galley(Pos2::new(x, boton.center().y - etiqueta.size().y / 2.0), etiqueta, BLANCO);
                if b.clicked() {
                    Panel::pedir("ACCION", Some(&n.clave), Some(i));
                    resultado = Some(true);
                }
                x = boton.max.x + 12.0;
            }
        }
        p.line_segment([Pos2::new(r.min.x + texto_x, r.max.y - 0.5), Pos2::new(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, LINEA));
        if resultado.is_none() && resp.clicked() {
            Panel::pedir("ABRIR", Some(&n.clave), None);
            resultado = Some(true);
        }
        resultado
    }
}

fn galeria(ui: &egui::Ui, texto: &str, tam: f32, color: Color32, ancho: f32, filas: usize) -> Arc<egui::Galley> {
    let mut trabajo = LayoutJob::single_section(texto.to_string(), TextFormat { font_id: FontId::proportional(tam), color, ..Default::default() });
    trabajo.wrap = TextWrapping { max_width: ancho.max(10.0), max_rows: filas, break_anywhere: false, overflow_character: Some('…') };
    ui.painter().layout_job(trabajo)
}

fn boton_icono(ui: &mut egui::Ui, r: Rect, glifo: &str, ayuda: &str) -> bool {
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_text(ayuda);
    if resp.hovered() {
        ui.painter().circle_filled(r.center(), r.width() / 2.0, LINEA);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, glifo, FontId::proportional(17.0), APAGADO);
    resp.clicked()
}

fn linea(p: &egui::Painter, panel: Rect, y: f32) {
    p.line_segment([Pos2::new(panel.min.x, y), Pos2::new(panel.max.x, y)], Stroke::new(1.0, LINEA));
}

/// Ahora (ms) y el día de la semana, el mes y el día del mes.
fn ahora() -> (i64, &'static str, &'static str, i32) {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
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
