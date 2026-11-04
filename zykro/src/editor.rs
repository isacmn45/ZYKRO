//! Controles del juego al estilo de BlueStacks: insignias semitransparentes sobre el juego y un
//! editor con panel lateral para agregar los controles arrastrándolos a la pantalla, ajustar sus
//! propiedades (las mismas que los «ajustes avanzados» de BlueStacks) y tener varios esquemas de
//! controles por juego.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Id, LayerId, Layout, Order, Painter, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
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
    /// El punto de mira de la puntería.
    Mira,
}

impl Asa {
    fn nodo(self) -> Option<usize> {
        match self {
            Asa::Nodo(i) | Asa::Punto(i, _) | Asa::Tecla(i, _) | Asa::Radio(i) => Some(i),
            _ => None,
        }
    }

    /// Una parte del control de puntería: sus ajustes van en su propia ventana.
    fn de_apuntar(self) -> bool {
        matches!(self, Asa::Apuntar | Asa::Disparo | Asa::Ojo | Asa::Mira)
    }
}

/// A qué se le asigna la próxima tecla.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Objetivo {
    Tecla(usize, usize),
    Cambio,
    Pausa,
    Ojo,
    /// Un botón de la pestaña «Gamepad» de la puntería.
    Mando(CampoMando),
    /// El botón del mando de la tecla `k` de un control.
    MandoNodo(usize, usize),
}

/// Los botones de la pestaña «Gamepad» de la puntería.
#[derive(Clone, Copy, PartialEq, Debug)]
enum CampoMando {
    Iniciar,
    Suspender,
    Mirar,
    Disparar,
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

    // Al jugar, «Mostrar teclas en pantalla» de la puntería decide si se ven sus insignias.
    if let Some(raton) = mapa.raton.as_ref().filter(|r| editando || r.mostrar) {
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
        // En el editor la mira se ve donde va y se arrastra, como en BlueStacks.
        if editando && raton.mira.enabled {
            let pos = a_pantalla(rect, Punto::nuevo(raton.mira.x, raton.mira.y));
            dibujar_mira(p, rect, &raton.mira);
            if es(Asa::Mira) {
                p.circle_stroke(pos, r * 1.4, Stroke::new(2.0, ACENTO));
            }
            zonas.push(Zona { asa: Asa::Mira, centro: pos, radio: r * 1.4 });
        }
    }
    zonas
}

/// Texturas de los puntos de mira personalizados, por archivo (`None` si no se pudo leer).
static IMAGENES_MIRA: Mutex<Vec<(String, Option<TextureHandle>)>> = Mutex::new(Vec::new());

fn imagen_mira(ctx: &egui::Context, nombre: &str) -> Option<TextureHandle> {
    let mut lista = IMAGENES_MIRA.lock().unwrap();
    if let Some((_, textura)) = lista.iter().find(|(n, _)| n == nombre) {
        return textura.clone();
    }
    let imagen = crate::ajustes_android::leer_png(&mapeo::carpeta_miras().join(nombre));
    let textura = imagen.map(|i| ctx.load_texture(format!("mira-{nombre}"), i, TextureOptions::LINEAR));
    lista.push((nombre.to_string(), textura.clone()));
    textura
}

/// Copia la imagen elegida para la mira (PNG o JPG de hasta 5 MB, como en BlueStacks) a
/// `carpeta_miras` como PNG y devuelve su nombre.
fn guardar_imagen_mira(origen: &Path) -> Result<String, String> {
    let tamano = std::fs::metadata(origen).map_err(|e| e.to_string())?.len();
    if tamano > 5 * 1024 * 1024 {
        return Err("La imagen pasa de 5 MB".into());
    }
    let carpeta = mapeo::carpeta_miras();
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let nombre = format!("{}.png", origen.file_stem().map_or("mira".into(), |s| s.to_string_lossy()));
    let destino = carpeta.join(&nombre);
    if origen.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
        std::fs::copy(origen, &destino).map_err(|e| e.to_string())?;
    } else if !Command::new("magick").arg(origen).arg(&destino).status().is_ok_and(|s| s.success()) {
        return Err("Para usar un JPG hace falta ImageMagick; un PNG funciona siempre".into());
    }
    if crate::ajustes_android::leer_png(&destino).is_none() {
        return Err("No se pudo leer la imagen".into());
    }
    IMAGENES_MIRA.lock().unwrap().retain(|(n, _)| n != &nombre);
    Ok(nombre)
}

/// El punto de mira, mientras se apunta: donde diga `mira` (al centro de fábrica).
pub fn dibujar_mira(p: &Painter, rect: Rect, mira: &mapeo::Mira) {
    let c = a_pantalla(rect, Punto::nuevo(mira.x, mira.y));
    let color = mira.color32();
    let t = (rect.height() / 720.0).max(0.6) * mira.size.max(1.0);
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
        // La imagen en un cuadro de 36 px por cada punto de tamaño (en una ventana de 1000 de alto),
        // como el ícono de BlueStacks, y sin color: el de la imagen.
        EstiloMira::Imagen => {
            if let Some(textura) = imagen_mira(p.ctx(), &mira.image) {
                let lado = 36.0 * mira.size.max(1.0) * (rect.height() / 1000.0).max(0.6);
                let tam = textura.size_vec2();
                let caja = Rect::from_center_size(c, tam * (lado / tam.x.max(tam.y)));
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                p.image(textura.id(), caja, uv, Color32::WHITE.gamma_multiply((mira.opacity / 100.0).clamp(0.0, 1.0)));
            }
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
    /// La ventana «Ajustes de apuntar, desplazar y disparar» está abierta, y en qué pestaña.
    ajustes_apuntar: bool,
    pestana_mando: bool,
    /// Por qué no se pudo usar la imagen elegida para la mira.
    error_mira: Option<String>,
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
            ajustes_apuntar: false,
            pestana_mando: false,
            error_mira: None,
        }
    }

    pub fn abrir(&mut self, paquete: &str, mapa: Option<Mapa>) {
        self.mapa = mapa.unwrap_or_default();
        self.original = self.mapa.clone();
        self.paquete = paquete.to_string();
        self.elegida = None;
        self.esperando = None;
        self.ajustes_apuntar = false;
        self.pestana_mando = false;
        self.error_mira = None;
        self.nombre_esquema = None;
        self.aviso = None;
        self.esquemas = mapeo::esquemas(paquete);
        self.abierto = true;
    }

    pub fn esperando_tecla(&self) -> bool {
        self.esperando.is_some()
    }

    /// Espera un botón del mando.
    pub fn esperando_mando(&self) -> bool {
        matches!(self.esperando, Some(Objetivo::Mando(_) | Objetivo::MandoNodo(..)))
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
        // Los campos del mando solo aceptan botones del mando (y Retroceso, que lo quita); los demás,
        // solo teclas y el mouse.
        let del_mando = nombre.starts_with("Gamepad");
        if matches!(obj, Objetivo::Mando(_) | Objetivo::MandoNodo(..)) != del_mando && nombre != "Key_Backspace" {
            self.esperando = Some(obj);
            return;
        }
        // Retroceso deja sin tecla las que son opcionales (modificadores, cancelar).
        let n = if nombre == "Key_Backspace" { String::new() } else { nombre.to_string() };
        match obj {
            Objetivo::Tecla(i, k) => {
                if let Some(nodo) = self.mapa.nodos.get_mut(i) {
                    nodo.poner_tecla(k, n);
                }
            }
            Objetivo::MandoNodo(i, k) => {
                if let Some(nodo) = self.mapa.nodos.get_mut(i) {
                    nodo.poner_mando(k, n);
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
            Objetivo::Mando(campo) => {
                if let Some(r) = &mut self.mapa.raton {
                    let m = &mut r.mando;
                    *match campo {
                        CampoMando::Iniciar => &mut m.iniciar,
                        CampoMando::Suspender => &mut m.suspender,
                        CampoMando::Mirar => &mut m.mirar,
                        CampoMando::Disparar => &mut m.disparar,
                    } = n;
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
            Some(Asa::Apuntar | Asa::Disparo | Asa::Ojo) => {
                self.mapa.raton = None;
                self.ajustes_apuntar = false;
            }
            // Borrar la mira la apaga, como en BlueStacks.
            Some(Asa::Mira) => {
                if let Some(r) = &mut self.mapa.raton {
                    r.mira.enabled = false;
                }
            }
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
            Tipo::Toque => Nodo::Clic(Clic { key: String::new(), pos: p, cambia_modo: false, mando: Vec::new(), extra: extra() }, false),
            Tipo::Repetido => Nodo::Repetido(Repetido { key: String::new(), pos: p, veces: 5, hasta_soltar: false, por_segundo: 10.0, mando: Vec::new(), extra: extra() }),
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
                mando: Vec::new(),
                stick: String::new(),
                extra: extra(),
            }),
            Tipo::Apuntar => {
                let mut r = Raton::nuevo(p);
                // «Ajustes» como BlueStacks: 0 en un control nuevo y el del esquema de Free Fire ahí.
                r.tweaks = if self.paquete.starts_with("com.dts.freefire") { 16450 } else { 0 };
                self.mapa.raton = Some(r);
                self.elegida = Some(Asa::Apuntar);
                self.esperando = None;
                self.ajustes_apuntar = true;
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
                mando: Vec::new(), extra: extra(),
            }),
            Tipo::Script => Nodo::Script(ScriptNodo {
                key: String::new(),
                pos: p,
                texto: format!("# Toca donde está el control\ntap {:.1} {:.1}\n", p.x * 100.0, p.y * 100.0),
                mando: Vec::new(), extra: extra(),
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
                mando: Vec::new(), extra: extra(),
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
                mando: Vec::new(), extra: extra(),
            }),
            Tipo::MobaDpad => Nodo::MobaDpad(MobaDpad { pos: p, radio: 0.08, heroe: Punto::nuevo(0.5, 0.55), velocidad_heroe: 0.35, cancelar: String::new(), mando: Vec::new(), extra: extra() }),
            Tipo::Habilidad => Nodo::MobaHabilidad(MobaHabilidad {
                key: String::new(),
                pos: p,
                radio: 0.1,
                modo: 0,
                detener_dpad: false,
                cancelar: String::new(),
                cancelar_pos: None,
                mando: Vec::new(), extra: extra(),
            }),
            Tipo::Rotar => Nodo::Rotar(Rotar {
                pos: p,
                horario: "E".into(),
                antihorario: "Q".into(),
                radio: 0.08,
                angulo: 0.0,
                velocidad: 90.0,
                activacion_ms: 0,
                mando: Vec::new(), extra: extra(),
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
            Asa::Mira => {
                if let Some(r) = &mut self.mapa.raton {
                    (r.mira.x, r.mira.y) = (p.x, p.y);
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
                if let Some(asa) = self.arrastrando {
                    self.elegida = Some(asa);
                    self.esperando = None;
                    self.ajustes_apuntar &= asa.de_apuntar();
                }
            }
            if resp.dragged() {
                if let Some(asa) = self.arrastrando {
                    self.mover(asa, a_punto(rect, pos), pos, rect);
                }
            }
            if resp.clicked() {
                self.elegida = tocada(pos);
                // Al hacer clic en una tecla, la próxima tecla que presiones se asigna. La puntería
                // abre su ventana de ajustes (sus teclas se cambian ahí).
                self.esperando = match self.elegida {
                    Some(Asa::Nodo(i)) if self.mapa.nodos.get(i).is_some_and(|n| !teclas(n).is_empty()) => Some(Objetivo::Tecla(i, 0)),
                    Some(Asa::Tecla(i, k)) => Some(Objetivo::Tecla(i, k)),
                    _ => None,
                };
                self.ajustes_apuntar = self.elegida.is_some_and(Asa::de_apuntar);
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
        self.ventana_apuntar(ui.ctx(), rect);
        if self.esperando.is_some() {
            let aviso = if self.esperando_mando() {
                "Presiona un botón del mando  ·  Esc cancela  ·  Retroceso lo quita"
            } else {
                "Presiona una tecla o botón del mouse  ·  Esc cancela  ·  Retroceso la quita"
            };
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
            selector_tecla(ui, &mut self.esperando, tecla, objetivo);
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
        // Las teclas de dirección de la mirada libre solo sirven con el teclado.
        let casillas: Vec<(usize, &str, String)> = teclas(&nodo)
            .into_iter()
            .enumerate()
            .filter(|(k, _)| !matches!(&nodo, Nodo::MiradaLibre(m) if !m.teclado) || *k == 0)
            .map(|(k, (texto, tecla))| (k, texto, tecla))
            .collect();
        for (k, texto, tecla) in &casillas {
            let tecla = if mapeo::es_mando(tecla) { "" } else { tecla.as_str() };
            self.boton_tecla(ui, texto, tecla, Objetivo::Tecla(i, *k));
        }
        // Los botones del mando de esas teclas (la pestaña «Gamepad» de BlueStacks).
        if !casillas.is_empty() {
            seccion(ui, "Mando");
            for (k, texto, _) in &casillas {
                let boton = nodo.mando_de(*k).to_string();
                self.boton_tecla(ui, texto, &boton, Objetivo::MandoNodo(i, *k));
            }
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
                ui.horizontal(|ui| {
                    ui.add_sized(Vec2::new(86.0, 20.0), egui::Label::new(RichText::new("Joystick").color(ICONO)));
                    selector_joystick(ui, "joystick-dpad", &mut d.stick);
                });
                nota(ui, "Con un joystick del mando, el dedo va tan lejos como lo inclines.");
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
        titulo(ui, ic::CROSSHAIR, "Apuntar y disparar");
        nota(ui, "Sus ajustes están en la ventana «Ajustes de apuntar, desplazar y disparar», como en BlueStacks.");
        if !self.ajustes_apuntar && ui.button(format!("{}  Abrir los ajustes", ic::GEAR)).clicked() {
            self.ajustes_apuntar = true;
        }
    }

    /// «Ajustes de apuntar, desplazar y disparar»: los campos de la ventana de BlueStacks, en el mismo
    /// orden (los de su pestaña «Teclado y ratón»: el mando llega directo a Android).
    fn ventana_apuntar(&mut self, ctx: &egui::Context, area: Rect) {
        if self.mapa.raton.is_none() {
            self.ajustes_apuntar = false;
        }
        if !self.ajustes_apuntar {
            return;
        }
        let cambio = self.mapa.cambio.clone();
        let marco = egui::Frame::new().fill(FONDO).corner_radius(8.0).stroke(Stroke::new(1.0, FONDO_HOVER)).inner_margin(egui::Margin::same(12));
        // Del lado contrario al control, para no taparlo (después se queda donde la dejes).
        let a_la_derecha = self.mapa.raton.as_ref().is_some_and(|r| r.inicio.x < 0.5);
        let x = if a_la_derecha { area.max.x - 362.0 } else { area.min.x + 16.0 };
        let (mut cerrar, mut eliminar, mut elegir_imagen) = (false, false, false);
        egui::Window::new("Ajustes de apuntar, desplazar y disparar")
            .id(Id::new("ajustes-apuntar"))
            .title_bar(false)
            .frame(marco)
            // Del tamaño de lo que tiene (si no, egui la deja de 420 de alto); lo largo se desplaza.
            .auto_sized()
            .constrain_to(area)
            .default_pos(Pos2::new(x, area.min.y + 16.0))
            .show(ctx, |ui| {
                ui.set_width(322.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ic::CROSSHAIR).size(15.0).color(ACENTO));
                    ui.label(RichText::new("Ajustes de apuntar, desplazar y disparar").size(13.0).strong().color(TEXTO));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        cerrar = crate::icono(ui, ic::X, "Cerrar", Vec2::new(24.0, 22.0), crate::CERRAR_HOVER).clicked();
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (mando, texto) in [(false, "Teclado y mouse"), (true, "Gamepad")] {
                        if pestana(ui, self.pestana_mando == mando, texto) {
                            self.pestana_mando = mando;
                            self.esperando = None;
                        }
                    }
                });
                let Some(r) = self.mapa.raton.as_mut() else { return };
                let esperando = &mut self.esperando;
                let error_mira = &self.error_mira;
                let pestana_mando = self.pestana_mando;
                egui::ScrollArea::vertical().max_height((area.height() - 130.0).max(160.0)).show(ui, |ui| {
                    if pestana_mando {
                        ajustes_mando(ui, r, esperando);
                        return;
                    }
                    seccion(ui, "Atajos de teclado");
                    let pausa = r.tecla_pausa.clone().unwrap_or_default();
                    rejilla(ui, "apuntar-teclas", |ui| {
                        fila(ui, "Iniciar/Detener", |ui| selector_tecla(ui, esperando, &cambio, Objetivo::Cambio));
                        fila(ui, "Suspender", |ui| selector_tecla(ui, esperando, &pausa, Objetivo::Pausa));
                    });
                    nota(ui, "Iniciar/Detener entra y sale de la puntería; mientras mantienes Suspender, el cursor queda libre.");

                    seccion(ui, "Propiedades");
                    rejilla(ui, "apuntar-propiedades", |ui| {
                        posicion_filas(ui, ("X", "Y"), &mut r.inicio);
                        // Como en BlueStacks, Y es la vertical de verdad (en el archivo va como proporción de X).
                        let mut vertical = r.sens * r.ratio_y;
                        fila(ui, "Sensibilidad del mouse X", |ui| {
                            decimal(ui, &mut r.sens, 0.01..=100.0);
                        });
                        fila(ui, "Sensibilidad del mouse Y", |ui| {
                            decimal(ui, &mut vertical, 0.01..=100.0);
                        });
                        r.ratio_y = vertical / r.sens.max(0.01);
                        fila(ui, "Ajustes", |ui| {
                            ui.add(egui::DragValue::new(&mut r.tweaks).range(0..=999_999_999)).on_hover_text(AYUDA_AJUSTES);
                        });
                        fila(ui, "Aceleración del mouse", |ui| {
                            ui.checkbox(&mut r.aceleracion, "");
                        });
                    });

                    ui.add_space(4.0);
                    ui.checkbox(&mut r.mira.enabled, RichText::new("Punto de mira").strong().color(TEXTO));
                    if r.mira.enabled {
                        let mira = &mut r.mira;
                        rejilla(ui, "apuntar-mira", |ui| {
                            fila(ui, "Estilo", |ui| {
                                let actual = EstiloMira::TODOS.iter().find(|e| e.0 == mira.style).map_or("", |e| e.1);
                                egui::ComboBox::from_id_salt("estilo-mira").selected_text(actual).show_ui(ui, |ui| {
                                    for (estilo, nombre) in EstiloMira::TODOS {
                                        ui.selectable_value(&mut mira.style, estilo, nombre);
                                    }
                                });
                            });
                            if mira.style == EstiloMira::Imagen {
                                fila(ui, "Imagen", |ui| {
                                    elegir_imagen = ui.button(if mira.image.is_empty() { "Elegir archivo" } else { "Cambiar archivo" }).clicked();
                                });
                            }
                            fila(ui, "Tamaño", |ui| {
                                decimal(ui, &mut mira.size, 1.0..=10.0);
                            });
                            fila(ui, "Opacidad", |ui| {
                                ui.add(egui::DragValue::new(&mut mira.opacity).range(0.0..=100.0).speed(0.5).fixed_decimals(0).suffix(" %"));
                            });
                            // La imagen va con sus colores.
                            if mira.style != EstiloMira::Imagen {
                                fila(ui, "Color", |ui| {
                                    ui.horizontal(|ui| {
                                        let mut color = mira.color32();
                                        if ui.color_edit_button_srgba(&mut color).changed() {
                                            let [rr, g, b, _] = color.to_srgba_unmultiplied();
                                            mira.color = format!("#{rr:02X}{g:02X}{b:02X}");
                                        }
                                        ui.add(egui::TextEdit::singleline(&mut mira.color).desired_width(70.0));
                                    });
                                });
                            }
                        });
                        if mira.style == EstiloMira::Imagen {
                            let texto = match error_mira {
                                Some(e) => e.clone(),
                                None if mira.image.is_empty() => "Un PNG o JPG de hasta 5 MB.".into(),
                                None => format!("{} · un PNG o JPG de hasta 5 MB.", mira.image),
                            };
                            nota(ui, &texto);
                        }
                        nota(ui, "La opacidad de la mira no depende de la de los controles en pantalla. Arrástrala en la pantalla para moverla.");
                    }

                    ui.add_space(4.0);
                    ui.checkbox(&mut r.disparo_con_clic, RichText::new("Disparar con clic izquierdo").strong().color(TEXTO));
                    if r.disparo_con_clic {
                        nota(ui, &format!("{}  Pon el ícono de fuego sobre el botón de disparo del juego.", ic::MOUSE_LEFT_CLICK));
                        let inicio = r.inicio;
                        let disparo = r.disparo.get_or_insert(Punto::nuevo(inicio.x + 0.05, inicio.y + 0.2));
                        rejilla(ui, "apuntar-disparo", |ui| {
                            posicion_filas(ui, ("Posición de acción X", "Posición de acción Y"), disparo);
                            fila(ui, "Acción", |ui| {
                                ui.label(RichText::new("Clic izquierdo").color(TEXTO));
                            });
                        });
                    }

                    ui.add_space(4.0);
                    let mut mirar = r.mirada_pos.is_some();
                    if ui.checkbox(&mut mirar, RichText::new("Mirar alrededor").strong().color(TEXTO)).changed() {
                        // Como en BlueStacks: al prenderlo aparece el ojo para ponerlo y su tecla es V; al
                        // apagarlo se quitan los dos.
                        let tecla = r.mirada_tecla.take().unwrap_or_else(|| "V".into());
                        r.mirada_tecla = mirar.then_some(tecla);
                        r.mirada_pos = mirar.then(|| Punto::nuevo(r.inicio.x, (r.inicio.y - 0.25).max(0.05)));
                    }
                    let tecla_ojo = r.mirada_tecla.clone().unwrap_or_default();
                    if let Some(ojo) = &mut r.mirada_pos {
                        nota(
                            ui,
                            &format!(
                                "{}  Pon el ícono del ojo sobre el botón de mirar alrededor del juego, si lo tiene. Mientras mantienes Mirada libre, el mouse mueve el ojo y no la mira.",
                                ic::EYE
                            ),
                        );
                        rejilla(ui, "apuntar-mirar", |ui| {
                            posicion_filas(ui, ("Posición de mirada libre X", "Posición de mirada libre Y"), ojo);
                            fila(ui, "Mirada libre", |ui| selector_tecla(ui, esperando, &tecla_ojo, Objetivo::Ojo));
                        });
                    }

                    ui.add_space(4.0);
                    ui.checkbox(&mut r.mostrar, "Mostrar teclas en pantalla");
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    eliminar = ui.add(egui::Button::new(RichText::new(format!("{}  Eliminar", ic::TRASH)).color(crate::CERRAR_HOVER))).clicked();
                    ui.label(RichText::new("Los cambios se guardan con «Guardar».").size(11.0).color(TENUE));
                });
            });
        if cerrar {
            self.ajustes_apuntar = false;
        }
        if eliminar {
            self.elegida = Some(Asa::Apuntar);
            self.eliminar();
        }
        if elegir_imagen {
            if let Some(ruta) = elegir_imagen_mira() {
                match guardar_imagen_mira(&ruta) {
                    Ok(nombre) => {
                        if let Some(r) = &mut self.mapa.raton {
                            r.mira.image = nombre;
                        }
                        self.error_mira = None;
                    }
                    Err(e) => self.error_mira = Some(e),
                }
            }
        }
    }
}

/// La pestaña «Gamepad» de la puntería, como la de BlueStacks (más un botón para disparar).
fn ajustes_mando(ui: &mut egui::Ui, r: &mut Raton, esperando: &mut Option<Objetivo>) {
    let m = &mut r.mando;
    seccion(ui, "Atajos del mando");
    let (iniciar, suspender, mirar, disparar) = (m.iniciar.clone(), m.suspender.clone(), m.mirar.clone(), m.disparar.clone());
    rejilla(ui, "mando-teclas", |ui| {
        fila(ui, "Iniciar/Detener", |ui| selector_tecla(ui, esperando, &iniciar, Objetivo::Mando(CampoMando::Iniciar)));
        fila(ui, "Suspender", |ui| selector_tecla(ui, esperando, &suspender, Objetivo::Mando(CampoMando::Suspender)));
    });

    seccion(ui, "Propiedades");
    rejilla(ui, "mando-propiedades", |ui| {
        fila(ui, "Joystick", |ui| selector_joystick(ui, "joystick-mando", &mut m.stick));
        fila(ui, "Sensibilidad del mando", |ui| {
            decimal(ui, &mut m.sens, 0.01..=100.0);
        });
        fila(ui, "Zona muerta", |ui| {
            ui.add(egui::DragValue::new(&mut m.zona_muerta).range(0.0..=90.0).speed(0.5).fixed_decimals(0).suffix(" %"));
        });
    });
    nota(ui, "El joystick gira la cámara como el mouse, con su propia sensibilidad; dentro de la zona muerta no la mueve.");

    seccion(ui, "Disparar");
    if r.disparo_con_clic {
        rejilla(ui, "mando-disparo", |ui| {
            fila(ui, "Disparar", |ui| selector_tecla(ui, esperando, &disparar, Objetivo::Mando(CampoMando::Disparar)));
        });
        nota(ui, "Toca la posición de acción de la pestaña «Teclado y mouse».");
    } else {
        nota(ui, "Para disparar con el mando, prende «Disparar con clic izquierdo» en la pestaña «Teclado y mouse».");
    }

    seccion(ui, "Mirar alrededor");
    if r.mirada_pos.is_some() {
        rejilla(ui, "mando-mirar", |ui| {
            fila(ui, "Mirada libre", |ui| selector_tecla(ui, esperando, &mirar, Objetivo::Mando(CampoMando::Mirar)));
        });
    } else {
        nota(ui, "Para mirar alrededor con el mando, prende «Mirar alrededor» en la pestaña «Teclado y mouse».");
    }

    ui.add_space(4.0);
    let conectados = crate::gamepad::CONECTADOS.lock().unwrap().clone();
    nota(ui, &if conectados.is_empty() { "No hay ningún mando conectado.".to_string() } else { format!("Mandos: {}.", conectados.join(", ")) });
    nota(
        ui,
        "Si este juego tiene botones del mando (aquí o en sus controles, en «Mando»), mientras lo juegas el mando es de los controles y no le llega a Android. La posición, la mira y «Ajustes» son los de la otra pestaña.",
    );
}

/// Qué joystick del mando (`LeftStick`, `RightStick` o ninguno).
fn selector_joystick(ui: &mut egui::Ui, id: &str, stick: &mut String) {
    let opciones = [("", "Ninguno"), ("LeftStick", "Izquierdo"), ("RightStick", "Derecho")];
    let actual = opciones.iter().find(|o| o.0 == stick.as_str()).map_or("Ninguno", |o| o.1);
    egui::ComboBox::from_id_salt(id).selected_text(actual).show_ui(ui, |ui| {
        for (valor, nombre) in opciones {
            if ui.selectable_label(stick.as_str() == valor, nombre).clicked() {
                *stick = valor.to_string();
            }
        }
    });
}

/// Una pestaña de la ventana, subrayada si es la elegida (como las de BlueStacks). Devuelve si se tocó.
fn pestana(ui: &mut egui::Ui, elegida: bool, texto: &str) -> bool {
    let galeria = ui.painter().layout_no_wrap(texto.to_string(), FontId::proportional(13.0), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galeria.size().x + 20.0, 28.0), Sense::click());
    let color = if elegida || resp.hovered() { TEXTO } else { TENUE };
    ui.painter().galley_with_override_text_color(rect.center() - galeria.size() / 2.0, galeria, color);
    let linea = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - 2.0), rect.max);
    ui.painter().rect_filled(linea, 1.0, if elegida { ACENTO } else { FONDO_HOVER });
    resp.clicked()
}

/// Lo que hace cada bit de «Ajustes» (los `Tweaks` de BlueStacks) que ZYKRO entiende.
const AYUDA_AJUSTES: &str = "Comportamientos de BlueStacks, sumados (los «Tweaks»).\n\
    0: lo normal: al salirse de la pantalla, el dedo vuelve a empezar en X, Y.\n\
    16450: el de Free Fire (2 + 64 + 16384).\n\
    1: vuelve a empezar cada 200 ms.\n\
    2: nunca vuelve a empezar.\n\
    4: no pasa del 55 % hacia la izquierda.\n\
    8: como mucho 125 movimientos por segundo.\n\
    16: llega hasta 1.2 veces la distancia a la orilla.\n\
    64: redondea a píxeles enteros.\n\
    128: vuelve a empezar si quedó lejos y el mouse casi no se mueve.\n\
    1024: no pasa de la mitad hacia arriba.\n\
    2048: espera 20 ms antes de volver a bajar el dedo.\n\
    8192: límites a los lados (los del archivo).\n\
    16384: la curva de BlueStacks para Free Fire.";

/// Una rejilla de dos columnas (nombre y valor), como los ajustes de BlueStacks.
fn rejilla(ui: &mut egui::Ui, id: &str, contenido: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).spacing(Vec2::new(12.0, 6.0)).min_col_width(150.0).show(ui, contenido);
}

fn fila(ui: &mut egui::Ui, texto: &str, valor: impl FnOnce(&mut egui::Ui)) {
    ui.label(RichText::new(texto).color(ICONO));
    valor(ui);
    ui.end_row();
}

/// Un número con dos decimales. Devuelve si cambió.
fn decimal(ui: &mut egui::Ui, valor: &mut f32, rango: std::ops::RangeInclusive<f32>) -> bool {
    let paso = ((*rango.end() - *rango.start()) / 2000.0).max(0.01);
    ui.add(egui::DragValue::new(valor).range(rango).speed(paso).fixed_decimals(2)).changed()
}

/// Una posición en % de la pantalla (dos filas, X y Y), como la escribe BlueStacks.
fn posicion_filas(ui: &mut egui::Ui, nombres: (&str, &str), p: &mut Punto) {
    let (mut x, mut y) = (p.x * 100.0, p.y * 100.0);
    let mut cambio = false;
    fila(ui, nombres.0, |ui| cambio |= decimal(ui, &mut x, 0.0..=100.0));
    fila(ui, nombres.1, |ui| cambio |= decimal(ui, &mut y, 0.0..=100.0));
    if cambio {
        *p = Punto::nuevo(x / 100.0, y / 100.0);
    }
}

/// El diálogo de KDE (o el de GNOME) para elegir la imagen de la mira.
fn elegir_imagen_mira() -> Option<PathBuf> {
    let inicio = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let tipos = "*.png *.jpg *.jpeg";
    let salida = Command::new("kdialog")
        .args(["--title", "Imagen del punto de mira", "--getopenfilename"])
        .arg(&inicio)
        .arg(format!("{tipos}|Imágenes"))
        .output()
        .or_else(|_| {
            Command::new("zenity").args(["--file-selection", "--title=Imagen del punto de mira", &format!("--file-filter=Imágenes | {tipos}")]).output()
        })
        .ok()?;
    let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    (salida.status.success() && !ruta.is_empty()).then(|| PathBuf::from(ruta))
}

/// El botón con la tecla de un control: al tocarlo, la próxima tecla que presiones se asigna.
fn selector_tecla(ui: &mut egui::Ui, esperando: &mut Option<Objetivo>, tecla: &str, objetivo: Objetivo) {
    let activo = *esperando == Some(objetivo);
    let texto = if activo {
        "Presiona…".to_string()
    } else if tecla.is_empty() {
        if matches!(objetivo, Objetivo::Mando(_) | Objetivo::MandoNodo(..)) { "Sin botón" } else { "Sin tecla" }.to_string()
    } else {
        etiqueta(tecla)
    };
    let mut b = egui::Button::new(RichText::new(texto).strong()).min_size(Vec2::new(120.0, 24.0));
    if activo {
        b = b.fill(ACENTO.gamma_multiply(0.35)).stroke(Stroke::new(1.0, ACENTO));
    }
    if ui.add(b).clicked() {
        *esperando = Some(objetivo);
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn textos(forma: &egui::Shape, salida: &mut Vec<String>) {
        match forma {
            egui::Shape::Text(t) => salida.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|f| textos(f, salida)),
            _ => {}
        }
    }

    const PANTALLA: Vec2 = Vec2::new(1600.0, 900.0);

    /// Un cuadro del editor sobre una pantalla de 1600×900; devuelve los textos dibujados.
    fn cuadro(ctx: &egui::Context, editor: &mut Editor, eventos: Vec<egui::Event>, tiempo: f64) -> Vec<String> {
        let pantalla = Rect::from_min_size(Pos2::ZERO, PANTALLA);
        let entrada = egui::RawInput { screen_rect: Some(pantalla), events: eventos, time: Some(tiempo), ..Default::default() };
        let mut salida = ctx.run_ui(entrada, |ui| editor.pantalla(ui, pantalla));
        let mut dibujados = Vec::new();
        for forma in std::mem::take(&mut salida.shapes) {
            textos(&forma.shape, &mut dibujados);
        }
        salida.drop_without_applying_deltas();
        dibujados
    }

    /// Un clic en el control de puntería abre «Ajustes de apuntar, desplazar y disparar» con los
    /// campos de BlueStacks (y ya no espera una tecla).
    #[test]
    fn la_punteria_abre_sus_ajustes() {
        let ctx = egui::Context::default();
        let mut editor = Editor::new();
        let mut mapa = Mapa::default();
        mapa.raton = Some(Raton::nuevo(Punto::nuevo(0.25, 0.5)));
        editor.abrir("com.dts.freefireth", Some(mapa));
        let pos = Pos2::new(PANTALLA.x * 0.25, PANTALLA.y * 0.5);
        let clic = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        cuadro(&ctx, &mut editor, vec![egui::Event::PointerMoved(pos)], 0.0);
        cuadro(&ctx, &mut editor, vec![clic(true)], 0.05);
        cuadro(&ctx, &mut editor, vec![clic(false)], 0.1);
        assert!(editor.ajustes_apuntar);
        assert_eq!(editor.esperando, None);
        cuadro(&ctx, &mut editor, vec![], 0.2);
        let dibujados = cuadro(&ctx, &mut editor, vec![], 0.3);
        for texto in [
            "Ajustes de apuntar, desplazar y disparar",
            "Iniciar/Detener",
            "Suspender",
            "Sensibilidad del mouse X",
            "Sensibilidad del mouse Y",
            "Ajustes",
            "Aceleración del mouse",
            "Punto de mira",
            "Disparar con clic izquierdo",
            "Posición de acción X",
            "Mirar alrededor",
            "Mostrar teclas en pantalla",
        ] {
            assert!(dibujados.iter().any(|d| d.contains(texto)), "falta «{texto}» en {dibujados:?}");
        }
        let ventana = ctx.memory(|m| m.area_rect(Id::new("ajustes-apuntar"))).expect("la ventana");
        assert!(Rect::from_min_size(Pos2::ZERO, PANTALLA).contains_rect(ventana), "{ventana:?}");

        // Un clic dentro de la ventana no le llega a la pantalla de atrás (no la cierra).
        let dentro = Pos2::new(ventana.center().x, ventana.min.y + 6.0);
        let clic = |pressed| egui::Event::PointerButton { pos: dentro, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        cuadro(&ctx, &mut editor, vec![egui::Event::PointerMoved(dentro)], 0.32);
        cuadro(&ctx, &mut editor, vec![clic(true)], 0.34);
        cuadro(&ctx, &mut editor, vec![clic(false)], 0.36);
        assert!(editor.ajustes_apuntar);
        assert_eq!(editor.elegida, Some(Asa::Apuntar));

        // Con todo prendido sigue cabiendo en la pantalla (lo que no quepa se desplaza adentro).
        if let Some(r) = &mut editor.mapa.raton {
            r.mira.enabled = true;
            r.mirada_pos = Some(Punto::nuevo(0.3, 0.3));
        }
        cuadro(&ctx, &mut editor, vec![], 0.4);
        cuadro(&ctx, &mut editor, vec![], 0.5);
        let ventana = ctx.memory(|m| m.area_rect(Id::new("ajustes-apuntar"))).expect("la ventana");
        assert!(Rect::from_min_size(Pos2::ZERO, PANTALLA).contains_rect(ventana), "{ventana:?}");
    }
}
