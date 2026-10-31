//! Ajustes de Android al estilo de la ventana de Configuración de ZYKRO: lo que más se usa en un
//! emulador, compacto (la app de Configuración de Android se ve enorme en una ventana de laptop).
//! Se abre con el engrane de la pantalla de inicio de ZYKRO. ZYKRO cambia los ajustes desde la shell
//! de Android, como root; lo que necesita la interfaz de Android (cuentas, permisos) se abre ahí.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Align, ColorImage, Layout, RichText, Stroke, TextureHandle, TextureOptions, Vec2};
use egui_phosphor::regular as ic;

use crate::ajustes::{fila, linea, nota, pestana, titulo, ALTO_CONTENIDO, ANCHO_CONTENIDO, ANCHO_PESTANAS};
use crate::android::{self, EstadoCompartido};
use crate::{ACENTO, CERRAR_HOVER, FONDO, FONDO_CONTROL, FONDO_HOVER, TENUE, TEXTO};

/// Carpeta de Android donde la pantalla de inicio deja la lista de apps y sus íconos.
const EXPORTADO: &str = "/data/media/0/Android/data/com.zykro.inicio/files";

const IDIOMAS: [(&str, &str); 5] = [
    ("es-419", "Español (Latinoamérica)"),
    ("es-US", "Español (Estados Unidos)"),
    ("es-ES", "Español (España)"),
    ("en-US", "English (United States)"),
    ("pt-BR", "Português (Brasil)"),
];
const LETRAS: [(f32, &str); 3] = [(0.85, "Chica"), (1.0, "Normal"), (1.15, "Grande")];
const ANIMACIONES: [(f32, &str); 3] = [(1.0, "Normales"), (0.5, "Rápidas"), (0.0, "Sin animaciones")];
/// Pantallas de la Configuración de Android para lo que ZYKRO no cubre.
const PANTALLAS: [(&str, &str, &str); 6] = [
    (ic::USER_CIRCLE, "Cuentas (Google)", "android.settings.SYNC_SETTINGS"),
    (ic::KEYBOARD, "Idiomas y teclado", "android.settings.LOCALE_SETTINGS"),
    (ic::CLOCK, "Fecha y hora", "android.settings.DATE_SETTINGS"),
    (ic::SHIELD_CHECK, "Seguridad y privacidad", "android.settings.SECURITY_SETTINGS"),
    (ic::CODE, "Opciones para desarrolladores", "android.settings.APPLICATION_DEVELOPMENT_SETTINGS"),
    (ic::GEAR_SIX, "Toda la Configuración de Android", "android.settings.SETTINGS"),
];

#[derive(Clone, Copy, PartialEq)]
enum Pestana {
    Apps,
    General,
    Animaciones,
    Almacenamiento,
    Mas,
}

const PESTANAS: [(Pestana, &str, &str); 5] = [
    (Pestana::Apps, ic::SQUARES_FOUR, "Apps"),
    (Pestana::General, ic::GLOBE, "Idioma y letra"),
    (Pestana::Animaciones, ic::SPARKLE, "Animaciones"),
    (Pestana::Almacenamiento, ic::HARD_DRIVES, "Almacenamiento"),
    (Pestana::Mas, ic::DOTS_THREE_OUTLINE, "Más ajustes"),
];

#[derive(Clone)]
struct App {
    paquete: String,
    nombre: String,
    sistema: bool,
    /// App, datos, archivos y obb, en KB (solo de las apps propias).
    tamano_kb: Option<u64>,
}

/// Lo que se leyó de Android.
#[derive(Clone, Default)]
struct Datos {
    apps: Vec<App>,
    idioma: String,
    hora24: bool,
    letra: f32,
    animacion: f32,
    toques: bool,
    /// Lo que ocupa Android y el disco de la laptop donde guarda sus datos, en KB.
    android_kb: u64,
    disco_total_kb: u64,
    disco_libre_kb: u64,
}

#[derive(Clone, Copy)]
enum Peligro {
    BorrarDatos,
    Desinstalar,
}

pub struct AjustesAndroid {
    pub abierto: bool,
    pestana: Pestana,
    /// `None` mientras se lee de Android.
    datos: Arc<Mutex<Option<Datos>>>,
    /// Íconos leídos que falta subir a la GPU.
    iconos_nuevos: Arc<Mutex<Vec<(String, ColorImage)>>>,
    texturas: HashMap<String, TextureHandle>,
    mostrar_sistema: bool,
    /// Idioma elegido que falta aplicar (se aplica reiniciando Android).
    idioma: Option<String>,
    confirmar: Option<(Peligro, App)>,
    /// Hay un cambio aplicándose.
    ocupado: Arc<AtomicBool>,
}

impl AjustesAndroid {
    pub fn new() -> Self {
        AjustesAndroid {
            abierto: false,
            pestana: Pestana::Apps,
            datos: Arc::default(),
            iconos_nuevos: Arc::default(),
            texturas: HashMap::new(),
            mostrar_sistema: false,
            idioma: None,
            confirmar: None,
            ocupado: Arc::default(),
        }
    }

    pub fn abrir(&mut self, ctx: &egui::Context) {
        self.abierto = true;
        self.idioma = None;
        self.confirmar = None;
        *self.datos.lock().unwrap() = None;
        let (datos, iconos, ctx) = (self.datos.clone(), self.iconos_nuevos.clone(), ctx.clone());
        thread::spawn(move || {
            let (leidos, nuevos) = leer(true);
            *datos.lock().unwrap() = Some(leidos);
            *iconos.lock().unwrap() = nuevos;
            ctx.request_repaint();
        });
    }

    /// Corre `orden` en Android, avisa con `aviso` y vuelve a leer los ajustes.
    fn aplicar(&self, ctx: &egui::Context, estado: &EstadoCompartido, orden: String, aviso: String) {
        let (datos, ocupado, estado, ctx) = (self.datos.clone(), self.ocupado.clone(), estado.clone(), ctx.clone());
        ocupado.store(true, Ordering::Relaxed);
        thread::spawn(move || {
            let bien = android::shell(&orden).is_some();
            android::avisar(&estado, if bien { aviso } else { "No se pudo aplicar el cambio".into() });
            let (leidos, _) = leer(false);
            *datos.lock().unwrap() = Some(leidos);
            ocupado.store(false, Ordering::Relaxed);
            ctx.request_repaint();
        });
    }

    /// Abre una pantalla de la Configuración de Android (o una app) y cierra el panel.
    fn abrir_en_android(&mut self, orden: String) {
        thread::spawn(move || {
            android::shell(&orden);
        });
        self.abierto = false;
    }

    pub fn mostrar(&mut self, ctx: &egui::Context, estado: &EstadoCompartido) {
        if !self.abierto {
            return;
        }
        for (paquete, imagen) in self.iconos_nuevos.lock().unwrap().drain(..) {
            let textura = ctx.load_texture(format!("icono-{paquete}"), imagen, TextureOptions::LINEAR);
            self.texturas.insert(paquete, textura);
        }
        let datos = self.datos.lock().unwrap().clone();
        let ocupado = self.ocupado.load(Ordering::Relaxed);
        let mut cerrar = false;
        let marco = egui::Frame::new().fill(FONDO).corner_radius(8.0).stroke(Stroke::new(1.0, FONDO_HOVER));
        let alto = (ctx.content_rect().height() - 110.0).clamp(240.0, ALTO_CONTENIDO);
        let respuesta = egui::Modal::new(egui::Id::new("ajustes-android")).frame(marco).show(ctx, |ui| {
            ui.set_width(ANCHO_PESTANAS + ANCHO_CONTENIDO);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            ui.horizontal(|ui| {
                ui.set_height(40.0);
                ui.add_space(16.0);
                ui.label(RichText::new(ic::ANDROID_LOGO).size(16.0).color(ACENTO));
                ui.label(RichText::new("Ajustes de Android").size(15.0).strong().color(TEXTO));
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
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.vertical(|ui| {
                    ui.set_width(ANCHO_PESTANAS);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (p, glifo, nombre) in PESTANAS {
                        if pestana(ui, self.pestana == p, glifo, nombre).clicked() {
                            self.pestana = p;
                        }
                    }
                });
                ui.vertical(|ui| {
                    ui.set_width(ANCHO_CONTENIDO);
                    ui.set_height(alto);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(alto).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        ui.add_space(6.0);
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 4)).show(ui, |ui| {
                            let Some(d) = &datos else {
                                ui.horizontal(|ui| {
                                    ui.add(egui::Spinner::new().size(14.0).color(TENUE));
                                    nota(ui, "Leyendo los ajustes de Android…");
                                });
                                return;
                            };
                            match self.pestana {
                                Pestana::Apps => self.apps(ui, ctx, estado, d),
                                Pestana::General => self.general(ui, ctx, estado, d),
                                Pestana::Animaciones => self.animaciones(ui, ctx, estado, d),
                                Pestana::Almacenamiento => self.almacenamiento(ui, ctx, estado, d),
                                Pestana::Mas => self.mas(ui),
                            }
                        });
                    });
                });
            });
        });
        if respuesta.should_close() || cerrar {
            self.abierto = false;
        }
        self.confirmacion(ctx, estado);
    }

    fn apps(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, estado: &EstadoCompartido, d: &Datos) {
        ui.horizontal(|ui| {
            titulo(ui, "Apps");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.checkbox(&mut self.mostrar_sistema, "Mostrar las del sistema");
            });
        });
        let mostrar_sistema = self.mostrar_sistema;
        for app in d.apps.iter().filter(|a| mostrar_sistema || !a.sistema) {
            ui.horizontal(|ui| {
                ui.set_height(38.0);
                let lado = Vec2::splat(32.0);
                match self.texturas.get(&app.paquete) {
                    Some(t) => {
                        ui.add(egui::Image::new(t).fit_to_exact_size(lado).corner_radius(7.0));
                    }
                    None => {
                        let (rect, _) = ui.allocate_exact_size(lado, egui::Sense::hover());
                        ui.painter().rect_filled(rect, 7.0, FONDO_CONTROL);
                    }
                }
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(&app.nombre).size(13.5).color(TEXTO));
                    let detalle = match (app.sistema, app.tamano_kb) {
                        (true, _) => "App del sistema".to_string(),
                        (false, Some(kb)) => tamano(kb),
                        (false, None) => String::new(),
                    };
                    ui.label(RichText::new(detalle).size(11.0).color(TENUE));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let t = Vec2::new(30.0, 28.0);
                    let p = &app.paquete;
                    if !app.sistema {
                        if crate::icono(ui, ic::TRASH, "Desinstalar", t, CERRAR_HOVER).clicked() {
                            self.confirmar = Some((Peligro::Desinstalar, app.clone()));
                        }
                        if crate::icono(ui, ic::ERASER, "Borrar datos", t, CERRAR_HOVER).clicked() {
                            self.confirmar = Some((Peligro::BorrarDatos, app.clone()));
                        }
                        if crate::icono(ui, ic::BROOM, "Borrar caché", t, FONDO_HOVER).clicked() {
                            let orden = format!("rm -rf /data/data/{p}/cache/* /data/data/{p}/code_cache/* /data/media/0/Android/data/{p}/cache/*");
                            self.aplicar(ctx, estado, orden, format!("Caché de {} borrada", app.nombre));
                        }
                    }
                    if crate::icono(ui, ic::STOP, "Detener", t, FONDO_HOVER).clicked() {
                        self.aplicar(ctx, estado, format!("am force-stop {p}"), format!("{} detenida", app.nombre));
                    }
                    if crate::icono(ui, ic::PLAY, "Abrir", t, FONDO_HOVER).clicked() {
                        let paquete = p.clone();
                        thread::spawn(move || android::abrir_app(&paquete));
                        self.abierto = false;
                    }
                });
            });
        }
        nota(ui, "Los permisos de cada app se cambian en la Configuración de Android (en «Más ajustes»).");
    }

    fn general(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, estado: &EstadoCompartido, d: &Datos) {
        titulo(ui, "Idioma");
        let elegido = self.idioma.clone().unwrap_or_else(|| d.idioma.clone());
        fila(ui, "Idioma de Android", |ui| {
            let actual = IDIOMAS.iter().find(|(c, _)| *c == elegido).map(|(_, n)| n.to_string()).unwrap_or(elegido.clone());
            egui::ComboBox::from_id_salt("idioma-android").selected_text(actual).width(240.0).show_ui(ui, |ui| {
                for (codigo, nombre) in IDIOMAS {
                    if ui.selectable_label(elegido == codigo, nombre).clicked() {
                        self.idioma = Some(codigo.to_string());
                    }
                }
            });
        });
        if let Some(nuevo) = self.idioma.clone().filter(|i| *i != d.idioma) {
            ui.horizontal(|ui| {
                nota(ui, "Para cambiar el idioma se reinicia Android (unos 10 s).");
                if ui.button("Aplicar").clicked() {
                    // Android lo lee al arrancar; al apagarse, ZYKRO lo vuelve a encender solo.
                    let orden = format!("setprop persist.sys.locale {nuevo} && settings put system system_locales {nuevo} && reboot");
                    self.aplicar(ctx, estado, orden, "Reiniciando Android con el idioma nuevo…".into());
                    self.idioma = None;
                }
            });
        }

        ui.add_space(4.0);
        titulo(ui, "Hora");
        let mut hora24 = d.hora24;
        if ui.checkbox(&mut hora24, "Formato de 24 horas").changed() {
            let valor = if hora24 { "24" } else { "12" };
            self.aplicar(ctx, estado, format!("settings put system time_12_24 {valor}"), "Formato de hora cambiado".into());
        }
        nota(ui, "La zona horaria se toma de la laptop cada vez que abres ZYKRO.");

        ui.add_space(4.0);
        titulo(ui, "Letra");
        fila(ui, "Tamaño de la letra", |ui| {
            for (escala, nombre) in LETRAS {
                if ui.selectable_label((d.letra - escala).abs() < 0.01, nombre).clicked() {
                    self.aplicar(ctx, estado, format!("settings put system font_scale {escala}"), format!("Letra {}", nombre.to_lowercase()));
                }
            }
        });
        nota(ui, "Cambia la letra de todas las apps de Android; con «Chica», la Configuración de Android se ve más compacta.");
    }

    fn animaciones(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, estado: &EstadoCompartido, d: &Datos) {
        titulo(ui, "Animaciones");
        fila(ui, "Animaciones", |ui| {
            for (escala, nombre) in ANIMACIONES {
                if ui.selectable_label((d.animacion - escala).abs() < 0.01, nombre).clicked() {
                    let orden = format!(
                        "settings put global window_animation_scale {escala} && settings put global transition_animation_scale {escala} \
                         && settings put global animator_duration_scale {escala}"
                    );
                    self.aplicar(ctx, estado, orden, format!("Animaciones: {}", nombre.to_lowercase()));
                }
            }
        });
        nota(ui, "«Rápidas» hace que abrir y cambiar de app se sienta más ágil; sin animaciones, todavía más.");

        ui.add_space(4.0);
        titulo(ui, "Toques");
        let mut toques = d.toques;
        if ui.checkbox(&mut toques, "Mostrar dónde se toca la pantalla").changed() {
            let valor = toques as u8;
            self.aplicar(ctx, estado, format!("settings put system show_touches {valor}"), "Listo".into());
        }
        nota(ui, "Dibuja un punto en cada toque: sirve para revisar dónde caen las teclas del mapeo.");
    }

    fn almacenamiento(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, estado: &EstadoCompartido, d: &Datos) {
        titulo(ui, "Almacenamiento");
        fila(ui, "Android ocupa", |ui| {
            ui.label(RichText::new(tamano(d.android_kb)).color(TEXTO));
        });
        if d.disco_total_kb > 0 {
            fila(ui, "Libre en la laptop", |ui| {
                let texto = format!("{} de {}", tamano(d.disco_libre_kb), tamano(d.disco_total_kb));
                ui.label(RichText::new(texto).color(TEXTO));
            });
            let usado = 1.0 - d.disco_libre_kb as f32 / d.disco_total_kb as f32;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3.0, FONDO_CONTROL);
            let lleno = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * usado, rect.height()));
            ui.painter().rect_filled(lleno, 3.0, ACENTO);
        }
        nota(ui, "Android guarda sus apps y datos en el disco de la laptop: el espacio libre es el mismo.");
        ui.add_space(4.0);
        if ui.button(format!("{}  Liberar espacio", ic::BROOM)).clicked() {
            self.aplicar(ctx, estado, "pm trim-caches 9999G".into(), "Caché de las apps borrada".into());
        }
        nota(ui, "Borra la caché de todas las apps; tus datos, partidas y sesiones no se tocan.");
    }

    fn mas(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Más ajustes de Android");
        nota(ui, "Se abren en la Configuración de Android.");
        for (glifo, nombre, accion) in PANTALLAS {
            let boton = egui::Button::new(RichText::new(format!("{glifo}   {nombre}")).size(13.5).color(TEXTO))
                .min_size(Vec2::new(ui.available_width(), 34.0))
                .fill(FONDO_CONTROL);
            if ui.add(boton).clicked() {
                self.abrir_en_android(format!("am start -a {accion}"));
            }
        }
    }

    /// Pregunta antes de borrar los datos de una app o desinstalarla.
    fn confirmacion(&mut self, ctx: &egui::Context, estado: &EstadoCompartido) {
        let Some((peligro, app)) = self.confirmar.clone() else { return };
        let (pregunta, detalle, boton, orden, aviso) = match peligro {
            Peligro::BorrarDatos => (
                format!("¿Borrar los datos de {}?", app.nombre),
                "Se pierde todo lo que guardó la app: la sesión, los ajustes y lo que descargó. Queda como recién instalada.",
                "Borrar datos",
                format!("pm clear {}", app.paquete),
                format!("Datos de {} borrados", app.nombre),
            ),
            Peligro::Desinstalar => (
                format!("¿Desinstalar {}?", app.nombre),
                "Se quitan la app y todos sus datos.",
                "Desinstalar",
                format!("pm uninstall {}", app.paquete),
                format!("{} desinstalada", app.nombre),
            ),
        };
        let marco = egui::Frame::new().fill(FONDO).corner_radius(8.0).stroke(Stroke::new(1.0, FONDO_HOVER)).inner_margin(18.0);
        let mut decidido = None;
        egui::Modal::new(egui::Id::new("confirmar-android")).frame(marco).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new(pregunta).size(15.0).strong().color(TEXTO));
            ui.add_space(4.0);
            ui.label(RichText::new(detalle).size(12.5).color(TENUE));
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let peligroso = egui::Button::new(RichText::new(boton).color(egui::Color32::WHITE).strong()).fill(CERRAR_HOVER);
                if ui.add(peligroso).clicked() {
                    decidido = Some(true);
                }
                if ui.button("Cancelar").clicked() {
                    decidido = Some(false);
                }
            });
        });
        if let Some(si) = decidido {
            self.confirmar = None;
            if si {
                self.aplicar(ctx, estado, orden, aviso);
            }
        }
    }
}

/// Lee los ajustes y las apps de Android. Con `iconos`, también trae los íconos que dejó la pantalla
/// de inicio de ZYKRO.
fn leer(iconos: bool) -> (Datos, Vec<(String, ColorImage)>) {
    let mut d = Datos { letra: 1.0, animacion: 1.0, ..Default::default() };
    let ajustes = android::shell(
        "getprop persist.sys.locale; getprop ro.product.locale; settings get system time_12_24; settings get system font_scale; \
         settings get global animator_duration_scale; settings get system show_touches; df -k /data | tail -1; du -sxk /data | cut -f1",
    )
    .unwrap_or_default();
    let campos: Vec<&str> = ajustes.lines().map(str::trim).collect();
    let campo = |i: usize| campos.get(i).copied().unwrap_or("");
    d.idioma = if campo(0).is_empty() { campo(1) } else { campo(0) }.to_string();
    d.hora24 = campo(2) == "24";
    d.letra = campo(3).parse().unwrap_or(1.0);
    d.animacion = campo(4).parse().unwrap_or(1.0);
    d.toques = campo(5) == "1";
    let disco: Vec<&str> = campo(6).split_whitespace().collect();
    d.disco_total_kb = disco.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    d.disco_libre_kb = disco.get(3).and_then(|v| v.parse().ok()).unwrap_or(0);
    d.android_kb = campo(7).parse().unwrap_or(0);

    // Las apps con sus nombres, de la lista que deja la pantalla de inicio; si no está, los paquetes.
    let lista = android::shell(&format!("cat {EXPORTADO}/apps.json")).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    d.apps = match lista.as_ref().and_then(|v| v.as_array()) {
        Some(apps) => apps
            .iter()
            .filter_map(|a| {
                Some(App {
                    paquete: a["paquete"].as_str()?.to_string(),
                    nombre: a["nombre"].as_str()?.to_string(),
                    sistema: a["sistema"].as_bool().unwrap_or(false),
                    tamano_kb: None,
                })
            })
            .collect(),
        None => android::shell("pm list packages -3")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.strip_prefix("package:"))
            .map(|p| App { paquete: p.to_string(), nombre: crate::recientes::nombre_app(p), sistema: false, tamano_kb: None })
            .collect(),
    };
    // Solo paquetes con nombres normales: van dentro de órdenes de la shell.
    d.apps.retain(|a| a.paquete.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_'));

    // Lo que ocupa cada app propia: la app, sus datos y sus archivos (obb incluidos).
    let tamanos = android::shell(
        "for p in $(pm list packages -3 | cut -d: -f2); do a=$(pm path $p | head -1 | cut -d: -f2); \
         echo \"$p $(du -sk ${a%/*} /data/data/$p /data/media/0/Android/data/$p /data/media/0/Android/obb/$p 2>/dev/null \
         | awk '{s+=$1} END {print s+0}')\"; done",
    )
    .unwrap_or_default();
    let tamanos: HashMap<&str, u64> =
        tamanos.lines().filter_map(|l| l.split_once(' ')).filter_map(|(p, kb)| Some((p, kb.trim().parse().ok()?))).collect();
    for app in &mut d.apps {
        app.tamano_kb = tamanos.get(app.paquete.as_str()).copied();
    }

    let mut nuevos = Vec::new();
    if iconos {
        let carpeta = carpeta_iconos();
        let _ = std::fs::remove_dir_all(&carpeta);
        if let Some(padre) = carpeta.parent() {
            let _ = std::fs::create_dir_all(padre);
        }
        if android::copiar_desde(&format!("{EXPORTADO}/iconos"), &carpeta) {
            for app in &d.apps {
                if let Some(imagen) = leer_png(&carpeta.join(format!("{}.png", app.paquete))) {
                    nuevos.push((app.paquete.clone(), imagen));
                }
            }
        }
    }
    (d, nuevos)
}

fn carpeta_iconos() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"))
        .join("zykro/iconos")
}

pub(crate) fn leer_png(ruta: &Path) -> Option<ColorImage> {
    let archivo = std::io::BufReader::new(std::fs::File::open(ruta).ok()?);
    let mut decodificador = png::Decoder::new(archivo);
    decodificador.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut lector = decodificador.read_info().ok()?;
    let mut pixeles = vec![0; lector.output_buffer_size()?];
    let info = lector.next_frame(&mut pixeles).ok()?;
    let (ancho, alto) = (info.width as usize, info.height as usize);
    let pixeles = &pixeles[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => Some(ColorImage::from_rgba_unmultiplied([ancho, alto], pixeles)),
        png::ColorType::Rgb => Some(ColorImage::from_rgb([ancho, alto], pixeles)),
        _ => None,
    }
}

/// «35 MB», «1.2 GB».
fn tamano(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}
