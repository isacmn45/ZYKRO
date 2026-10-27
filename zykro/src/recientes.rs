//! Apps abiertas, como la vista de recientes de Android: la app que estás usando se encoge hasta ser
//! una tarjeta (sigue en vivo) y las anteriores quedan a su izquierda. Tocar una tarjeta la agranda y
//! vuelve a esa app; arrastrarla hacia arriba la cierra; tocar el fondo lleva a la pantalla de inicio.
//! Android no tiene vista de recientes (Quickstep está desactivado), así que la dibuja ZYKRO: las
//! tarjetas de las apps de atrás usan la última imagen que vio de cada una.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, emath::easing, Align2, Color32, ColorImage, FontId, Id, Order, Pos2, Rect, Sense, TextureHandle, TextureId, TextureOptions, Vec2};
use egui_phosphor::regular as ic;

use crate::android::{self, Tarea};
use crate::video::Cuadro;
use crate::{CERRAR_HOVER, FONDO_CONTROL, FONDO_HOVER, FONDO_OSCURO, ICONO, TENUE, TEXTO};

/// Ancho de las miniaturas guardadas (el alto sale de la proporción de la pantalla).
const ANCHO_MINIATURA: usize = 384;
/// Lo que tarda la app en encogerse a tarjeta (y en volver a crecer).
const DURACION: f32 = 0.25;
/// Tamaño de las tarjetas frente a la imagen completa.
const ESCALA: f32 = 0.62;
const SEPARACION: f32 = 36.0;
const REDONDEO: f32 = 14.0;
/// Lo que dura una tarjeta saliendo hacia arriba al cerrarla.
const SALIDA: Duration = Duration::from_millis(200);

/// Lo que la ventana tiene que hacer después de dibujar el selector.
pub enum Accion {
    /// Ir a la pantalla de inicio de Android.
    Inicio,
}

#[derive(Clone, Copy, PartialEq)]
enum Eje {
    Horizontal,
    Vertical,
}

/// Una tarjeta: la tarea de Android (falta mientras se pregunta la lista) y si es la app en vivo.
struct Tarjeta {
    tarea: Option<Tarea>,
    paquete: String,
    vivo: bool,
}

impl Tarjeta {
    /// Clave para sus animaciones.
    fn clave(&self) -> u32 {
        self.tarea.as_ref().map_or(0, |t| t.raiz)
    }
}

#[derive(Default)]
pub struct Recientes {
    /// Hay algo en pantalla (también mientras la tarjeta vuelve a crecer al cerrar).
    abierto: bool,
    /// Se va hacia mostrado (`false` = cerrándose).
    visible: bool,
    /// `None` mientras se pregunta a Android.
    lista: Arc<Mutex<Option<Vec<Tarea>>>>,
    /// La app que estaba al frente al abrir: su tarjeta es la imagen en vivo.
    actual: Option<String>,
    /// Al cerrar, la tarjeta que crece hasta la pantalla completa (si no, ninguna: se va al inicio).
    elegida: Option<String>,
    /// Desplazamiento horizontal en px: positivo trae a la vista las apps más viejas (a la izquierda).
    desplazamiento: f32,
    /// Hacia dónde va el arrastre en curso (lo decide el primer movimiento).
    eje: Option<Eje>,
    /// Cuánto se subió cada tarjeta (por tarea raíz) y las que se están yendo.
    subidas: HashMap<u32, f32>,
    saliendo: HashMap<u32, Instant>,
    /// Última imagen de cada app (por paquete) y si falta subirla a la GPU.
    miniaturas: HashMap<String, (ColorImage, bool)>,
    texturas: HashMap<String, TextureHandle>,
}

impl Recientes {
    pub fn abierto(&self) -> bool {
        self.abierto
    }

    /// El botón de recientes: abre, o vuelve a la app que estaba al frente.
    pub fn alternar(&mut self, ctx: &egui::Context) {
        if self.abierto {
            if self.visible {
                self.elegida = self.actual.clone();
                self.visible = false;
            }
            return;
        }
        self.abierto = true;
        self.visible = true;
        self.actual = android::app_activa().filter(|p| p != android::INICIO);
        self.elegida = None;
        self.desplazamiento = 0.0;
        self.eje = None;
        self.subidas.clear();
        self.saliendo.clear();
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

    /// Tarjetas de la más reciente a la más vieja; la app en vivo, primero.
    fn tarjetas(&self) -> Vec<Tarjeta> {
        let lista = self.lista.lock().unwrap().clone();
        let Some(tareas) = lista else {
            // Mientras Android contesta, solo la app que estaba al frente.
            return self.actual.iter().map(|p| Tarjeta { tarea: None, paquete: p.clone(), vivo: true }).collect();
        };
        let mut tarjetas: Vec<Tarjeta> = tareas
            .into_iter()
            .map(|t| Tarjeta { vivo: self.actual.as_ref() == Some(&t.paquete), paquete: t.paquete.clone(), tarea: Some(t) })
            .collect();
        if let Some(i) = tarjetas.iter().position(|t| t.vivo) {
            let viva = tarjetas.remove(i);
            tarjetas.insert(0, viva);
            tarjetas.iter_mut().skip(1).for_each(|t| t.vivo = false);
        }
        tarjetas
    }

    /// Dibuja el selector sobre `area` (la zona del video); `imagen` es donde está la imagen de
    /// Android y `video` su textura en vivo.
    pub fn mostrar(&mut self, ctx: &egui::Context, area: Rect, imagen: Rect, video: Option<TextureId>) -> Option<Accion> {
        if !self.abierto {
            return None;
        }
        self.subir_miniaturas(ctx);
        let t = ctx.animate_bool_with_time_and_easing(Id::new("recientes-t"), self.visible, DURACION, easing::cubic_out);
        if !self.visible && t <= 0.0 {
            self.abierto = false;
            return None;
        }
        let quieto = self.visible && t >= 1.0;
        let mut accion = None;

        // Las tarjetas que terminaron de salir se quitan de la lista.
        let ahora = Instant::now();
        let fuera: Vec<u32> = self.saliendo.iter().filter(|(_, desde)| ahora - **desde >= SALIDA).map(|(r, _)| *r).collect();
        if !fuera.is_empty() {
            if let Some(tareas) = self.lista.lock().unwrap().as_mut() {
                tareas.retain(|t| !fuera.contains(&t.raiz));
            }
            fuera.iter().for_each(|r| {
                self.saliendo.remove(r);
            });
        }
        if !self.saliendo.is_empty() {
            ctx.request_repaint();
        }

        let tarjetas = self.tarjetas();
        let lista_lista = self.lista.lock().unwrap().is_some();
        if quieto && lista_lista && tarjetas.is_empty() {
            // Se cerró la última app: a la pantalla de inicio.
            self.visible = false;
            self.elegida = None;
            accion = Some(Accion::Inicio);
        }

        let tam = imagen.size() * ESCALA;
        let paso = tam.x + SEPARACION;
        let maximo = (tarjetas.len().saturating_sub(1)) as f32 * paso;
        if quieto {
            let rueda = ctx.input(|i| i.smooth_scroll_delta);
            self.desplazamiento += rueda.x + rueda.y;
        }
        self.desplazamiento = self.desplazamiento.clamp(0.0, maximo);
        let centro_y = area.center().y - 16.0;
        let foco = if self.visible { self.actual.clone() } else { self.elegida.clone() };

        let mut salir = quieto && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut abrir: Option<String> = None;
        let mut cerrar: Vec<Tarea> = Vec::new();
        let mut al_inicio = false;

        egui::Area::new(Id::new("recientes")).order(Order::Foreground).fixed_pos(area.min).show(ctx, |ui| {
            let p = ui.painter().clone();
            // Fondo opaco: la app ya no se ve detrás, va en su tarjeta.
            p.rect_filled(area, 0.0, FONDO_OSCURO.gamma_multiply(t));
            let fondo = ui.allocate_rect(area, if quieto { Sense::click_and_drag() } else { Sense::hover() });
            if fondo.clicked() {
                al_inicio = true;
            }
            if fondo.dragged() {
                self.desplazamiento = (self.desplazamiento + fondo.drag_delta().x).clamp(0.0, maximo);
            }

            if lista_lista && tarjetas.is_empty() {
                p.text(area.center(), Align2::CENTER_CENTER, "No hay apps abiertas", FontId::proportional(15.0), TENUE.gamma_multiply(t));
                return;
            }

            for (i, tarjeta) in tarjetas.iter().enumerate() {
                let clave = tarjeta.clave();
                // La posición se anima para que, al cerrar una, las demás se corran suaves.
                let x_meta = area.center().x - i as f32 * paso + self.desplazamiento;
                let x = ctx.animate_value_with_time(Id::new(("recientes-x", clave)), x_meta, if self.eje.is_some() { 0.0 } else { 0.18 });
                let destino = Rect::from_center_size(Pos2::new(x, centro_y), tam);
                let es_foco = foco.as_deref() == Some(tarjeta.paquete.as_str()) && (tarjeta.vivo || !self.visible);

                // Arrastrada hacia arriba, o saliendo después de soltarla.
                let arrastrando = self.eje == Some(Eje::Vertical);
                let subida = *self.subidas.get(&clave).unwrap_or(&0.0);
                let mut dy = ctx.animate_value_with_time(Id::new(("recientes-y", clave)), subida, if arrastrando { 0.0 } else { 0.15 });
                let mut opacidad = t;
                if let Some(desde) = self.saliendo.get(&clave) {
                    let avance = ((ahora - *desde).as_secs_f32() / SALIDA.as_secs_f32()).min(1.0);
                    dy = subida - avance * area.height();
                    opacidad *= 1.0 - avance;
                }

                // La tarjeta en foco viaja entre la pantalla completa y su lugar; las demás aparecen.
                let rect = if es_foco { lerp_rect(imagen, destino, t) } else { destino.translate(Vec2::new(0.0, dy)) };
                let redondeo = if es_foco { REDONDEO * t } else { REDONDEO };
                if !es_foco {
                    opacidad *= t;
                }
                if rect.max.x < area.min.x - 10.0 || rect.min.x > area.max.x + 10.0 {
                    continue;
                }

                // Nombre arriba, como en Android.
                let nombre = nombre_app(&tarjeta.paquete);
                let titulo = Pos2::new(rect.center().x, rect.min.y - 16.0);
                p.text(titulo, Align2::CENTER_CENTER, &nombre, FontId::proportional(14.0), TEXTO.gamma_multiply(opacidad * t));

                let tinte = Color32::WHITE.gamma_multiply(if es_foco { 1.0 } else { opacidad });
                match (tarjeta.vivo, video, self.texturas.get(&tarjeta.paquete)) {
                    (true, Some(id), _) => {
                        egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
                            .corner_radius(redondeo)
                            .tint(tinte)
                            .paint_at(ui, rect);
                    }
                    (_, _, Some(textura)) => {
                        egui::Image::from_texture(textura).corner_radius(redondeo).tint(tinte).paint_at(ui, rect);
                    }
                    _ => {
                        p.rect_filled(rect, redondeo, FONDO_CONTROL.gamma_multiply(opacidad));
                        let inicial = nombre.chars().next().unwrap_or('?').to_uppercase().to_string();
                        p.circle_filled(rect.center(), rect.height() * 0.16, FONDO_HOVER.gamma_multiply(opacidad));
                        let letra = FontId::proportional(rect.height() * 0.16);
                        p.text(rect.center(), Align2::CENTER_CENTER, inicial, letra, ICONO.gamma_multiply(opacidad));
                    }
                }

                if !quieto || self.saliendo.contains_key(&clave) {
                    continue;
                }
                let resp = ui.allocate_rect(rect, Sense::click_and_drag());
                if resp.drag_started() {
                    self.eje = None;
                }
                if resp.dragged() {
                    let d = resp.drag_delta();
                    if self.eje.is_none() && d.length() > 0.0 {
                        self.eje = Some(if d.y.abs() > d.x.abs() { Eje::Vertical } else { Eje::Horizontal });
                    }
                    match self.eje {
                        // Hacia abajo cuesta (no lleva a nada); hacia arriba sigue al mouse.
                        Some(Eje::Vertical) => {
                            let y = self.subidas.entry(clave).or_insert(0.0);
                            *y = (*y + if *y + d.y > 0.0 { d.y * 0.25 } else { d.y }).min(tam.y * 0.15);
                        }
                        Some(Eje::Horizontal) => self.desplazamiento = (self.desplazamiento + d.x).clamp(0.0, maximo),
                        None => {}
                    }
                }
                if resp.drag_stopped() {
                    let subida = self.subidas.remove(&clave).unwrap_or(0.0);
                    if self.eje == Some(Eje::Vertical) && subida < -tam.y * 0.25 {
                        if let Some(tarea) = &tarjeta.tarea {
                            self.subidas.insert(clave, subida);
                            self.saliendo.insert(clave, Instant::now());
                            cerrar.push(tarea.clone());
                        }
                    }
                    self.eje = None;
                } else if resp.clicked() {
                    abrir = Some(tarjeta.paquete.clone());
                }
            }

            // «Borrar todo», debajo de las tarjetas.
            if lista_lista && !tarjetas.is_empty() {
                let boton = Rect::from_center_size(Pos2::new(area.center().x, centro_y + tam.y / 2.0 + 38.0), Vec2::new(132.0, 32.0));
                let resp = ui.allocate_rect(boton, if quieto { Sense::click() } else { Sense::hover() });
                let color = if resp.hovered() { CERRAR_HOVER } else { FONDO_CONTROL };
                p.rect_filled(boton, 16.0, color.gamma_multiply(t));
                p.text(boton.center(), Align2::CENTER_CENTER, format!("{}  Borrar todo", ic::X), FontId::proportional(13.0), TEXTO.gamma_multiply(t));
                if resp.clicked() {
                    cerrar = tarjetas.iter().filter_map(|t| t.tarea.clone()).collect();
                    al_inicio = true;
                }
            }
        });

        if !cerrar.is_empty() {
            thread::spawn(move || cerrar.iter().for_each(|t| android::cerrar_tarea(t.raiz)));
        }
        if let Some(paquete) = abrir {
            // La tarjeta crece hasta la pantalla completa mientras Android trae la app al frente.
            if self.actual.as_ref() != Some(&paquete) {
                let paquete = paquete.clone();
                thread::spawn(move || android::abrir_app(&paquete));
            }
            self.elegida = Some(paquete);
            salir = false;
            self.visible = false;
        }
        if salir {
            self.elegida = self.actual.clone();
            self.visible = false;
        }
        if al_inicio {
            self.elegida = None;
            self.visible = false;
            accion = Some(Accion::Inicio);
        }
        accion
    }

    /// Sube a la GPU las miniaturas nuevas.
    fn subir_miniaturas(&mut self, ctx: &egui::Context) {
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
    }
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min + (b.min - a.min) * t, a.max + (b.max - a.max) * t)
}

/// Nombre para mostrar. Android no da las etiquetas de las apps por la terminal, así que se usa una
/// tabla con las conocidas y, si no, la última parte del paquete.
pub(crate) fn nombre_app(paquete: &str) -> String {
    let conocidas = [
        ("com.dts.freefireth", "Free Fire"),
        ("com.dts.freefiremax", "Free Fire MAX"),
        ("org.cromite.cromite", "Cromite"),
        ("com.android.chrome", "Chrome"),
        ("com.android.settings", "Ajustes"),
        ("com.aurora.store", "Aurora Store"),
        ("com.zykro.inicio", "Inicio de ZYKRO"),
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
