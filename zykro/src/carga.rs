//! Pantalla de carga mientras Android arranca, al estilo de BlueStacks 5: una ilustración que cubre
//! toda la imagen (un cielo con planetas, dibujado aquí con los colores de ZYKRO) y abajo una franja
//! con el paso actual y una barra de progreso delgada.

use std::f32::consts::TAU;
use std::time::Instant;

use eframe::egui::{self, text::LayoutJob, Color32, CornerRadius, FontId, Id, Mesh, Painter, Pos2, Rect, Shape, Stroke, TextFormat, Vec2};

use crate::android::Etapa;
use crate::{ACENTO, FONDO, FONDO_HOVER, NOMBRE, TEXTO};

const ERROR: Color32 = Color32::from_rgb(0xff, 0x8a, 0x80);

/// Se muestran de a uno, cada `CADA_CONSEJO` segundos.
const CONSEJOS: [&str; 7] = [
    "Arrastra hacia abajo desde el borde de arriba para ver las notificaciones, y hacia arriba para cerrarlas.",
    "Presiona ` para activar o suspender la puntería.",
    "F11 pone ZYKRO en pantalla completa.",
    "El botón del teclado, en la barra lateral, abre el menú de controles del juego.",
    "En «Apps abiertas», arrastra una app hacia arriba para cerrarla.",
    "Mantén presionado un espacio vacío del inicio para cambiar el fondo de pantalla.",
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

/// Un planeta: centro (en partes del ancho y el alto), radio (en partes del alto), colores de la cara
/// iluminada y del borde en sombra, manchas y, si tiene, anillo (color e inclinación en grados).
struct Planeta {
    x: f32,
    y: f32,
    radio: f32,
    luz: u32,
    sombra: u32,
    manchas: &'static [(f32, f32, f32)],
    anillo: Option<(u32, f32)>,
}

#[rustfmt::skip]
const PLANETAS: [Planeta; 6] = [
    Planeta { x: 0.04, y: 0.40, radio: 0.22, luz: 0xF4A259, sombra: 0x7A1F55, manchas: &[(-0.3, -0.2, 0.32), (0.25, 0.35, 0.22), (0.4, -0.45, 0.14)], anillo: None },
    Planeta { x: 0.98, y: 0.50, radio: 0.21, luz: 0xE0569B, sombra: 0x3E1150, manchas: &[(-0.35, 0.1, 0.3), (0.1, -0.4, 0.2), (0.2, 0.45, 0.16)], anillo: None },
    Planeta { x: 0.17, y: 0.70, radio: 0.065, luz: 0xFFD27A, sombra: 0xB4481C, manchas: &[(-0.2, 0.25, 0.3)], anillo: Some((0xE7A73C, -18.0)) },
    Planeta { x: 0.83, y: 0.74, radio: 0.055, luz: 0xFF7A99, sombra: 0x861A42, manchas: &[(0.2, -0.2, 0.3)], anillo: Some((0xD9A445, 14.0)) },
    Planeta { x: 0.23, y: 0.12, radio: 0.035, luz: 0xA9DDFF, sombra: 0x2C4C8F, manchas: &[(0.1, 0.1, 0.35)], anillo: None },
    Planeta { x: 0.79, y: 0.15, radio: 0.042, luz: 0xFFB8D6, sombra: 0x962A68, manchas: &[(-0.15, 0.2, 0.32)], anillo: None },
];

pub fn dibujar(ui: &egui::Ui, area: Rect, v: &Vista) {
    let p = ui.painter_at(area);
    let tiempo = ui.input(|i| i.time) as f32;
    // En ventanas bajas se encoge todo para que quepa.
    let e = (area.height() / 420.0).clamp(0.65, 1.0);
    let barra_alto = 40.0 * e;
    let escena = Rect::from_min_max(area.min, egui::pos2(area.max.x, area.max.y - barra_alto));

    if v.sobre_video {
        p.rect_filled(escena, 0.0, Color32::from_black_alpha(200));
    } else {
        cielo(&p, escena, tiempo);
        let c = escena.center() - Vec2::new(0.0, 18.0 * e);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        p.image(v.logo, Rect::from_center_size(c, Vec2::splat(64.0 * e)), uv, Color32::WHITE);
        let mut nombre = LayoutJob::default();
        let formato = TextFormat { font_id: FontId::proportional(19.0 * e), color: TEXTO, extra_letter_spacing: 6.0 * e, ..Default::default() };
        nombre.append(NOMBRE, 0.0, formato);
        let nombre = p.layout_job(nombre);
        // El espaciado también queda tras la última letra: se compensa para centrar.
        p.galley(Pos2::new(c.x - (nombre.size().x - 6.0 * e) / 2.0, c.y + 42.0 * e), nombre, TEXTO);
    }

    // Las pistas (si el arranque se trabó) o el consejo, sobre la franja de abajo.
    let mut lineas = v.mensaje.lines();
    let espera = match v.etapa {
        Etapa::Conectar => "Esperando la imagen de Android…",
        _ => "Iniciando ZYKRO",
    };
    let paso = lineas.next().unwrap_or(espera);
    let pistas: Vec<&str> = lineas.collect();
    let fuente = FontId::proportional(12.5 * e);
    let texto = if pistas.is_empty() {
        let consejo = CONSEJOS[(tiempo as f64 / CADA_CONSEJO) as usize % CONSEJOS.len()];
        let mut t = LayoutJob::default();
        t.append("Consejo  ", 0.0, TextFormat { font_id: fuente.clone(), color: ACENTO, ..Default::default() });
        t.append(consejo, 0.0, TextFormat { font_id: fuente.clone(), color: TEXTO, ..Default::default() });
        t
    } else {
        LayoutJob::simple(pistas.join("\n"), fuente.clone(), TEXTO, escena.width() - 80.0 * e)
    };
    let texto = p.layout_job(texto);
    let caja = Rect::from_center_size(
        Pos2::new(escena.center().x, escena.max.y - 18.0 * e - texto.size().y / 2.0 - 8.0 * e),
        texto.size() + Vec2::new(28.0, 16.0) * e,
    );
    p.rect_filled(caja, CornerRadius::same((caja.height() / 2.0).min(14.0) as u8), Color32::from_black_alpha(130));
    p.galley(caja.center() - texto.size() / 2.0, texto, TEXTO);

    // Franja de abajo: el paso a la izquierda y la barra hasta el otro lado.
    let franja = Rect::from_min_max(egui::pos2(area.min.x, escena.max.y), area.max);
    p.rect_filled(franja, 0.0, FONDO);
    let color = if v.error { ERROR } else { TEXTO };
    let etiqueta = p.layout_no_wrap(paso.to_string(), FontId::proportional(13.0 * e), color);
    let x0 = franja.min.x + 14.0 * e;
    p.galley(Pos2::new(x0, franja.center().y - etiqueta.size().y / 2.0), etiqueta.clone(), color);
    let inicio = x0 + etiqueta.size().x + 16.0 * e;
    let barra = Rect::from_min_max(Pos2::new(inicio, franja.center().y - 1.5 * e), Pos2::new(franja.max.x - 14.0 * e, franja.center().y + 1.5 * e));
    let meta = if v.error { progreso(v).min(0.97) } else { progreso(v) };
    let avance = ui.ctx().animate_value_with_time(Id::new("carga-progreso"), meta, 0.5);
    p.rect_filled(barra, CornerRadius::same(2), FONDO_HOVER);
    let lleno = Rect::from_min_size(barra.min, Vec2::new(barra.width() * avance, barra.height()));
    p.rect_filled(lleno, CornerRadius::same(2), if v.error { ERROR } else { ACENTO });
    if !v.error {
        // Un destello recorre la parte llena.
        let fase = (tiempo * 0.7).fract();
        let x = lleno.min.x + (lleno.width() + 80.0 * e) * fase - 40.0 * e;
        let destello = Rect::from_center_size(Pos2::new(x, barra.center().y), Vec2::new(80.0 * e, barra.height())).intersect(lleno);
        if destello.width() > 0.0 {
            p.rect_filled(destello, CornerRadius::same(2), Color32::from_white_alpha(90));
        }
    }
}

/// El cielo: degradado violeta, un brillo al centro, estrellas que titilan, planetas que flotan
/// despacio y una luna con cráteres abajo.
fn cielo(p: &Painter, r: Rect, t: f32) {
    let (w, h) = (r.width(), r.height());
    degradado(p, r, color(0x1A0F3F), color(0x0C0720));
    brillo(p, r.center() - Vec2::new(0.0, 0.08 * h), Vec2::new(0.42 * w, 0.55 * h), Color32::from_rgba_unmultiplied(0x8B, 0x3D, 0xF0, 90));

    // Estrellas: puntos y destellos de cuatro puntas, siempre en el mismo lugar.
    for i in 0..140u32 {
        let (x, y) = (azar(i * 3) * w, azar(i * 3 + 1) * h);
        let fase = azar(i * 3 + 2) * TAU;
        let brillo = 0.35 + 0.65 * (0.5 + 0.5 * (t * (0.8 + azar(i) * 1.6) + fase).sin());
        let centro = r.min + Vec2::new(x, y);
        if i % 9 == 0 {
            let largo = (4.0 + azar(i + 7) * 6.0) * (h / 600.0).max(0.6);
            let c = Color32::from_white_alpha((200.0 * brillo) as u8);
            p.add(Shape::convex_polygon(vec![centro - Vec2::X * largo, centro - Vec2::Y * largo * 0.22, centro + Vec2::X * largo, centro + Vec2::Y * largo * 0.22], c, Stroke::NONE));
            p.add(Shape::convex_polygon(vec![centro - Vec2::Y * largo, centro - Vec2::X * largo * 0.22, centro + Vec2::Y * largo, centro + Vec2::X * largo * 0.22], c, Stroke::NONE));
        } else {
            let tonos = [0xFFFFFF, 0xF472B6, 0x60A5FA];
            let base = color(tonos[(i % 3) as usize]);
            p.circle_filled(centro, 0.8 + azar(i + 11) * 1.6, base.gamma_multiply(0.75 * brillo));
        }
    }

    // Colinas en las esquinas de abajo y la luna al centro.
    esfera(p, r.left_bottom() + Vec2::new(0.02 * w, 0.06 * h), 0.24 * h, color(0xB4357A), color(0x3D0B2E));
    esfera(p, r.right_bottom() + Vec2::new(-0.03 * w, 0.08 * h), 0.27 * h, color(0xA42F75), color(0x360A2B));
    let radio_luna = 0.62 * w;
    let luna = Pos2::new(r.center().x, r.max.y + radio_luna - 0.2 * h);
    brillo(p, luna - Vec2::new(0.0, radio_luna), Vec2::new(0.5 * w, 0.16 * h), Color32::from_rgba_unmultiplied(0xA7, 0x6B, 0xFF, 70));
    esfera(p, luna, radio_luna, color(0x9B6BFF), color(0x2E1466));
    // Cráteres sobre la superficie (siguen su curva): fondo oscuro y el borde de abajo iluminado.
    for &(dx, abajo, ancho) in &[(-0.24, 0.45, 0.055), (-0.07, 0.25, 0.035), (0.12, 0.5, 0.065), (0.27, 0.3, 0.032), (-0.13, 0.75, 0.03), (0.02, 0.7, 0.045)] {
        let x = dx * w;
        let borde = luna.y - (radio_luna * radio_luna - x * x).max(0.0).sqrt();
        let c = Pos2::new(r.center().x + x, borde + (r.max.y - borde) * abajo);
        let radio = Vec2::new(ancho * w, ancho * w * 0.34);
        p.add(Shape::ellipse_filled(c + Vec2::new(0.0, radio.y * 0.3), radio, color(0xB99BFF).gamma_multiply(0.55)));
        p.add(Shape::ellipse_filled(c, radio, color(0x3A1B85).gamma_multiply(0.9)));
    }

    for (i, pl) in PLANETAS.iter().enumerate() {
        let flota = (t * 0.45 + i as f32 * 1.7).sin() * 0.006 * h;
        let c = r.min + Vec2::new(pl.x * w, pl.y * h + flota);
        let radio = pl.radio * h;
        brillo(p, c, Vec2::splat(radio * 1.45), color(pl.luz).gamma_multiply(0.18));
        if let Some((tono, grados)) = pl.anillo {
            anillo(p, c, radio, color(tono), grados, false);
        }
        esfera(p, c, radio, color(pl.luz), color(pl.sombra));
        for &(dx, dy, tam) in pl.manchas {
            let m = c + Vec2::new(dx, dy) * radio;
            p.add(Shape::ellipse_filled(m, Vec2::new(tam * radio, tam * radio * 0.6), color(pl.sombra).gamma_multiply(0.35)));
        }
        if let Some((tono, grados)) = pl.anillo {
            anillo(p, c, radio, color(tono), grados, true);
        }
    }
}

/// Un círculo con volumen: la cara iluminada arriba a la izquierda y el borde en sombra.
fn esfera(p: &Painter, c: Pos2, radio: f32, luz: Color32, sombra: Color32) {
    let mut malla = Mesh::default();
    malla.colored_vertex(c - Vec2::splat(radio * 0.35), luz);
    let n = 64;
    for i in 0..=n {
        let a = i as f32 / n as f32 * TAU;
        malla.colored_vertex(c + Vec2::angled(a) * radio, sombra);
    }
    for i in 0..n {
        malla.add_triangle(0, 1 + i, 2 + i);
    }
    p.add(malla);
}

/// Un anillo inclinado `grados`: la mitad de atrás se dibuja antes del planeta y la de adelante después.
fn anillo(p: &Painter, c: Pos2, radio: f32, tono: Color32, grados: f32, adelante: bool) {
    let giro = grados.to_radians();
    let (ancho, alto) = (radio * 1.9, radio * 0.42);
    let desde = if adelante { 0.0 } else { TAU / 2.0 };
    let puntos: Vec<Pos2> = (0..=32)
        .map(|i| {
            let a = desde + i as f32 / 32.0 * TAU / 2.0;
            let (x, y) = (a.cos() * ancho, a.sin() * alto);
            c + Vec2::new(x * giro.cos() - y * giro.sin(), x * giro.sin() + y * giro.cos())
        })
        .collect();
    p.add(Shape::line(puntos, Stroke::new((radio * 0.16).max(2.0), tono.gamma_multiply(if adelante { 0.95 } else { 0.6 }))));
}

/// Un brillo redondo que se desvanece hacia afuera.
fn brillo(p: &Painter, c: Pos2, radio: Vec2, tono: Color32) {
    let mut malla = Mesh::default();
    malla.colored_vertex(c, tono);
    let n = 48;
    for i in 0..=n {
        let a = i as f32 / n as f32 * TAU;
        malla.colored_vertex(c + Vec2::new(a.cos() * radio.x, a.sin() * radio.y), Color32::TRANSPARENT);
    }
    for i in 0..n {
        malla.add_triangle(0, 1 + i, 2 + i);
    }
    p.add(malla);
}

fn degradado(p: &Painter, r: Rect, arriba: Color32, abajo: Color32) {
    let mut malla = Mesh::default();
    malla.colored_vertex(r.left_top(), arriba);
    malla.colored_vertex(r.right_top(), arriba);
    malla.colored_vertex(r.right_bottom(), abajo);
    malla.colored_vertex(r.left_bottom(), abajo);
    malla.add_triangle(0, 1, 2);
    malla.add_triangle(0, 2, 3);
    p.add(malla);
}

fn color(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// De 0 a 1, fijo para cada `n`.
fn azar(n: u32) -> f32 {
    let mut h = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    h = (h ^ (h >> 12)).wrapping_mul(0x297A_2D39);
    (h ^ (h >> 15)) as f32 / u32::MAX as f32
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
