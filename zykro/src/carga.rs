//! Pantalla de carga mientras Android arranca, al estilo de BlueStacks 5: el logo, una barra de
//! progreso delgada con el paso actual debajo y, abajo, consejos que van cambiando.

use std::time::Instant;

use eframe::egui::{self, text::LayoutJob, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, TextFormat, Vec2};

use crate::android::Etapa;
use crate::{ACENTO, FONDO_HOVER, FONDO_OSCURO, NOMBRE, TENUE, TEXTO, VERSION};

const ERROR: Color32 = Color32::from_rgb(0xff, 0x8a, 0x80);

/// Se muestran de a uno, cada `CADA_CONSEJO` segundos.
const CONSEJOS: [&str; 7] = [
    "Arrastra hacia abajo desde el borde de arriba para ver las notificaciones, y hacia arriba para cerrarlas.",
    "Presiona ` para activar o suspender la puntería.",
    "F11 pone ZYKRO en pantalla completa.",
    "El botón del control, arriba, abre el menú de controles del juego.",
    "En «Apps abiertas», arrastra una app hacia arriba para cerrarla.",
    "En Configuración, pestaña Rendimiento, puedes darle más potencia a Android.",
    "La rueda del mouse desplaza como si arrastraras el dedo.",
];
const CADA_CONSEJO: f64 = 6.0;

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
    let tiempo = ui.input(|i| i.time);
    // En ventanas bajas se encoge todo para que quepa.
    let e = (area.height() / 420.0).clamp(0.65, 1.0);
    let c = area.center();

    if v.sobre_video {
        p.rect_filled(area, 0.0, Color32::from_black_alpha(200));
    } else {
        p.rect_filled(area, 0.0, FONDO_OSCURO);
    }

    // Logo y nombre con letras separadas.
    let logo = c - Vec2::new(0.0, 46.0 * e);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    p.image(v.logo, Rect::from_center_size(logo, Vec2::splat(72.0 * e)), uv, Color32::WHITE);
    let mut nombre = LayoutJob::default();
    let formato = TextFormat { font_id: FontId::proportional(20.0 * e), color: TEXTO, extra_letter_spacing: 6.0 * e, ..Default::default() };
    nombre.append(NOMBRE, 0.0, formato);
    let nombre = p.layout_job(nombre);
    let y_nombre = logo.y + 46.0 * e;
    // El espaciado también queda tras la última letra: se compensa para centrar.
    p.galley(Pos2::new(c.x - (nombre.size().x - 6.0 * e) / 2.0, y_nombre), nombre, TEXTO);

    // Barra de progreso.
    let barra = Rect::from_center_size(Pos2::new(c.x, y_nombre + 56.0 * e), Vec2::new(300.0 * e, 4.0 * e));
    let meta = if v.error { progreso(v).min(0.97) } else { progreso(v) };
    let avance = ui.ctx().animate_value_with_time(Id::new("carga-progreso"), meta, 0.5);
    let color = if v.error { ERROR } else { ACENTO };
    p.rect_filled(barra, CornerRadius::same(2), FONDO_HOVER);
    let lleno = Rect::from_min_size(barra.min, Vec2::new(barra.width() * avance, barra.height()));
    p.rect_filled(lleno, CornerRadius::same(2), color);
    if !v.error {
        // Un destello recorre la parte llena.
        let fase = (tiempo * 0.7).fract() as f32;
        let x = lleno.min.x + (lleno.width() + 60.0 * e) * fase - 30.0 * e;
        let destello = Rect::from_center_size(Pos2::new(x, barra.center().y), Vec2::new(60.0 * e, barra.height())).intersect(lleno);
        if destello.width() > 0.0 {
            p.rect_filled(destello, CornerRadius::same(2), Color32::from_white_alpha(70));
        }
    }

    // Paso actual (primera línea del mensaje) y pistas debajo.
    let mut lineas = v.mensaje.lines();
    let espera = if v.etapa == Etapa::Conectar { "Esperando la imagen de Android…" } else { "Iniciando Android…" };
    let primera = lineas.next().unwrap_or(espera);
    let mut y = barra.max.y + 14.0 * e;
    p.text(Pos2::new(c.x, y), Align2::CENTER_TOP, primera, FontId::proportional(12.5 * e), if v.error { ERROR } else { TENUE });
    for linea in lineas {
        y += 19.0 * e;
        p.text(Pos2::new(c.x, y), Align2::CENTER_TOP, linea, FontId::proportional(12.0 * e), TENUE);
    }

    // Consejo abajo y versión en la esquina.
    let consejo = CONSEJOS[(tiempo / CADA_CONSEJO) as usize % CONSEJOS.len()];
    let abajo = area.max.y - 34.0 * e;
    let mut texto = LayoutJob::default();
    let fuente = FontId::proportional(12.5 * e);
    texto.append("Consejo  ", 0.0, TextFormat { font_id: fuente.clone(), color: ACENTO, ..Default::default() });
    texto.append(consejo, 0.0, TextFormat { font_id: fuente, color: TEXTO.gamma_multiply(0.85), ..Default::default() });
    let texto = p.layout_job(texto);
    p.galley(Pos2::new(c.x - texto.size().x / 2.0, abajo - texto.size().y / 2.0), texto, TEXTO);
    p.text(area.max - Vec2::splat(10.0 * e), Align2::RIGHT_BOTTOM, format!("v{VERSION}"), FontId::proportional(10.5 * e), TENUE);
}

/// Avance estimado del arranque (0 a 1): cada paso ocupa un tramo de la barra y se acerca a su final
/// con el tiempo, sin llegar, porque no se sabe cuánto va a tardar.
fn progreso(v: &Vista) -> f32 {
    let s = v.desde.map(|d| d.elapsed().as_secs_f32()).unwrap_or(0.0);
    let acercar = |desde: f32, hasta: f32, ritmo: f32| desde + (hasta - desde) * (1.0 - (-s / ritmo).exp());
    match v.etapa {
        Etapa::Encender => acercar(0.03, 0.15, 2.0),
        Etapa::Arrancar => acercar(0.15, 0.85, 8.0),
        Etapa::Conectar => acercar(0.85, 0.98, 1.5),
    }
}
