//! Fondos de pantalla de la pantalla de inicio de ZYKRO, en una ventana con el tema de ZYKRO. Se abre
//! manteniendo presionado un espacio vacío del inicio. Los fondos de ZYKRO se dibujan aquí (degradados
//! con brillos, sin imágenes de nadie); también sirve una imagen de la laptop o de la galería de
//! Android. ZYKRO deja la imagen en la carpeta de la pantalla de inicio y le pide ponerla
//! (`Fondo.java`): ella la recorta a la pantalla y la dibuja detrás de los íconos.

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Align, Align2, ColorImage, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use egui_phosphor::regular as ic;

use crate::ajustes::{linea, nota, titulo, ANCHO_CONTENIDO, ANCHO_PESTANAS};
use crate::android::{self, EstadoCompartido};
use crate::{ACENTO, CERRAR_HOVER, FONDO, FONDO_CONTROL, FONDO_HOVER, TENUE, TEXTO};

/// Carpeta de Android de la pantalla de inicio, donde deja el fondo puesto y su miniatura.
const EXPORTADO: &str = "/data/media/0/Android/data/com.zykro.inicio/files";
/// Miniaturas de los fondos de ZYKRO (16:9) y cuántas van por fila.
const MINI: [usize; 2] = [256, 144];
const COLUMNAS: usize = 4;
const SEP: f32 = 12.0;

/// Un fondo de ZYKRO: un degradado de arriba a abajo (colores en hexadecimal) con brillos suaves
/// encima: centro (de 0 a 1 en cada lado), radio (por el lado más largo), color y fuerza.
struct Diseno {
    nombre: &'static str,
    arriba: u32,
    abajo: u32,
    brillos: &'static [(f32, f32, f32, u32, f32)],
}

/// El primero es el de siempre de la pantalla de inicio (elegirlo quita el fondo puesto).
#[rustfmt::skip]
const DISENOS: [Diseno; 12] = [
    Diseno { nombre: "ZYKRO", arriba: 0x1B1729, abajo: 0x0E0B17, brillos: &[] },
    Diseno { nombre: "Violeta", arriba: 0x130F21, abajo: 0x09070F, brillos: &[(0.12, 0.05, 0.55, 0xA855F7, 0.50), (0.92, 1.0, 0.50, 0x6D28D9, 0.45)] },
    Diseno { nombre: "Neón", arriba: 0x08070E, abajo: 0x0A0714, brillos: &[(0.08, 0.95, 0.45, 0x22D3EE, 0.40), (0.88, 0.10, 0.50, 0xC026D3, 0.45)] },
    Diseno { nombre: "Océano", arriba: 0x07131F, abajo: 0x030911, brillos: &[(0.25, -0.05, 0.60, 0x0EA5E9, 0.40), (0.80, 1.05, 0.55, 0x1D4ED8, 0.45)] },
    Diseno { nombre: "Aurora", arriba: 0x061210, abajo: 0x05090C, brillos: &[(0.28, 0.18, 0.42, 0x10B981, 0.40), (0.68, 0.30, 0.40, 0x06B6D4, 0.32), (0.55, 1.05, 0.55, 0x6D28D9, 0.30)] },
    Diseno { nombre: "Atardecer", arriba: 0x170C18, abajo: 0x0C060E, brillos: &[(0.55, 1.08, 0.55, 0xF97316, 0.48), (0.12, 0.90, 0.42, 0xDB2777, 0.40), (0.90, 0.15, 0.38, 0x7C3AED, 0.28)] },
    Diseno { nombre: "Bosque", arriba: 0x0A140E, abajo: 0x050A07, brillos: &[(0.18, 0.12, 0.50, 0x22C55E, 0.28), (0.85, 0.95, 0.55, 0x15803D, 0.35)] },
    Diseno { nombre: "Brasas", arriba: 0x140A09, abajo: 0x090505, brillos: &[(0.82, 0.90, 0.50, 0xEF4444, 0.42), (0.18, 0.08, 0.42, 0xF59E0B, 0.24)] },
    Diseno { nombre: "Índigo", arriba: 0x0D0F24, abajo: 0x06070F, brillos: &[(0.75, 0.15, 0.50, 0x6366F1, 0.45), (0.15, 0.95, 0.45, 0x312E81, 0.50)] },
    Diseno { nombre: "Rosa", arriba: 0x1A0B16, abajo: 0x0C060B, brillos: &[(0.20, 0.20, 0.50, 0xEC4899, 0.42), (0.85, 0.90, 0.45, 0x9D174D, 0.45)] },
    Diseno { nombre: "Ámbar", arriba: 0x161007, abajo: 0x0A0804, brillos: &[(0.50, 1.05, 0.60, 0xF59E0B, 0.40), (0.10, 0.10, 0.35, 0xB45309, 0.30)] },
    Diseno { nombre: "Grafito", arriba: 0x1B1B21, abajo: 0x0B0B0E, brillos: &[(0.50, -0.15, 0.75, 0xE5E7EB, 0.10)] },
];

/// El fondo que tiene puesto la pantalla de inicio.
struct Actual {
    /// El nombre del fondo de ZYKRO, o vacío si es una imagen propia.
    nombre: String,
    /// La miniatura de la imagen propia.
    imagen: Option<ColorImage>,
}

#[derive(Default)]
pub struct Fondos {
    abierto: bool,
    miniaturas: Vec<TextureHandle>,
    leido: Arc<Mutex<Option<Actual>>>,
    nombre: Option<String>,
    imagen: Option<TextureHandle>,
    /// Se está preparando un fondo o está abierto el diálogo para elegir una imagen.
    ocupado: Arc<AtomicBool>,
}

impl Fondos {
    pub fn abierto(&self) -> bool {
        self.abierto
    }

    pub fn abrir(&mut self, ctx: &egui::Context) {
        self.abierto = true;
        self.recargar(ctx);
    }

    /// Vuelve a leer qué fondo tiene puesto la pantalla de inicio (en otro hilo).
    pub fn recargar(&self, ctx: &egui::Context) {
        if !self.abierto {
            return;
        }
        let (leido, ctx) = (self.leido.clone(), ctx.clone());
        thread::spawn(move || {
            *leido.lock().unwrap() = Some(leer());
            ctx.request_repaint();
        });
    }

    pub fn mostrar(&mut self, ctx: &egui::Context, estado: &EstadoCompartido) {
        if !self.abierto {
            return;
        }
        if self.miniaturas.is_empty() {
            self.miniaturas = DISENOS
                .iter()
                .map(|d| ctx.load_texture(format!("fondo-{}", d.nombre), imagen(d, MINI[0], MINI[1]), TextureOptions::LINEAR))
                .collect();
        }
        if let Some(actual) = self.leido.lock().unwrap().take() {
            self.imagen = actual.imagen.map(|i| ctx.load_texture("fondo-actual", i, TextureOptions::LINEAR));
            self.nombre = Some(actual.nombre);
        }
        let ocupado = self.ocupado.load(Ordering::Relaxed);
        let mut cerrar = false;
        let marco = egui::Frame::new().fill(FONDO).corner_radius(8.0).stroke(Stroke::new(1.0, FONDO_HOVER));
        let alto = (ctx.content_rect().height() - 110.0).clamp(240.0, 560.0);
        let respuesta = egui::Modal::new(egui::Id::new("fondos")).frame(marco).show(ctx, |ui| {
            ui.set_width(ANCHO_PESTANAS + ANCHO_CONTENIDO);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            ui.horizontal(|ui| {
                ui.set_height(40.0);
                ui.add_space(16.0);
                ui.label(RichText::new(ic::IMAGE).size(16.0).color(ACENTO));
                ui.label(RichText::new("Fondo de pantalla").size(15.0).strong().color(TEXTO));
                if ocupado {
                    ui.add(egui::Spinner::new().size(14.0).color(TENUE));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(6.0);
                    if crate::icono(ui, ic::X, "Cerrar", Vec2::new(34.0, 30.0), CERRAR_HOVER).clicked() {
                        cerrar = true;
                    }
                });
            });
            linea(ui);
            egui::ScrollArea::vertical().auto_shrink([false, true]).max_height(alto).min_scrolled_height(alto).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 12)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    self.actual(ui, ctx, estado, ocupado, &mut cerrar);
                    ui.add_space(4.0);
                    titulo(ui, "Fondos de ZYKRO");
                    self.disenos(ui, ctx, estado, ocupado);
                });
            });
        });
        if respuesta.should_close() || cerrar {
            self.abierto = false;
        }
    }

    /// El fondo puesto y los botones para usar una imagen propia.
    fn actual(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, estado: &EstadoCompartido, ocupado: bool, cerrar: &mut bool) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(224.0, 126.0), Sense::hover());
            let elegido = self.elegido();
            match (&self.imagen, elegido) {
                (Some(t), _) => egui::Image::from_texture(t).corner_radius(8.0).paint_at(ui, rect),
                (None, Some(i)) => egui::Image::from_texture(&self.miniaturas[i]).corner_radius(8.0).paint_at(ui, rect),
                (None, None) => {
                    ui.painter().rect_filled(rect, 8.0, FONDO_CONTROL);
                }
            }
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                titulo(ui, "Fondo actual");
                nota(
                    ui,
                    match (&self.nombre, elegido) {
                        (None, _) => "Leyendo…",
                        (Some(_), Some(i)) => DISENOS[i].nombre,
                        (Some(_), None) => "Imagen propia",
                    },
                );
                ui.add_space(4.0);
                let boton = |texto: String| {
                    egui::Button::new(RichText::new(texto).size(13.5).color(TEXTO)).min_size(Vec2::new(250.0, 34.0)).fill(FONDO_CONTROL)
                };
                if ui.add_enabled(!ocupado, boton(format!("{}   Imagen de la computadora…", ic::DESKTOP))).clicked() {
                    self.elegir_de_la_laptop(ctx, estado);
                }
                if ui.add_enabled(!ocupado, boton(format!("{}   Galería de Android…", ic::IMAGES))).clicked() {
                    thread::spawn(|| android::shell("am start -n com.zykro.inicio/.ElegirFondo"));
                    *cerrar = true;
                }
            });
        });
    }

    /// Los fondos de ZYKRO en miniatura: tocar uno lo pone.
    fn disenos(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, estado: &EstadoCompartido, ocupado: bool) {
        let ancho = (ui.available_width() - SEP * (COLUMNAS - 1) as f32) / COLUMNAS as f32;
        let alto_mini = ancho * MINI[1] as f32 / MINI[0] as f32;
        let elegido = self.elegido();
        for fila in (0..DISENOS.len()).collect::<Vec<_>>().chunks(COLUMNAS) {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), alto_mini + 24.0), Sense::hover());
            for (j, &i) in fila.iter().enumerate() {
                let mini = Rect::from_min_size(Pos2::new(r.min.x + j as f32 * (ancho + SEP), r.min.y), Vec2::new(ancho, alto_mini));
                // `interact` y no `allocate_rect`: la fila ya reservó su lugar.
                let resp = ui.interact(mini, ui.id().with(("fondo", i)), if ocupado { Sense::hover() } else { Sense::click() });
                egui::Image::from_texture(&self.miniaturas[i]).corner_radius(8.0).paint_at(ui, mini);
                let borde = if elegido == Some(i) {
                    Stroke::new(2.0, ACENTO)
                } else if resp.hovered() {
                    Stroke::new(2.0, FONDO_HOVER)
                } else {
                    Stroke::new(1.0, FONDO_HOVER)
                };
                ui.painter().rect_stroke(mini, 8.0, borde, egui::StrokeKind::Inside);
                if elegido == Some(i) {
                    let marca = Pos2::new(mini.max.x - 16.0, mini.min.y + 16.0);
                    ui.painter().circle_filled(marca, 10.0, ACENTO);
                    ui.painter().text(marca, Align2::CENTER_CENTER, ic::CHECK, FontId::proportional(12.0), TEXTO);
                }
                let nombre = Pos2::new(mini.center().x, mini.max.y + 12.0);
                ui.painter().text(nombre, Align2::CENTER_CENTER, DISENOS[i].nombre, FontId::proportional(12.5), TEXTO);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.poner_diseno(ctx, estado, i);
                }
            }
        }
    }

    /// Cuál de los fondos de ZYKRO está puesto (ninguno si es una imagen propia).
    fn elegido(&self) -> Option<usize> {
        let nombre = self.nombre.as_deref()?;
        DISENOS.iter().position(|d| d.nombre == nombre)
    }

    fn poner_diseno(&mut self, ctx: &egui::Context, estado: &EstadoCompartido, i: usize) {
        self.nombre = Some(DISENOS[i].nombre.to_string());
        self.imagen = None;
        let pantalla = estado.lock().unwrap().pantalla.unwrap_or((1280, 720));
        self.trabajar(ctx, estado, move || {
            if i == 0 {
                return pedir(None, DISENOS[0].nombre);
            }
            let (ancho, alto) = (pantalla.0 as usize, pantalla.1 as usize);
            let ruta = cache().join("fondo.png");
            guardar_png(&ruta, &pintar(&DISENOS[i], ancho, alto), ancho, alto) && pedir(Some(&ruta), DISENOS[i].nombre)
        });
    }

    /// Abre el diálogo de KDE (o el de GNOME) para elegir una imagen de la laptop.
    fn elegir_de_la_laptop(&mut self, ctx: &egui::Context, estado: &EstadoCompartido) {
        self.trabajar(ctx, estado, || match dialogo() {
            Ok(Some(ruta)) => pedir(Some(&ruta), ""),
            Ok(None) => true,
            Err(()) => false,
        });
    }

    /// Corre `trabajo` en otro hilo con la ventana ocupada; si falla, avisa.
    fn trabajar(&self, ctx: &egui::Context, estado: &EstadoCompartido, trabajo: impl FnOnce() -> bool + Send + 'static) {
        if self.ocupado.swap(true, Ordering::Relaxed) {
            return;
        }
        let (ocupado, estado, ctx) = (self.ocupado.clone(), estado.clone(), ctx.clone());
        thread::spawn(move || {
            if !trabajo() {
                android::avisar(&estado, "No se pudo cambiar el fondo de pantalla");
            }
            ocupado.store(false, Ordering::Relaxed);
            ctx.request_repaint();
        });
    }
}

/// Deja `imagen` en la carpeta de la pantalla de inicio y le pide ponerla de fondo; sin imagen,
/// le pide volver al de siempre.
fn pedir(imagen: Option<&Path>, nombre: &str) -> bool {
    let mut orden = "am broadcast -n com.zykro.inicio/.Acciones -a com.zykro.inicio.FONDO".to_string();
    if let Some(imagen) = imagen {
        let destino = format!("{EXPORTADO}/fondo-nuevo");
        if !android::copiar_a(imagen, &destino) {
            return false;
        }
        orden = format!("chmod 644 {destino}; {orden} --es archivo fondo-nuevo --es nombre '{}'", nombre.replace('\'', ""));
    }
    android::shell(&orden).is_some()
}

/// La imagen elegida en el diálogo de la laptop; `Ok(None)` si se canceló y `Err` si no hay diálogo.
fn dialogo() -> Result<Option<PathBuf>, ()> {
    let tipos = "*.png *.jpg *.jpeg *.webp *.bmp *.gif *.heic";
    let inicio = std::env::var("HOME").unwrap_or_default();
    let salida = Command::new("kdialog")
        .args(["--title", "Fondo de pantalla", "--getopenfilename", &inicio, &format!("{tipos}|Imágenes")])
        .output()
        .or_else(|_| {
            Command::new("zenity")
                .args(["--file-selection", "--title=Fondo de pantalla", &format!("--file-filter=Imágenes | {tipos}")])
                .output()
        })
        .map_err(|_| ())?;
    let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    Ok((salida.status.success() && !ruta.is_empty()).then(|| PathBuf::from(ruta)))
}

/// Lee qué fondo tiene puesto la pantalla de inicio (y la miniatura, si es una imagen propia).
fn leer() -> Actual {
    let respuesta = android::shell(&format!("cd {EXPORTADO} && [ -f fondo.png ] && echo imagen && cat fondo-nombre 2>/dev/null; true"));
    let respuesta = respuesta.unwrap_or_default();
    let mut lineas = respuesta.lines();
    if lineas.next() != Some("imagen") {
        return Actual { nombre: DISENOS[0].nombre.to_string(), imagen: None };
    }
    let nombre = lineas.next().unwrap_or("").trim().to_string();
    let local = cache().join("fondo-mini.png");
    let imagen = if nombre.is_empty() && android::copiar_desde(&format!("{EXPORTADO}/fondo-mini.png"), &local) {
        crate::ajustes_android::leer_png(&local)
    } else {
        None
    };
    Actual { nombre, imagen }
}

fn imagen(d: &Diseno, ancho: usize, alto: usize) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([ancho, alto], &pintar(d, ancho, alto))
}

/// Dibuja `d` de `ancho` × `alto` (RGBA). Lleva un poco de ruido para que los degradados no se vean
/// en escalones.
fn pintar(d: &Diseno, ancho: usize, alto: usize) -> Vec<u8> {
    let canal = |color: u32, i: usize| ((color >> (16 - 8 * i)) & 0xFF) as f32;
    let lado = ancho.max(alto) as f32;
    let mut pixeles = Vec::with_capacity(ancho * alto * 4);
    for y in 0..alto {
        let t = y as f32 / (alto - 1).max(1) as f32;
        for x in 0..ancho {
            let mut c: [f32; 3] = std::array::from_fn(|i| canal(d.arriba, i) + (canal(d.abajo, i) - canal(d.arriba, i)) * t);
            for &(bx, by, radio, color, fuerza) in d.brillos {
                let dx = (x as f32 - bx * ancho as f32) / (radio * lado);
                let dy = (y as f32 - by * alto as f32) / (radio * lado);
                let peso = fuerza * (-(dx * dx + dy * dy) * 2.5).exp();
                for (i, v) in c.iter_mut().enumerate() {
                    *v += (canal(color, i) - *v) * peso;
                }
            }
            let ruido = ruido(x, y);
            pixeles.extend(c.iter().map(|v| (v + ruido).round().clamp(0.0, 255.0) as u8));
            pixeles.push(255);
        }
    }
    pixeles
}

/// De -0.75 a 0.75, fijo para cada píxel.
fn ruido(x: usize, y: usize) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h >> 24) as f32 / 255.0 - 0.5) * 1.5
}

fn guardar_png(ruta: &Path, pixeles: &[u8], ancho: usize, alto: usize) -> bool {
    let Ok(archivo) = std::fs::File::create(ruta) else { return false };
    let mut codificador = png::Encoder::new(BufWriter::new(archivo), ancho as u32, alto as u32);
    codificador.set_compression(png::Compression::Fast);
    codificador.set_color(png::ColorType::Rgba);
    codificador.set_depth(png::BitDepth::Eight);
    codificador.write_header().and_then(|mut w| w.write_image_data(pixeles)).is_ok()
}

fn cache() -> PathBuf {
    let carpeta = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"))
        .join("zykro");
    let _ = std::fs::create_dir_all(&carpeta);
    carpeta
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn png_rapido_conserva_pixeles_y_resolucion() {
        let ruta = std::env::temp_dir().join(format!("zykro-fondo-{}.png", std::process::id()));
        let pixeles = pintar(&DISENOS[2], 256, 144);
        assert!(guardar_png(&ruta, &pixeles, 256, 144));
        let mut lector = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&ruta).unwrap())).read_info().unwrap();
        let mut salida = vec![0; lector.output_buffer_size().unwrap()];
        let info = lector.next_frame(&mut salida).unwrap();
        assert_eq!((info.width, info.height), (256, 144));
        assert_eq!(&salida[..info.buffer_size()], pixeles.as_slice());
        std::fs::remove_file(ruta).unwrap();
    }

    #[test]
    #[ignore]
    fn medir_compresion_de_fondos() {
        let pixeles = pintar(&DISENOS[2], 1920, 1080);
        for _ in 0..3 {
            for compresion in [png::Compression::Balanced, png::Compression::Fast] {
                let inicio = std::time::Instant::now();
                let mut salida = Vec::new();
                let mut codificador = png::Encoder::new(&mut salida, 1920, 1080);
                codificador.set_color(png::ColorType::Rgba);
                codificador.set_depth(png::BitDepth::Eight);
                codificador.set_compression(compresion);
                codificador.write_header().unwrap().write_image_data(&pixeles).unwrap();
                eprintln!("[fondos] {compresion:?}: {:?}, {} bytes", inicio.elapsed(), salida.len());
            }
        }
    }
}
