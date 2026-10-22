//! Pantalla de carga mientras Android arranca: logo con un anillo que gira, los pasos del arranque
//! (encender, arrancar Android, conectar) y el mensaje del paso actual con los segundos que lleva.

use std::f32::consts::TAU;
use std::time::Instant;

use eframe::egui::{self, epaint::Vertex, text::LayoutJob, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, TextFormat, Vec2};
use egui_phosphor::regular as ic;

use crate::android::Etapa;
use crate::{ACENTO, FONDO_HOVER, FONDO_OSCURO, NOMBRE, TENUE, TEXTO, VERSION};

const ERROR: Color32 = Color32::from_rgb(0xff, 0x8a, 0x80);
const PASOS: [(Etapa, &str); 3] = [(Etapa::Encender, "Encender"), (Etapa::Arrancar, "Arrancar Android"), (Etapa::Conectar, "Conectar")];

/// Lo que muestra la pantalla de carga (copiado del estado de Android).
pub struct Vista {
    pub etapa: Etapa,
    pub desde: Option<Instant>,
    pub mensaje: String,
    pub error: bool,
    /// Hay un cuadro de Android detrás (se perdió la conexión): se oscurece en vez de taparlo.
    pub sobre_video: bool,
    /// Ícono de la app.
    pub logo: egui::TextureId,
}

pub fn dibujar(ui: &egui::Ui, area: Rect, v: &Vista) {
    let p = ui.painter_at(area);
    let t = ui.input(|i| i.time) as f32;
    // En ventanas bajas se encoge todo para que quepa.
    let e = (area.height() / 380.0).clamp(0.65, 1.0);
    let c = area.center();

    if v.sobre_video {
        p.rect_filled(area, 0.0, Color32::from_black_alpha(190));
    } else {
        p.rect_filled(area, 0.0, FONDO_OSCURO);
    }

    // Logo con brillo y anillo girando.
    let logo = c - Vec2::new(0.0, 78.0 * e);
    let pulso = 0.5 + 0.5 * (t * 1.6).sin();
    brillo(&p, logo, (150.0 + 20.0 * pulso) * e, ACENTO.gamma_multiply(0.10 + 0.05 * pulso));
    p.circle_stroke(logo, 58.0 * e, Stroke::new(2.0 * e, FONDO_HOVER));
    cometa(&p, logo, 58.0 * e, t * 3.2, 3.0 * e);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    p.image(v.logo, Rect::from_center_size(logo, Vec2::splat(84.0 * e)), uv, Color32::WHITE);

    // Nombre con letras separadas y versión.
    let mut nombre = LayoutJob::default();
    let formato = TextFormat { font_id: FontId::proportional(22.0 * e), color: TEXTO, extra_letter_spacing: 7.0 * e, ..Default::default() };
    nombre.append(NOMBRE, 0.0, formato);
    let nombre = p.layout_job(nombre);
    let arriba = logo.y + 66.0 * e;
    // El espaciado también queda tras la última letra: se compensa para centrar.
    p.galley(Pos2::new(c.x - (nombre.size().x - 7.0 * e) / 2.0, arriba), nombre, TEXTO);
    p.text(Pos2::new(c.x, arriba + 30.0 * e), Align2::CENTER_TOP, format!("v{VERSION}"), FontId::proportional(11.0 * e), TENUE);

    pasos(&p, Pos2::new(c.x, arriba + 78.0 * e), e, v, t);

    // Mensaje: la primera línea es el estado; las demás, pistas.
    let mut lineas = v.mensaje.lines();
    // Sin mensaje: aún no arrancó nada o ya conectó y falta el primer cuadro.
    let espera = if v.etapa == Etapa::Conectar { "Esperando la imagen de Android…" } else { "Iniciando Android…" };
    let primera = lineas.next().unwrap_or(espera);
    let segundos = v.desde.map(|d| d.elapsed().as_secs()).filter(|s| *s >= 3 && !v.error && v.etapa != Etapa::Conectar);
    let texto = match segundos {
        Some(s) => format!("{primera}  ·  {s} s"),
        None => primera.to_string(),
    };
    let mut y = arriba + 136.0 * e;
    let color = if v.error { ERROR } else { TEXTO };
    p.text(Pos2::new(c.x, y), Align2::CENTER_TOP, texto, FontId::proportional(14.0 * e), color);
    for linea in lineas {
        y += 22.0 * e;
        p.text(Pos2::new(c.x, y), Align2::CENTER_TOP, linea, FontId::proportional(12.5 * e), TENUE);
    }
}

/// Los tres pasos del arranque unidos por una línea: hechos (con ✓), el actual (latiendo) y los pendientes.
fn pasos(p: &Painter, centro: Pos2, e: f32, v: &Vista, t: f32) {
    let separacion = 132.0 * e;
    let actual = PASOS.iter().position(|(etapa, _)| *etapa == v.etapa).unwrap_or(0);
    let x = |i: usize| centro.x + (i as f32 - 1.0) * separacion;
    for i in 0..PASOS.len() - 1 {
        let color = if i < actual { ACENTO } else { FONDO_HOVER };
        p.line_segment([Pos2::new(x(i) + 12.0 * e, centro.y), Pos2::new(x(i + 1) - 12.0 * e, centro.y)], Stroke::new(2.0 * e, color));
    }
    for (i, (_, nombre)) in PASOS.iter().enumerate() {
        let punto = Pos2::new(x(i), centro.y);
        let r = 9.0 * e;
        let color_texto = if i < actual {
            p.circle_filled(punto, r, ACENTO);
            p.text(punto, Align2::CENTER_CENTER, ic::CHECK, FontId::proportional(11.0 * e), Color32::WHITE);
            TEXTO
        } else if i == actual {
            let color = if v.error { ERROR } else { ACENTO };
            let late = 0.5 + 0.5 * (t * 4.0).sin();
            p.circle_filled(punto, r + 5.0 * e * late, color.gamma_multiply(0.18 * (1.0 - late)));
            p.circle_stroke(punto, r, Stroke::new(2.0 * e, color));
            p.circle_filled(punto, 3.5 * e, color);
            TEXTO
        } else {
            p.circle_stroke(punto, r, Stroke::new(2.0 * e, FONDO_HOVER));
            TENUE
        };
        p.text(punto + Vec2::new(0.0, 18.0 * e), Align2::CENTER_TOP, *nombre, FontId::proportional(12.0 * e), color_texto);
    }
}

/// Brillo circular que se desvanece hacia afuera.
fn brillo(p: &Painter, centro: Pos2, radio: f32, color: Color32) {
    let lados = 48;
    let mut malla = Mesh::default();
    malla.vertices.push(Vertex { pos: centro, uv: egui::epaint::WHITE_UV, color });
    for i in 0..=lados {
        let punto = centro + Vec2::angled(i as f32 / lados as f32 * TAU) * radio;
        malla.vertices.push(Vertex { pos: punto, uv: egui::epaint::WHITE_UV, color: Color32::TRANSPARENT });
    }
    for i in 1..=lados as u32 {
        malla.add_triangle(0, i, i + 1);
    }
    p.add(Shape::mesh(malla));
}

/// Arco que gira sobre el anillo, con la cola desvaneciéndose.
fn cometa(p: &Painter, centro: Pos2, radio: f32, angulo: f32, grosor: f32) {
    let tramos = 28;
    let largo = TAU * 0.3;
    for i in 0..tramos {
        let a0 = angulo - largo * i as f32 / tramos as f32;
        let a1 = angulo - largo * (i + 1) as f32 / tramos as f32;
        let alfa = 1.0 - i as f32 / tramos as f32;
        let segmento = [centro + Vec2::angled(a0) * radio, centro + Vec2::angled(a1) * radio];
        p.line_segment(segmento, Stroke::new(grosor, ACENTO.gamma_multiply(alfa)));
    }
}
