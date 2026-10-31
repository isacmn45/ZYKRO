//! Controles del juego al estilo de BlueStacks: insignias semitransparentes sobre el juego y un
//! editor con panel lateral para agregar los controles arrastrándolos a la pantalla, ajustar sus
//! propiedades (las mismas que los «ajustes avanzados» de BlueStacks) y tener varios esquemas de
//! controles por juego.

use std::path::PathBuf;
use std::process::Command;

use eframe::egui::{self, Align, Align2, Color32, FontId, Id, LayerId, Layout, Order, Painter, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as ic;
use serde_json::Map;

use crate::mapeo::{
    self, etiqueta, Borde, Clic, Deslizar, Desplazar, Dpad, EstiloMira, Mapa, MiradaLibre, MobaDpad, MobaHabilidad, Nodo, Punto, Raton,
    Repetido, Rotar, RuedaNodo, ScriptNodo, Zoom,
};
use crate::{ACENTO, FONDO, FONDO_CONTROL, FONDO_HOVER, ICONO, TENUE, TEXTO};

/// Parte de un control que se puede tocar en el editor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Asa {
    /// El control entero: moverlo lo mueve todo.
    Nodo(usize),
    /// Otro punto del control (el fin de un deslizamiento, un paso de varios toques, el personaje del
    /// D-pad MOBA, el botón de cancelar, la rueda hacia abajo).
    Punto(usize, usize),
    /// Una de sus teclas: tocarla espera la tecla nueva.
    Tecla(usize, usize),
    /// El borde que cambia su tamaño.
    Radio(usize),
    Apuntar,
    Disparo,
    /// El botón del ojo de «mirar alrededor» dentro de la puntería.
    Ojo,
}

impl Asa {
    fn nodo(self) -> Option<usize> {
        match self {
            Asa::Nodo(i) | Asa::Punto(i, _) | Asa::Tecla(i, _) | Asa::Radio(i) => Some(i),
            _ => None,
        }
    }
}

/// A qué se le asigna la próxima tecla.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Objetivo {
    Tecla(usize, usize),
    Cambio,
    Pausa,
    Ojo,
}

#[derive(Clone, Copy, PartialEq)]
enum Tipo {
    Toque,
    Repetido,
    Dpad,
    Apuntar,
    Mirada,
    Script,
    Deslizar,
    Zoom,
    MobaDpad,
    Habilidad,
    Rotar,
    Borde,
    Desplazar,
    Rueda,
}

const TIPOS: [(Tipo, &str, &str); 14] = [
    (Tipo::Toque, ic::HAND_TAP, "Tocar el punto"),
    (Tipo::Repetido, ic::REPEAT, "Toques repetidos"),
    (Tipo::Dpad, ic::JOYSTICK, "D-pad"),
    (Tipo::Apuntar, ic::CROSSHAIR, "Apuntar y disparar"),
    (Tipo::Mirada, ic::EYE, "Mirada libre"),
    (Tipo::Script, ic::CODE, "Script"),
    (Tipo::Deslizar, ic::HAND_SWIPE_RIGHT, "Deslizar"),
    (Tipo::Zoom, ic::MAGNIFYING_GLASS_PLUS, "Zoom"),
    (Tipo::MobaDpad, ic::NAVIGATION_ARROW, "D-pad MOBA"),
    (Tipo::Habilidad, ic::LIGHTNING, "Habilidad MOBA"),
    (Tipo::Rotar, ic::ARROWS_CLOCKWISE, "Rotar"),
    (Tipo::Borde, ic::ARROWS_OUT, "Desplazar borde"),
    (Tipo::Desplazar, ic::MOUSE_SCROLL, "Desplazarse"),
    (Tipo::Rueda, ic::MOUSE_MIDDLE_CLICK, "Rueda del mouse"),
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
// Las partes de cada control
// ---------------------------------------------------------------------------------------------

/// Las teclas de un control: nombre para el panel y la tecla.
fn teclas(n: &Nodo) -> Vec<(&'static str, String)> {
    let v = |x: &String| x.clone();
    match n {
        Nodo::Clic(c, _) => vec![("Tecla", v(&c.key))],
        Nodo::Multi(m) => vec![("Tecla", v(&m.key))],
        Nodo::Arrastre(a) => vec![("Tecla", v(&a.key))],
        Nodo::Repetido(r) => vec![("Tecla", v(&r.key))],
        Nodo::Script(s) => vec![("Tecla", v(&s.key))],
        Nodo::Dpad(d) => vec![
            ("Arriba", v(&d.arriba)),
            ("Abajo", v(&d.abajo)),
            ("Izquierda", v(&d.izquierda)),
            ("Derecha", v(&d.derecha)),
            ("Velocidad 1", v(&d.tecla1)),
            ("Velocidad 2", v(&d.tecla2)),
        ],
        Nodo::MiradaLibre(m) => vec![("Tecla", v(&m.key)), ("Arriba", v(&m.arriba)), ("Abajo", v(&m.abajo)), ("Izquierda", v(&m.izquierda)), ("Derecha", v(&m.derecha))],
        Nodo::Deslizar(d) => vec![("Arriba", v(&d.arriba)), ("Abajo", v(&d.abajo)), ("Izquierda", v(&d.izquierda)), ("Derecha", v(&d.derecha))],
        Nodo::Zoom(z) => vec![("Acercar", v(&z.acercar)), ("Alejar", v(&z.alejar)), ("Modificador", v(&z.modificador))],
        Nodo::MobaDpad(m) => vec![("Detener", v(&m.cancelar))],
        Nodo::MobaHabilidad(h) => vec![("Tecla", v(&h.key)), ("Cancelar", v(&h.cancelar))],
        Nodo::Rotar(r) => vec![("Horario", v(&r.horario)), ("Antihorario", v(&r.antihorario))],
        Nodo::Borde(_) | Nodo::Desplazar(_) | Nodo::Rueda(_) | Nodo::Otro(_) => Vec::new(),
    }
}

fn tecla_mut(n: &mut Nodo, k: usize) -> Option<&mut String> {
    Some(match (n, k) {
        (Nodo::Clic(c, _), 0) => &mut c.key,
        (Nodo::Multi(m), 0) => &mut m.key,
        (Nodo::Arrastre(a), 0) => &mut a.key,
        (Nodo::Repetido(r), 0) => &mut r.key,
        (Nodo::Script(s), 0) => &mut s.key,
        (Nodo::Dpad(d), k) => [&mut d.arriba, &mut d.abajo, &mut d.izquierda, &mut d.derecha, &mut d.tecla1, &mut d.tecla2].into_iter().nth(k)?,
        (Nodo::MiradaLibre(m), k) => [&mut m.key, &mut m.arriba, &mut m.abajo, &mut m.izquierda, &mut m.derecha].into_iter().nth(k)?,
        (Nodo::Deslizar(d), k) => [&mut d.arriba, &mut d.abajo, &mut d.izquierda, &mut d.derecha].into_iter().nth(k)?,
        (Nodo::Zoom(z), k) => [&mut z.acercar, &mut z.alejar, &mut z.modificador].into_iter().nth(k)?,
        (Nodo::MobaDpad(m), 0) => &mut m.cancelar,
        (Nodo::MobaHabilidad(h), k) => [&mut h.key, &mut h.cancelar].into_iter().nth(k)?,
        (Nodo::Rotar(r), k) => [&mut r.horario, &mut r.antihorario].into_iter().nth(k)?,
        _ => return None,
    })
}

/// La posición principal del control.
fn posicion(n: &Nodo) -> Option<Punto> {
    Some(match n {
        Nodo::Clic(c, _) => c.pos,
        Nodo::Multi(m) => m.pasos.first()?.pos,
        Nodo::Dpad(d) => d.centro,
        Nodo::Arrastre(a) => a.inicio,
        Nodo::Repetido(r) => r.pos,
        Nodo::MiradaLibre(m) => m.pos,
        Nodo::Script(s) => s.pos,
        Nodo::Deslizar(d) => d.pos,
        Nodo::Zoom(z) => z.pos,
        Nodo::MobaDpad(m) => m.pos,
        Nodo::MobaHabilidad(h) => h.pos,
        Nodo::Rotar(r) => r.pos,
        Nodo::Borde(b) => b.pos,
        Nodo::Desplazar(d) => d.pos,
        Nodo::Rueda(r) => r.arriba,
        Nodo::Otro(_) => return None,
    })
}

/// El punto `k` del control (0 = la posición principal).
fn punto_mut(n: &mut Nodo, k: usize) -> Option<&mut Punto> {
    Some(match (n, k) {
        (Nodo::Clic(c, _), 0) => &mut c.pos,
        (Nodo::Multi(m), k) => &mut m.pasos.get_mut(k)?.pos,
        (Nodo::Dpad(d), 0) => &mut d.centro,
        (Nodo::Arrastre(a), 0) => &mut a.inicio,
        (Nodo::Arrastre(a), 1) => &mut a.fin,
        (Nodo::Repetido(r), 0) => &mut r.pos,
        (Nodo::MiradaLibre(m), 0) => &mut m.pos,
        (Nodo::Script(s), 0) => &mut s.pos,
        (Nodo::Deslizar(d), 0) => &mut d.pos,
        (Nodo::Zoom(z), 0) => &mut z.pos,
        (Nodo::MobaDpad(m), 0) => &mut m.pos,
        (Nodo::MobaDpad(m), 1) => &mut m.heroe,
        (Nodo::MobaHabilidad(h), 0) => &mut h.pos,
        (Nodo::MobaHabilidad(h), 1) => h.cancelar_pos.as_mut()?,
        (Nodo::Rotar(r), 0) => &mut r.pos,
        (Nodo::Borde(b), 0) => &mut b.pos,
        (Nodo::Desplazar(d), 0) => &mut d.pos,
        (Nodo::Rueda(r), 0) => &mut r.arriba,
        (Nodo::Rueda(r), 1) => &mut r.abajo,
        _ => return None,
    })
}

/// El radio (en altos de pantalla) de los controles redondos de BlueStacks.
fn radio_mut(n: &mut Nodo) -> Option<&mut f32> {
    Some(match n {
        Nodo::Deslizar(d) => &mut d.radio,
        Nodo::MobaDpad(m) => &mut m.radio,
        Nodo::MobaHabilidad(h) => &mut h.radio,
        Nodo::Rotar(r) => &mut r.radio,
        _ => return None,
    })
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

    fn borde(&self, elegida: bool) -> Stroke {
        if elegida {
            Stroke::new(2.0, ACENTO)
        } else {
            Stroke::new(1.2, self.alfa(Color32::WHITE, 0.75))
        }
    }
}

/// Insignia redonda con la tecla, como las de BlueStacks. Los textos largos se vuelven píldora.
fn insignia(p: &Painter, e: &Estilo, centro: Pos2, texto: &str, radio: f32, elegida: bool) {
    let largo = texto.chars().count();
    let tam = if largo <= 2 { radio * 0.9 } else { radio * 0.62 };
    let galeria = p.layout_no_wrap(texto.to_string(), FontId::proportional(tam), e.alfa(Color32::WHITE, 1.0));
    let ancho = (galeria.size().x + radio * 0.9).max(radio * 2.0);
    let caja = Rect::from_center_size(centro, Vec2::new(ancho, radio * 2.0));
    p.rect(caja, radio, e.alfa(Color32::from_black_alpha(165), 1.0), e.borde(elegida), egui::StrokeKind::Middle);
    p.galley(centro - galeria.size() / 2.0, galeria, Color32::WHITE);
}

fn icono_insignia(p: &Painter, e: &Estilo, centro: Pos2, glifo: &str, sub: Option<&str>, elegida: bool) {
    let r = e.radio * 1.15;
    p.circle(centro, r, e.alfa(Color32::from_black_alpha(165), 1.0), e.borde(elegida));
    p.text(centro, Align2::CENTER_CENTER, glifo, FontId::proportional(r * 1.05), e.alfa(Color32::WHITE, 1.0));
    if let Some(s) = sub.filter(|s| !s.is_empty()) {
        insignia(p, e, centro + Vec2::new(0.0, r + e.radio * 0.75), s, e.radio * 0.62, false);
    }
}

/// Un círculo con las cuatro teclas alrededor (D-pad, deslizar).
#[allow(clippy::too_many_arguments)]
fn cruz(p: &Painter, e: &Estilo, zonas: &mut Vec<Zona>, i: usize, c: Pos2, rad: f32, teclas: [&String; 4], elegida: Option<Asa>, sel: bool, flechas: bool) {
    p.circle(c, rad, e.alfa(Color32::from_black_alpha(90), 1.0), if sel { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.5, e.alfa(Color32::WHITE, 0.6)) });
    let lados = [Vec2::new(0.0, -rad), Vec2::new(0.0, rad), Vec2::new(-rad, 0.0), Vec2::new(rad, 0.0)];
    for (k, (tecla, desp)) in teclas.iter().zip(lados).enumerate() {
        if flechas {
            p.arrow(c, desp * 0.62, Stroke::new(1.5, e.alfa(Color32::WHITE, 0.55)));
        }
        let pos = c + desp;
        insignia(p, e, pos, &etiqueta(tecla), e.radio * 0.85, elegida == Some(Asa::Tecla(i, k)));
        zonas.push(Zona { asa: Asa::Tecla(i, k), centro: pos, radio: e.radio * 0.85 });
    }
}

/// Dibuja los controles del mapa. Devuelve las zonas que se pueden tocar en el editor.
pub fn dibujar(p: &Painter, rect: Rect, mapa: &Mapa, opacidad: f32, elegida: Option<Asa>, editando: bool) -> Vec<Zona> {
    let e = Estilo { radio: (rect.height() / 720.0 * 15.0).clamp(10.0, 24.0), opacidad };
    let mut zonas = Vec::new();
    let r = e.radio;
    let h = rect.height();
    let es = |a: Asa| elegida == Some(a);
    let del_nodo = |i: usize| elegida.and_then(Asa::nodo) == Some(i);

    for (i, nodo) in mapa.nodos.iter().enumerate() {
        let sel = del_nodo(i);
        // Asa para cambiar el tamaño de los controles redondos.
        let asa_radio = |zonas: &mut Vec<Zona>, c: Pos2, rad: f32| {
            if editando {
                let asa = c + Vec2::new(rad, rad).normalized() * rad;
                p.circle(asa, 5.0, ACENTO, Stroke::new(1.0, FONDO));
                zonas.push(Zona { asa: Asa::Radio(i), centro: asa, radio: 8.0 });
            }
        };
        match nodo {
            Nodo::Clic(c, doble) => {
                let pos = a_pantalla(rect, c.pos);
                let texto = if *doble { format!("{}×2", etiqueta(&c.key)) } else { etiqueta(&c.key) };
                insignia(p, &e, pos, &texto, r, es(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r });
            }
            Nodo::Repetido(n) => {
                let pos = a_pantalla(rect, n.pos);
                let veces = if n.hasta_soltar { "∞".to_string() } else { format!("×{}", n.veces) };
                insignia(p, &e, pos, &format!("{} {veces}", etiqueta(&n.key)), r, es(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r });
            }
            Nodo::Multi(m) => {
                if let Some(primero) = m.pasos.first() {
                    let pos = a_pantalla(rect, primero.pos);
                    insignia(p, &e, pos, &etiqueta(&m.key), r, es(Asa::Nodo(i)));
                    zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r });
                }
                if editando {
                    for (j, paso) in m.pasos.iter().enumerate().skip(1) {
                        let pos = a_pantalla(rect, paso.pos);
                        insignia(p, &e, pos, &(j + 1).to_string(), r * 0.6, es(Asa::Punto(i, j)));
                        zonas.push(Zona { asa: Asa::Punto(i, j), centro: pos, radio: r * 0.6 });
                    }
                }
            }
            Nodo::Dpad(d) => {
                let c = a_pantalla(rect, d.centro);
                let rad = (d.radio() * rect.width()).max(d.radio() * h).max(r * 2.6);
                cruz(p, &e, &mut zonas, i, c, rad, [&d.arriba, &d.abajo, &d.izquierda, &d.derecha], elegida, sel, false);
                p.circle_filled(c, r * 0.9, e.alfa(Color32::WHITE, 0.25));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: c, radio: r * 1.4 });
                if editando {
                    let asa = c + Vec2::new(rad, rad).normalized() * rad;
                    p.circle(asa, 5.0, ACENTO, Stroke::new(1.0, FONDO));
                    zonas.push(Zona { asa: Asa::Radio(i), centro: asa, radio: 8.0 });
                }
            }
            Nodo::Arrastre(a) => {
                let (ini, fin) = (a_pantalla(rect, a.inicio), a_pantalla(rect, a.fin));
                let color = e.alfa(Color32::WHITE, 0.8);
                p.arrow(ini, fin - ini, Stroke::new(2.0, color));
                if editando {
                    p.circle(fin, r * 0.45, e.alfa(Color32::from_black_alpha(165), 1.0), Stroke::new(1.2, color));
                    zonas.push(Zona { asa: Asa::Punto(i, 1), centro: fin, radio: r * 0.6 });
                }
                insignia(p, &e, ini, &etiqueta(&a.key), r, es(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: ini, radio: r });
            }
            Nodo::MiradaLibre(m) => {
                let pos = a_pantalla(rect, m.pos);
                icono_insignia(p, &e, pos, ic::EYE, Some(&etiqueta(&m.key)), sel);
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r * 1.2 });
            }
            Nodo::Script(s) => {
                let pos = a_pantalla(rect, s.pos);
                icono_insignia(p, &e, pos, ic::CODE, Some(&etiqueta(&s.key)), sel);
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r * 1.2 });
            }
            Nodo::Deslizar(d) => {
                let c = a_pantalla(rect, d.pos);
                let rad = (d.radio * h).max(r * 2.2);
                cruz(p, &e, &mut zonas, i, c, rad, [&d.arriba, &d.abajo, &d.izquierda, &d.derecha], elegida, sel, true);
                p.text(c, Align2::CENTER_CENTER, ic::HAND_SWIPE_RIGHT, FontId::proportional(r * 1.1), e.alfa(Color32::WHITE, 0.85));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: c, radio: r * 1.3 });
                asa_radio(&mut zonas, c, rad);
            }
            Nodo::Zoom(z) => {
                let pos = a_pantalla(rect, z.pos);
                icono_insignia(p, &e, pos, ic::MAGNIFYING_GLASS_PLUS, Some(&format!("{} / {}", etiqueta(&z.acercar), etiqueta(&z.alejar))), sel);
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r * 1.2 });
            }
            Nodo::MobaDpad(m) => {
                let c = a_pantalla(rect, m.pos);
                let rad = (m.radio * h).max(r * 2.2);
                p.circle(c, rad, e.alfa(Color32::from_black_alpha(90), 1.0), if sel { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.5, e.alfa(Color32::WHITE, 0.6)) });
                icono_insignia(p, &e, c, ic::NAVIGATION_ARROW, Some(&etiqueta("RightButton")), es(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: c, radio: r * 1.3 });
                asa_radio(&mut zonas, c, rad);
                if editando || sel {
                    let heroe = a_pantalla(rect, m.heroe);
                    icono_insignia(p, &e, heroe, ic::PERSON_SIMPLE, Some("Personaje"), es(Asa::Punto(i, 1)));
                    zonas.push(Zona { asa: Asa::Punto(i, 1), centro: heroe, radio: r * 1.2 });
                }
            }
            Nodo::MobaHabilidad(hb) => {
                let c = a_pantalla(rect, hb.pos);
                let rad = (hb.radio * h).max(r * 1.6);
                p.circle_stroke(c, rad, Stroke::new(1.2, e.alfa(Color32::WHITE, 0.45)));
                insignia(p, &e, c, &etiqueta(&hb.key), r, es(Asa::Nodo(i)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: c, radio: r });
                asa_radio(&mut zonas, c, rad);
                if let Some(cancelar) = hb.cancelar_pos.filter(|_| editando || sel) {
                    let pos = a_pantalla(rect, cancelar);
                    icono_insignia(p, &e, pos, ic::X_CIRCLE, Some(&etiqueta(&hb.cancelar)), es(Asa::Punto(i, 1)));
                    zonas.push(Zona { asa: Asa::Punto(i, 1), centro: pos, radio: r * 1.2 });
                }
            }
            Nodo::Rotar(ro) => {
                let c = a_pantalla(rect, ro.pos);
                let rad = (ro.radio * h).max(r * 2.2);
                p.circle(c, rad, e.alfa(Color32::from_black_alpha(70), 1.0), if sel { Stroke::new(2.0, ACENTO) } else { Stroke::new(1.5, e.alfa(Color32::WHITE, 0.6)) });
                p.text(c, Align2::CENTER_CENTER, ic::ARROWS_CLOCKWISE, FontId::proportional(r * 1.2), e.alfa(Color32::WHITE, 0.85));
                for (k, (tecla, desp)) in [(&ro.horario, Vec2::new(rad, 0.0)), (&ro.antihorario, Vec2::new(-rad, 0.0))].into_iter().enumerate() {
                    insignia(p, &e, c + desp, &etiqueta(tecla), r * 0.85, es(Asa::Tecla(i, k)));
                    zonas.push(Zona { asa: Asa::Tecla(i, k), centro: c + desp, radio: r * 0.85 });
                }
                zonas.push(Zona { asa: Asa::Nodo(i), centro: c, radio: r * 1.3 });
                asa_radio(&mut zonas, c, rad);
            }
            Nodo::Borde(b) => {
                let pos = a_pantalla(rect, b.pos);
                icono_insignia(p, &e, pos, ic::ARROWS_OUT, Some("Orillas"), sel);
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r * 1.2 });
                if sel {
                    // Las orillas donde el cursor mueve el mapa.
                    let color = ACENTO.gamma_multiply(0.18);
                    let (mx, my) = (b.margen_x * rect.width(), b.margen_y * h);
                    p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(mx, h)), 0.0, color);
                    p.rect_filled(Rect::from_min_size(Pos2::new(rect.max.x - mx, rect.min.y), Vec2::new(mx, h)), 0.0, color);
                    p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), my)), 0.0, color);
                    p.rect_filled(Rect::from_min_size(Pos2::new(rect.min.x, rect.max.y - my), Vec2::new(rect.width(), my)), 0.0, color);
                }
            }
            Nodo::Desplazar(d) => {
                let pos = a_pantalla(rect, d.pos);
                icono_insignia(p, &e, pos, ic::MOUSE_SCROLL, Some("Rueda"), sel);
                zonas.push(Zona { asa: Asa::Nodo(i), centro: pos, radio: r * 1.2 });
            }
            Nodo::Rueda(ru) => {
                let (arriba, abajo) = (a_pantalla(rect, ru.arriba), a_pantalla(rect, ru.abajo));
                icono_insignia(p, &e, arriba, ic::ARROW_FAT_UP, Some("Rueda arriba"), es(Asa::Nodo(i)));
                icono_insignia(p, &e, abajo, ic::ARROW_FAT_DOWN, Some("Rueda abajo"), es(Asa::Punto(i, 1)));
                zonas.push(Zona { asa: Asa::Nodo(i), centro: arriba, radio: r * 1.2 });
                zonas.push(Zona { asa: Asa::Punto(i, 1), centro: abajo, radio: r * 1.2 });
            }
            Nodo::Otro(_) => {}
        }
    }

    if let Some(raton) = &mapa.raton {
        let pos = a_pantalla(rect, raton.inicio);
        icono_insignia(p, &e, pos, ic::CROSSHAIR, Some(&etiqueta(&mapa.cambio)), es(Asa::Apuntar));
        zonas.push(Zona { asa: Asa::Apuntar, centro: pos, radio: r * 1.2 });
        if let Some(d) = raton.disparo.filter(|_| raton.disparo_con_clic) {
            let pos = a_pantalla(rect, d);
            icono_insignia(p, &e, pos, ic::MOUSE_LEFT_CLICK, None, es(Asa::Disparo));
            zonas.push(Zona { asa: Asa::Disparo, centro: pos, radio: r * 1.2 });
        }
        if let Some(ojo) = raton.mirada_pos {
            let pos = a_pantalla(rect, ojo);
            icono_insignia(p, &e, pos, ic::EYE, raton.mirada_tecla.as_deref().map(etiqueta).as_deref(), es(Asa::Ojo));
            zonas.push(Zona { asa: Asa::Ojo, centro: pos, radio: r * 1.2 });
        }
    }
    zonas
}

/// El punto de mira en el centro de la imagen, mientras se apunta.
pub fn dibujar_mira(p: &Painter, rect: Rect, mira: &mapeo::Mira) {
    let c = rect.center();
    let color = mira.color32();
    let t = (rect.height() / 720.0).max(0.6) * mira.size.clamp(1.0, 5.0);
    let trazo = Stroke::new(1.6 * t.sqrt(), color);
    let sombra = Stroke::new(1.6 * t.sqrt() + 1.5, Color32::from_black_alpha((90.0 * mira.opacity / 100.0) as u8));
    let brazo = |p: &Painter, trazo: Stroke| {
        for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            p.line_segment([c + d * 4.0 * t, c + d * 11.0 * t], trazo);
        }
    };
    match mira.style {
        EstiloMira::Cruz => {
            brazo(p, sombra);
            brazo(p, trazo);
        }
        EstiloMira::Punto => {
            p.circle_filled(c, 2.2 * t + 1.0, sombra.color);
            p.circle_filled(c, 2.2 * t, color);
        }
        EstiloMira::Circulo => {
            p.circle_stroke(c, 9.0 * t, sombra);
            p.circle_stroke(c, 9.0 * t, trazo);
        }
        EstiloMira::CruzPunto => {
            brazo(p, sombra);
            brazo(p, trazo);
            p.circle_filled(c, 1.6 * t, color);
        }
    }
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
    esquemas: Vec<String>,
    /// Nombre del esquema nuevo que se está escribiendo.
    nombre_esquema: Option<String>,
    aviso: Option<String>,
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
            esquemas: Vec::new(),
            nombre_esquema: None,
            aviso: None,
        }
    }

    pub fn abrir(&mut self, paquete: &str, mapa: Option<Mapa>) {
        self.mapa = mapa.unwrap_or_default();
        self.original = self.mapa.clone();
        self.paquete = paquete.to_string();
        self.elegida = None;
        self.esperando = None;
        self.nombre_esquema = None;
        self.aviso = None;
        self.esquemas = mapeo::esquemas(paquete);
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
        // Retroceso deja sin tecla las que son opcionales (modificadores, cancelar).
        let n = if nombre == "Key_Backspace" { String::new() } else { nombre.to_string() };
        match obj {
            Objetivo::Tecla(i, k) => {
                if let Some(t) = self.mapa.nodos.get_mut(i).and_then(|nodo| tecla_mut(nodo, k)) {
                    *t = n;
                }
            }
            Objetivo::Cambio => self.mapa.cambio = n,
            Objetivo::Pausa => {
                if let Some(r) = &mut self.mapa.raton {
                    r.tecla_pausa = (!n.is_empty()).then_some(n);
                }
            }
            Objetivo::Ojo => {
                if let Some(r) = &mut self.mapa.raton {
                    r.mirada_tecla = (!n.is_empty()).then_some(n);
                }
            }
        }
    }

    fn eliminar(&mut self) {
        match self.elegida.take() {
            Some(asa @ (Asa::Nodo(_) | Asa::Punto(..) | Asa::Tecla(..) | Asa::Radio(_))) => {
                if let Some(i) = asa.nodo().filter(|&i| i < self.mapa.nodos.len()) {
                    self.mapa.nodos.remove(i);
                }
            }
            Some(Asa::Apuntar | Asa::Disparo | Asa::Ojo) => self.mapa.raton = None,
            None => {}
        }
        self.esperando = None;
    }

    fn unico(&self, tipo: Tipo) -> bool {
        match tipo {
            Tipo::Apuntar => self.mapa.raton.is_some(),
            Tipo::MobaDpad => self.mapa.nodos.iter().any(|n| matches!(n, Nodo::MobaDpad(_))),
            Tipo::Borde => self.mapa.nodos.iter().any(|n| matches!(n, Nodo::Borde(_))),
            Tipo::Desplazar => self.mapa.nodos.iter().any(|n| matches!(n, Nodo::Desplazar(_))),
            Tipo::Rueda => self.mapa.nodos.iter().any(|n| matches!(n, Nodo::Rueda(_))),
            _ => false,
        }
    }

    fn crear(&mut self, tipo: Tipo, p: Punto) {
        let extra = Map::new;
        let mas = |dx: f32, dy: f32| Punto::nuevo(p.x + dx, p.y + dy);
        let nodo = match tipo {
            Tipo::Toque => Nodo::Clic(Clic { key: String::new(), pos: p, cambia_modo: false, extra: extra() }, false),
            Tipo::Repetido => Nodo::Repetido(Repetido { key: String::new(), pos: p, veces: 5, hasta_soltar: false, por_segundo: 10.0, extra: extra() }),
            Tipo::Dpad => Nodo::Dpad(Dpad {
                centro: p,
                arriba: "W".into(),
                abajo: "S".into(),
                izquierda: "A".into(),
                derecha: "D".into(),
                d_arriba: 0.04,
                d_abajo: 0.04,
                d_izquierda: 0.04,
                d_derecha: 0.04,
                tecla1: String::new(),
                radio1: 0.0,
                tecla2: String::new(),
                radio2: 0.0,
                velocidad: 0.0,
                activacion_ms: 0,
                velocidad_inicial: 0.0,
                zona_muerta: 0.0,
                extra: extra(),
            }),
            Tipo::Apuntar => {
                self.mapa.raton = Some(Raton::nuevo(p));
                self.elegida = Some(Asa::Apuntar);
                self.esperando = None;
                return;
            }
            Tipo::Mirada => Nodo::MiradaLibre(MiradaLibre {
                key: String::new(),
                pos: p,
                teclado: false,
                arriba: "Key_Up".into(),
                abajo: "Key_Down".into(),
                izquierda: "Key_Left".into(),
                derecha: "Key_Right".into(),
                velocidad: 1.0,
                espera_orilla_ms: 300,
                extra: extra(),
            }),
            Tipo::Script => Nodo::Script(ScriptNodo {
                key: String::new(),
                pos: p,
                texto: format!("# Toca donde está el control\ntap {:.1} {:.1}\n", p.x * 100.0, p.y * 100.0),
                extra: extra(),
            }),
            Tipo::Deslizar => Nodo::Deslizar(Deslizar {
                pos: p,
                arriba: "Key_Up".into(),
                abajo: "Key_Down".into(),
                izquierda: "Key_Left".into(),
                derecha: "Key_Right".into(),
                radio: 0.08,
                velocidad: 1.0,
                mantener: false,
                extra: extra(),
            }),
            Tipo::Zoom => Nodo::Zoom(Zoom {
                pos: p,
                acercar: "Key_Equal".into(),
                alejar: "Key_Minus".into(),
                modificador: String::new(),
                amplitud: 0.06,
                aceleracion: 1.5,
                velocidad: 0.6,
                modo: 0,
                espera_ms: 300,
                exclusivo: false,
                extra: extra(),
            }),
            Tipo::MobaDpad => Nodo::MobaDpad(MobaDpad { pos: p, radio: 0.08, heroe: Punto::nuevo(0.5, 0.55), velocidad_heroe: 0.35, cancelar: String::new(), extra: extra() }),
            Tipo::Habilidad => Nodo::MobaHabilidad(MobaHabilidad {
                key: String::new(),
                pos: p,
                radio: 0.1,
                modo: 0,
                detener_dpad: false,
                cancelar: String::new(),
                cancelar_pos: None,
                extra: extra(),
            }),
            Tipo::Rotar => Nodo::Rotar(Rotar {
                pos: p,
                horario: "E".into(),
                antihorario: "Q".into(),
                radio: 0.08,
                angulo: 0.0,
                velocidad: 90.0,
                activacion_ms: 0,
                extra: extra(),
            }),
            Tipo::Borde => Nodo::Borde(Borde {
                pos: Punto::nuevo(0.5, 0.5),
                vel_x: 0.5,
                vel_y: 0.5,
                margen_x: 0.03,
                margen_y: 0.03,
                factor: 2.0,
                acelerar_ms: 1000,
                espera_ms: 50,
                enabled: true,
                extra: extra(),
            }),
            Tipo::Desplazar => Nodo::Desplazar(Desplazar { pos: p, velocidad: 2.0, amplitud: 0.08, exclusivo: true, extra: extra() }),
            Tipo::Rueda => Nodo::Rueda(RuedaNodo { arriba: p, abajo: mas(0.0, 0.12), extra: extra() }),
        };
        self.mapa.nodos.push(nodo);
        let i = self.mapa.nodos.len() - 1;
        self.elegida = Some(Asa::Nodo(i));
        // Los controles de una tecla esperan su tecla al soltarlos en la pantalla.
        self.esperando = matches!(tipo, Tipo::Toque | Tipo::Repetido | Tipo::Mirada | Tipo::Script | Tipo::Habilidad).then_some(Objetivo::Tecla(i, 0));
    }

    fn mover(&mut self, asa: Asa, p: Punto, pos: Pos2, rect: Rect) {
        match asa {
            Asa::Nodo(i) | Asa::Tecla(i, _) => {
                let Some(nodo) = self.mapa.nodos.get_mut(i) else { return };
                // El deslizamiento de QtScrcpy se mueve entero.
                if let Nodo::Arrastre(a) = nodo {
                    let (dx, dy) = (a.fin.x - a.inicio.x, a.fin.y - a.inicio.y);
                    a.inicio = p;
                    a.fin = Punto::nuevo(p.x + dx, p.y + dy);
                } else if let Some(punto) = punto_mut(nodo, 0) {
                    *punto = p;
                }
            }
            Asa::Punto(i, k) => {
                if let Some(punto) = self.mapa.nodos.get_mut(i).and_then(|n| punto_mut(n, k)) {
                    *punto = p;
                }
            }
            Asa::Radio(i) => {
                let Some(nodo) = self.mapa.nodos.get_mut(i) else { return };
                let Some(c) = posicion(nodo) else { return };
                let distancia = a_pantalla(rect, c).distance(pos);
                if let Nodo::Dpad(d) = nodo {
                    d.fijar_radio(distancia / rect.width());
                } else if let Some(radio) = radio_mut(nodo) {
                    *radio = (distancia / rect.height()).clamp(0.02, 0.45);
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
            Asa::Ojo => {
                if let Some(r) = &mut self.mapa.raton {
                    r.mirada_pos = Some(p);
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
                    Some(Asa::Nodo(i)) if self.mapa.nodos.get(i).is_some_and(|n| !teclas(n).is_empty()) => Some(Objetivo::Tecla(i, 0)),
                    Some(Asa::Tecla(i, k)) => Some(Objetivo::Tecla(i, k)),
                    Some(Asa::Apuntar) => Some(Objetivo::Cambio),
                    Some(Asa::Ojo) => Some(Objetivo::Ojo),
                    _ => None,
                };
            }
            // Con un script elegido, el clic derecho agrega un toque en ese punto (como BlueStacks).
            if resp.secondary_clicked() {
                if let Some(Nodo::Script(s)) = self.elegida.and_then(Asa::nodo).and_then(|i| self.mapa.nodos.get_mut(i)) {
                    let p = a_punto(rect, pos);
                    if !s.texto.is_empty() && !s.texto.ends_with('\n') {
                        s.texto.push('\n');
                    }
                    s.texto += &format!("tap {:.1} {:.1}\n", p.x * 100.0, p.y * 100.0);
                }
            }
        }
        if resp.drag_stopped() {
            self.arrastrando = None;
        }
        if self.esperando.is_some() {
            let aviso = "Presiona una tecla o botón del mouse  ·  Esc cancela  ·  Retroceso la quita";
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
                ui.label(RichText::new("Editor de controles").size(14.5).strong().color(TEXTO));
                let app = if self.paquete.is_empty() { "Sin app".to_string() } else { self.paquete.clone() };
                ui.label(RichText::new(app).size(11.0).color(TENUE));
            });
        });

        let alto = (ui.available_height() - 96.0).max(120.0);
        egui::ScrollArea::vertical().max_height(alto).auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
                self.esquemas_ui(ui);

                seccion(ui, "Agregar control");
                ui.label(RichText::new("Arrástralo a la pantalla o haz clic").size(11.5).color(TENUE));
                egui::Grid::new("tipos").num_columns(3).spacing(Vec2::splat(4.0)).show(ui, |ui| {
                    for (k, (tipo, glifo, nombre)) in TIPOS.iter().enumerate() {
                        let habilitado = !self.unico(*tipo);
                        let resp = mosaico(ui, glifo, nombre, habilitado);
                        if habilitado && resp.drag_started() {
                            self.nuevo = Some(*tipo);
                        }
                        if habilitado && resp.clicked() && self.ultimo_rect.is_positive() {
                            self.crear(*tipo, a_punto(self.ultimo_rect, self.ultimo_rect.center()));
                        }
                        if k % 3 == 2 {
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
                ui.checkbox(&mut self.mapa.overlay.enabled, "Mostrar las teclas en pantalla al jugar");
            });
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

    /// Esquemas de controles del juego: elegir, crear, importar, exportar y borrar.
    fn esquemas_ui(&mut self, ui: &mut egui::Ui) {
        seccion(ui, "Esquema de controles");
        let actual = self.mapa.esquema();
        let mut lista = self.esquemas.clone();
        if !lista.contains(&actual) {
            lista.insert(0, actual.clone());
        }
        ui.horizontal(|ui| {
            let mut elegido = actual.clone();
            egui::ComboBox::from_id_salt("esquema").selected_text(&actual).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                for nombre in &lista {
                    ui.selectable_value(&mut elegido, nombre.clone(), nombre);
                }
            });
            if elegido != actual {
                match mapeo::cargar_esquema(&self.paquete, &elegido) {
                    Some(mapa) => {
                        self.mapa = mapa;
                        self.elegida = None;
                        self.esperando = None;
                        self.aviso = Some(format!("Esquema «{elegido}»: guarda para usarlo al jugar"));
                    }
                    None => self.aviso = Some("No se pudo abrir ese esquema".into()),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let boton = |ui: &mut egui::Ui, glifo: &str, ayuda: &str| ui.add(egui::Button::new(RichText::new(glifo).size(15.0)).min_size(Vec2::new(34.0, 26.0))).on_hover_text(ayuda);
            if boton(ui, ic::PLUS, "Esquema nuevo (copia el actual)").clicked() {
                self.nombre_esquema = Some(String::new());
            }
            if boton(ui, ic::DOWNLOAD_SIMPLE, "Importar un esquema (.json)").clicked() {
                if let Some(ruta) = elegir_json(false, &actual) {
                    match Mapa::importar(&ruta) {
                        Some(mut mapa) => {
                            let nombre = ruta.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Importado".into());
                            mapa.poner_esquema(&nombre);
                            self.mapa = mapa;
                            self.elegida = None;
                            self.aviso = Some(format!("Esquema «{nombre}» importado: guarda para usarlo"));
                        }
                        None => self.aviso = Some("Ese archivo no es un esquema de controles".into()),
                    }
                }
            }
            if boton(ui, ic::EXPORT, "Exportar este esquema (.json)").clicked() {
                if let Some(ruta) = elegir_json(true, &actual) {
                    self.aviso = Some(if self.mapa.exportar(&ruta).is_ok() { format!("Exportado a {}", ruta.display()) } else { "No se pudo exportar".into() });
                }
            }
            if actual != mapeo::PREDETERMINADO && boton(ui, ic::TRASH, "Borrar este esquema").clicked() {
                mapeo::borrar_esquema(&self.paquete, &actual);
                self.esquemas.retain(|n| n != &actual);
                self.mapa = mapeo::cargar_esquema(&self.paquete, mapeo::PREDETERMINADO).unwrap_or_default();
                self.aviso = Some(format!("Esquema «{actual}» borrado"));
            }
        });
        if let Some(nombre) = &mut self.nombre_esquema {
            let mut crear = false;
            ui.horizontal(|ui| {
                let campo = ui.add(egui::TextEdit::singleline(nombre).hint_text("Nombre del esquema").desired_width(ui.available_width() - 70.0));
                crear = campo.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                crear |= ui.button("Crear").clicked();
            });
            let limpio = nombre.trim().to_string();
            if crear && !limpio.is_empty() {
                self.mapa.poner_esquema(&limpio);
                if !self.esquemas.contains(&limpio) {
                    self.esquemas.push(limpio.clone());
                }
                self.nombre_esquema = None;
                self.aviso = Some(format!("Esquema «{limpio}» creado: guarda para conservarlo"));
            }
        }
        if let Some(aviso) = &self.aviso {
            ui.label(RichText::new(aviso).size(11.5).color(TENUE));
        }
    }

    fn boton_tecla(&mut self, ui: &mut egui::Ui, texto: &str, tecla: &str, objetivo: Objetivo) {
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(86.0, 24.0), egui::Label::new(RichText::new(texto).color(ICONO)));
            let esperando = self.esperando == Some(objetivo);
            let etiqueta = if esperando {
                "Presiona…".to_string()
            } else if tecla.is_empty() {
                "Sin tecla".to_string()
            } else {
                etiqueta(tecla)
            };
            let mut b = egui::Button::new(RichText::new(etiqueta).strong()).min_size(Vec2::new(120.0, 24.0));
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
            ui.label(RichText::new("Haz clic en un control para ver sus ajustes, y en una tecla para cambiarla. Arrástralo para moverlo; Supr lo borra.").size(11.5).color(TENUE));
            return;
        };
        match asa.nodo() {
            Some(i) => self.propiedades_nodo(ui, i),
            None => self.propiedades_apuntar(ui),
        }
        ui.add_space(4.0);
        if ui.add(egui::Button::new(RichText::new(format!("{}  Eliminar", ic::TRASH)).color(crate::CERRAR_HOVER))).clicked() {
            self.eliminar();
        }
    }

    fn propiedades_nodo(&mut self, ui: &mut egui::Ui, i: usize) {
        let Some(nodo) = self.mapa.nodos.get(i).cloned() else { return };
        let (glifo, nombre) = match &nodo {
            Nodo::Clic(_, true) => (ic::CURSOR_CLICK, "Doble toque"),
            Nodo::Clic(..) => (ic::HAND_TAP, "Tocar el punto"),
            Nodo::Multi(_) => (ic::CURSOR_CLICK, "Varios toques"),
            Nodo::Arrastre(_) => (ic::HAND_SWIPE_RIGHT, "Deslizar de un punto a otro"),
            Nodo::Repetido(_) => (ic::REPEAT, "Toques repetidos"),
            Nodo::Dpad(_) => (ic::JOYSTICK, "D-pad"),
            Nodo::MiradaLibre(_) => (ic::EYE, "Mirada libre"),
            Nodo::Script(_) => (ic::CODE, "Script"),
            Nodo::Deslizar(_) => (ic::HAND_SWIPE_RIGHT, "Deslizar"),
            Nodo::Zoom(_) => (ic::MAGNIFYING_GLASS_PLUS, "Zoom"),
            Nodo::MobaDpad(_) => (ic::NAVIGATION_ARROW, "D-pad MOBA"),
            Nodo::MobaHabilidad(_) => (ic::LIGHTNING, "Habilidad MOBA"),
            Nodo::Rotar(_) => (ic::ARROWS_CLOCKWISE, "Rotar"),
            Nodo::Borde(_) => (ic::ARROWS_OUT, "Desplazamiento de borde"),
            Nodo::Desplazar(_) => (ic::MOUSE_SCROLL, "Desplazarse"),
            Nodo::Rueda(_) => (ic::MOUSE_MIDDLE_CLICK, "Rueda del mouse"),
            Nodo::Otro(_) => {
                ui.label(RichText::new("Este tipo de control todavía no se puede editar.").color(TENUE));
                return;
            }
        };
        titulo(ui, glifo, nombre);
        for (k, (texto, tecla)) in teclas(&nodo).into_iter().enumerate() {
            // Las teclas de dirección de la mirada libre solo sirven con el teclado.
            if matches!(&nodo, Nodo::MiradaLibre(m) if !m.teclado) && k > 0 {
                continue;
            }
            self.boton_tecla(ui, texto, &tecla, Objetivo::Tecla(i, k));
        }
        let Some(nodo) = self.mapa.nodos.get_mut(i) else { return };
        if let Some(p) = punto_mut(nodo, 0) {
            posicion_ui(ui, "Posición", p);
        }
        match nodo {
            Nodo::Clic(c, doble) => {
                ui.checkbox(doble, "Doble toque");
                ui.checkbox(&mut c.cambia_modo, "También activa o suspende la puntería");
            }
            Nodo::Arrastre(a) => {
                posicion_ui(ui, "Hasta", &mut a.fin);
                numero(ui, "Velocidad", &mut a.velocidad, 0.2..=3.0, "");
            }
            Nodo::Repetido(r) => {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut r.hasta_soltar, false, "Tocar");
                    ui.selectable_value(&mut r.hasta_soltar, true, "Mantener presionado");
                });
                if r.hasta_soltar {
                    nota(ui, "Toca sin parar mientras la tecla siga presionada.");
                } else {
                    let mut veces = r.veces as f32;
                    if numero(ui, "Veces", &mut veces, 1.0..=50.0, "") {
                        r.veces = veces as u32;
                    }
                }
                numero(ui, "Por segundo", &mut r.por_segundo, 1.0..=30.0, "toques");
            }
            Nodo::Dpad(d) => {
                let mut radio = d.radio();
                if numero(ui, "Radio", &mut radio, 0.015..=0.25, "") {
                    d.fijar_radio(radio);
                }
                numero(ui, "Radio 1", &mut d.radio1, 0.0..=0.25, "");
                numero(ui, "Radio 2", &mut d.radio2, 0.0..=0.25, "");
                nota(ui, "Con la tecla de velocidad 1 o 2 presionada, el D-pad usa ese radio (por ejemplo, Shift para correr).");
                numero(ui, "Velocidad", &mut d.velocidad, 0.0..=20.0, "");
                let mut activacion = d.activacion_ms as f32;
                if numero(ui, "Activación", &mut activacion, 0.0..=1000.0, "ms") {
                    d.activacion_ms = activacion as u64;
                }
                numero(ui, "Vel. inicial", &mut d.velocidad_inicial, 0.0..=20.0, "");
                numero(ui, "Zona muerta", &mut d.zona_muerta, 0.0..=0.2, "");
                nota(ui, "Velocidad 0 = el dedo salta de golpe. Más velocidad = deslizamiento más rápido.");
            }
            Nodo::MiradaLibre(m) => {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut m.teclado, false, "Con el mouse");
                    ui.selectable_value(&mut m.teclado, true, "Con el teclado");
                });
                numero(ui, "Velocidad", &mut m.velocidad, 0.1..=10.0, "");
                let mut espera = m.espera_orilla_ms as f32;
                if numero(ui, "Espera orilla", &mut espera, 0.0..=2000.0, "ms") {
                    m.espera_orilla_ms = espera as u64;
                }
                nota(ui, "Mientras presionas la tecla, miras alrededor sin mover al personaje.");
            }
            Nodo::Script(s) => {
                nota(ui, "Clic derecho sobre la pantalla agrega un toque en ese punto.");
                ui.add(egui::TextEdit::multiline(&mut s.texto).code_editor().desired_rows(7).desired_width(f32::INFINITY));
                let (_, errores) = crate::script::leer(&s.texto);
                for (linea, texto) in errores.iter().take(4) {
                    ui.label(RichText::new(format!("Línea {linea}: no se entiende «{texto}»")).size(11.0).color(crate::CERRAR_HOVER));
                }
                egui::CollapsingHeader::new(RichText::new("Comandos").size(12.0).color(ICONO)).show(ui, |ui| {
                    for linea in [
                        "tap x y [ms] · swipe x1 y1 x2 y2 [ms]",
                        "touch x1 y1 x2 y2 … · wait ms",
                        "keyDown TAB · keyUp TAB (keyDown2: directo)",
                        "mouseDown x y · mouseMove x y · mouseUp",
                        "text \"hola\" · text backspace 2",
                        "mouseWheel x y 1 · loop n … loopEnd · abort",
                        "enterShoot · exitShoot · toggleShoot",
                        "onMouseMove (dTouch x+3 y) · onRelease",
                        "x e y van en % de la pantalla (0 a 100).",
                    ] {
                        ui.label(RichText::new(linea).size(11.0).monospace().color(TENUE));
                    }
                });
            }
            Nodo::Deslizar(d) => {
                numero(ui, "Radio", &mut d.radio, 0.02..=0.45, "");
                numero(ui, "Velocidad", &mut d.velocidad, 0.0..=10.0, "");
                ui.checkbox(&mut d.mantener, "Deslizar y mantener");
                nota(ui, "Con «mantener», el dedo se queda al final mientras la tecla siga presionada.");
            }
            Nodo::Zoom(z) => {
                numero(ui, "Amplitud", &mut z.amplitud, 0.01..=0.3, "");
                numero(ui, "Aceleración", &mut z.aceleracion, 1.0..=5.0, "×");
                numero(ui, "Velocidad", &mut z.velocidad, 0.05..=5.0, "");
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Modo").color(ICONO));
                    ui.selectable_value(&mut z.modo, 0, "Parejo");
                    ui.selectable_value(&mut z.modo, 1, "Un dedo más rápido");
                });
                let mut espera = z.espera_ms as f32;
                if numero(ui, "Espera", &mut espera, 0.0..=2000.0, "ms") {
                    z.espera_ms = espera as u64;
                }
                ui.checkbox(&mut z.exclusivo, "Solo con sus teclas (sin Ctrl + = y Ctrl + -)");
                nota(ui, "La rueda del mouse también hace zoom (amplitud por paso).");
            }
            Nodo::MobaDpad(m) => {
                numero(ui, "Radio", &mut m.radio, 0.02..=0.45, "");
                posicion_ui(ui, "Personaje", &mut m.heroe);
                numero(ui, "Velocidad", &mut m.velocidad_heroe, 0.05..=3.0, "");
                nota(ui, "Clic derecho mueve al personaje hacia el cursor. Pon el muñeco sobre tu personaje; si se pasa del punto, sube la velocidad, y si no llega, bájala.");
            }
            Nodo::MobaHabilidad(hb) => {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut hb.modo, 0, "Rápido");
                    ui.selectable_value(&mut hb.modo, 1, "Automático");
                    ui.selectable_value(&mut hb.modo, 2, "Manual");
                });
                nota(
                    ui,
                    match hb.modo {
                        0 => "Apunta con el mouse y se lanza al soltar la tecla.",
                        1 => "Un toque: el juego elige el objetivo.",
                        _ => "Apunta con el mouse y se lanza con clic izquierdo.",
                    },
                );
                numero(ui, "Radio", &mut hb.radio, 0.02..=0.45, "");
                ui.checkbox(&mut hb.detener_dpad, "Detener el D-pad MOBA al lanzar");
                let mut con_cancelar = hb.cancelar_pos.is_some();
                if ui.checkbox(&mut con_cancelar, "Cancelar habilidad").changed() {
                    hb.cancelar_pos = con_cancelar.then(|| Punto::nuevo(hb.pos.x - 0.08, hb.pos.y - 0.12));
                }
                if let Some(c) = &mut hb.cancelar_pos {
                    posicion_ui(ui, "Botón cancelar", c);
                }
            }
            Nodo::Rotar(r) => {
                numero(ui, "Radio", &mut r.radio, 0.02..=0.45, "");
                numero(ui, "Ángulo inicial", &mut r.angulo, -180.0..=180.0, "°");
                numero(ui, "Velocidad", &mut r.velocidad, 10.0..=720.0, "°/s");
                let mut activacion = r.activacion_ms as f32;
                if numero(ui, "Activación", &mut activacion, 0.0..=1000.0, "ms") {
                    r.activacion_ms = activacion as u64;
                }
            }
            Nodo::Borde(b) => {
                ui.checkbox(&mut b.enabled, "Activado");
                numero(ui, "Velocidad X", &mut b.vel_x, 0.05..=3.0, "");
                numero(ui, "Velocidad Y", &mut b.vel_y, 0.05..=3.0, "");
                numero(ui, "Orilla X", &mut b.margen_x, 0.005..=0.2, "");
                numero(ui, "Orilla Y", &mut b.margen_y, 0.005..=0.2, "");
                numero(ui, "Acelerar ×", &mut b.factor, 1.0..=5.0, "");
                let mut acelerar = b.acelerar_ms as f32;
                if numero(ui, "Acelerar tras", &mut acelerar, 0.0..=5000.0, "ms") {
                    b.acelerar_ms = acelerar as u64;
                }
                let mut espera = b.espera_ms as f32;
                if numero(ui, "Espera", &mut espera, 0.0..=1000.0, "ms") {
                    b.espera_ms = espera as u64;
                }
                nota(ui, "Con el cursor en una orilla de la imagen (las franjas moradas), el mapa se mueve hacia allá.");
            }
            Nodo::Desplazar(d) => {
                numero(ui, "Velocidad", &mut d.velocidad, 0.1..=10.0, "");
                numero(ui, "Amplitud", &mut d.amplitud, 0.01..=0.4, "");
                ui.checkbox(&mut d.exclusivo, "Solo este control usa la rueda");
                nota(ui, "La rueda del mouse arrastra en este punto (listas, inventarios).");
            }
            Nodo::Rueda(r) => {
                posicion_ui(ui, "Abajo", &mut r.abajo);
                nota(ui, "Rueda hacia arriba toca el primer punto y hacia abajo el segundo (cambiar de arma, por ejemplo).");
            }
            Nodo::Multi(_) | Nodo::Otro(_) => {}
        }
    }

    fn propiedades_apuntar(&mut self, ui: &mut egui::Ui) {
        let (cambio, pausa, ojo) = {
            let r = self.mapa.raton.as_ref();
            (self.mapa.cambio.clone(), r.and_then(|r| r.tecla_pausa.clone()).unwrap_or_default(), r.and_then(|r| r.mirada_tecla.clone()).unwrap_or_default())
        };
        titulo(ui, ic::CROSSHAIR, "Apuntar y disparar");
        self.boton_tecla(ui, "Activar", &cambio, Objetivo::Cambio);
        self.boton_tecla(ui, "Suspender", &pausa, Objetivo::Pausa);
        let Some(r) = &mut self.mapa.raton else { return };
        posicion_ui(ui, "Posición", &mut r.inicio);
        sensibilidad(ui, r);
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new("Ajustes").color(ICONO)));
            ui.add(egui::DragValue::new(&mut r.tweaks).hexadecimal(4, false, true));
        });
        ui.checkbox(&mut r.aceleracion, "Aceleración del mouse");

        seccion(ui, "Punto de mira");
        ui.checkbox(&mut r.mira.enabled, "Mostrar el punto de mira");
        if r.mira.enabled {
            ui.horizontal(|ui| {
                ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new("Estilo").color(ICONO)));
                let actual = EstiloMira::TODOS.iter().find(|e| e.0 == r.mira.style).map_or("", |e| e.1);
                egui::ComboBox::from_id_salt("mira").selected_text(actual).show_ui(ui, |ui| {
                    for (estilo, nombre) in EstiloMira::TODOS {
                        ui.selectable_value(&mut r.mira.style, estilo, nombre);
                    }
                });
            });
            numero(ui, "Tamaño", &mut r.mira.size, 1.0..=5.0, "");
            numero(ui, "Opacidad", &mut r.mira.opacity, 10.0..=100.0, "%");
            ui.horizontal(|ui| {
                ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new("Color").color(ICONO)));
                let mut color = r.mira.color32();
                if ui.color_edit_button_srgba(&mut color).changed() {
                    let [rr, g, b, _] = color.to_srgba_unmultiplied();
                    r.mira.color = format!("#{rr:02X}{g:02X}{b:02X}");
                }
                ui.add(egui::TextEdit::singleline(&mut r.mira.color).desired_width(70.0));
            });
        }

        seccion(ui, "Disparar con clic izquierdo");
        ui.checkbox(&mut r.disparo_con_clic, "Disparar con clic izquierdo");
        if r.disparo_con_clic {
            let disparo = r.disparo.get_or_insert(Punto::nuevo(r.inicio.x + 0.05, r.inicio.y + 0.2));
            posicion_ui(ui, "Botón de fuego", disparo);
        }

        seccion(ui, "Mirar alrededor");
        let mut mirar = r.mirada_pos.is_some();
        if ui.checkbox(&mut mirar, "Mirar alrededor sin mover la mira").changed() {
            r.mirada_pos = mirar.then(|| Punto::nuevo(r.inicio.x, (r.inicio.y - 0.25).max(0.05)));
            if mirar && r.mirada_tecla.is_none() {
                r.mirada_tecla = Some("Key_Alt".into());
            }
        }
        if let Some(ojo_pos) = &mut r.mirada_pos {
            posicion_ui(ui, "Botón del ojo", ojo_pos);
            nota(ui, "Pon el ojo sobre el botón de mirar del juego. Mientras presionas su tecla, el mouse mira alrededor.");
            self.boton_tecla(ui, "Tecla", &ojo, Objetivo::Ojo);
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

fn nota(ui: &mut egui::Ui, texto: &str) {
    ui.label(RichText::new(texto).size(11.0).color(TENUE));
}

/// Un número con su nombre (como los ajustes avanzados de BlueStacks). Devuelve si cambió.
fn numero(ui: &mut egui::Ui, texto: &str, valor: &mut f32, rango: std::ops::RangeInclusive<f32>, unidad: &str) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new(texto).color(ICONO)));
        let paso = ((*rango.end() - *rango.start()) / 300.0).max(0.001);
        let decimales = if paso < 0.01 { 3 } else if paso < 1.0 { 2 } else { 0 };
        ui.add(egui::DragValue::new(valor).range(rango).speed(paso).max_decimals(decimales).suffix(if unidad.is_empty() { String::new() } else { format!(" {unidad}") })).changed()
    })
    .inner
}

/// La posición en porcentaje de la pantalla, como la escribe BlueStacks.
fn posicion_ui(ui: &mut egui::Ui, texto: &str, p: &mut Punto) {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new(texto).color(ICONO)));
        let (mut x, mut y) = (p.x * 100.0, p.y * 100.0);
        let cambio = ui.add(egui::DragValue::new(&mut x).range(0.0..=100.0).speed(0.2).max_decimals(2).prefix("X ")).changed()
            | ui.add(egui::DragValue::new(&mut y).range(0.0..=100.0).speed(0.2).max_decimals(2).prefix("Y ")).changed();
        if cambio {
            *p = Punto::nuevo(x / 100.0, y / 100.0);
        }
    });
}

fn deslizador(ui: &mut egui::Ui, texto: &str, valor: &mut f32, rango: std::ops::RangeInclusive<f32>) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new(texto).color(ICONO)));
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(80.0, 64.0), Sense::click_and_drag());
    let encima = habilitado && resp.hovered();
    ui.painter().rect_filled(rect, 6.0, if encima { FONDO_HOVER } else { FONDO_CONTROL });
    let color = if !habilitado { TENUE.gamma_multiply(0.6) } else if encima { Color32::WHITE } else { ICONO };
    ui.painter().text(rect.center_top() + Vec2::new(0.0, 18.0), Align2::CENTER_CENTER, glifo, FontId::proportional(20.0), color);
    let texto = ui.painter().layout(nombre.to_string(), FontId::proportional(10.5), color, rect.width() - 6.0);
    ui.painter().galley(Pos2::new(rect.center().x - texto.size().x / 2.0, rect.min.y + 33.0), texto, color);
    if habilitado {
        resp.on_hover_cursor(egui::CursorIcon::Grab)
    } else {
        resp.on_hover_text("Solo puede haber uno")
    }
}

/// El diálogo de KDE (o el de GNOME) para importar o exportar un esquema.
fn elegir_json(guardar: bool, nombre: &str) -> Option<PathBuf> {
    let inicio = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let sugerido = inicio.join(format!("{nombre}.json"));
    let (modo, titulo) = if guardar { ("--getsavefilename", "Exportar el esquema") } else { ("--getopenfilename", "Importar un esquema") };
    let salida = Command::new("kdialog")
        .args(["--title", titulo, modo])
        .arg(if guardar { &sugerido } else { &inicio })
        .arg("*.json|Esquemas de controles")
        .output()
        .or_else(|_| {
            let mut zenity = Command::new("zenity");
            zenity.args(["--file-selection", &format!("--title={titulo}"), "--file-filter=Esquemas de controles | *.json"]);
            if guardar {
                zenity.args(["--save", "--confirm-overwrite"]).arg(format!("--filename={}", sugerido.display()));
            }
            zenity.output()
        })
        .ok()?;
    let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    (salida.status.success() && !ruta.is_empty()).then(|| PathBuf::from(ruta))
}
