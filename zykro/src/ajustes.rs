//! Ventana de configuración con las secciones de la de BlueStacks 5: pestañas a la izquierda y botones
//! abajo. Lo que no tiene cómo funcionar en ZYKRO no está (cámara y micrófono: este Android no los
//! tiene; memoria: Android comparte la de la laptop; anuncios, Discord o la tienda al iniciar: no
//! aplican), y lo que ya está en otro lado no se repite: idioma, letra, animaciones, mostrar dónde se
//! toca y liberar espacio son ajustes de Android, en su panel (botón «Ajustes» del panel de
//! notificaciones).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Align, Align2, ColorImage, Event, FontId, Key, Layout, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use egui_phosphor::regular as ic;

use crate::atajos::{self, ATAJOS};
use crate::config::Config;
use crate::rendimiento::{self, Fabricante, Modo};
use crate::telefono::{OPERADORAS, PERFILES};
use crate::{android, datos, ACENTO, FONDO, FONDO_CONTROL, FONDO_HOVER, ICONO, NOMBRE, TENUE, TEXTO, VERSION};

#[derive(Clone, Copy, PartialEq)]
enum Pestana {
    Rendimiento,
    Pantalla,
    Graficos,
    Juegos,
    Dispositivos,
    Gamepad,
    Preferencias,
    Telefono,
    Atajos,
    Datos,
    Avanzado,
    Acerca,
}

const PESTANAS: [(Pestana, &str, &str); 12] = [
    (Pestana::Rendimiento, ic::LIGHTNING, "Rendimiento"),
    (Pestana::Pantalla, ic::MONITOR, "Pantalla"),
    (Pestana::Graficos, ic::GRAPHICS_CARD, "Gráficos"),
    (Pestana::Juegos, ic::JOYSTICK, "Juegos"),
    (Pestana::Dispositivos, ic::SPEAKER_HIGH, "Dispositivos"),
    (Pestana::Gamepad, ic::GAME_CONTROLLER, "Gamepad"),
    (Pestana::Preferencias, ic::SLIDERS_HORIZONTAL, "Preferencias"),
    (Pestana::Telefono, ic::DEVICE_MOBILE, "Teléfono"),
    (Pestana::Atajos, ic::KEYBOARD, "Atajos"),
    (Pestana::Datos, ic::DATABASE, "Datos del usuario"),
    (Pestana::Avanzado, ic::WRENCH, "Avanzado"),
    (Pestana::Acerca, ic::INFO, "Acerca de"),
];

#[derive(Clone, Copy, PartialEq)]
enum Orientacion {
    Horizontal,
    Vertical,
    Ultra,
    Personalizada,
}

const ORIENTACIONES: [(Orientacion, &str); 4] = [
    (Orientacion::Horizontal, "Horizontal"),
    (Orientacion::Vertical, "Vertical"),
    (Orientacion::Ultra, "Ultrapanorámica"),
    (Orientacion::Personalizada, "Personalizada"),
];

const RESOLUCIONES: [(Orientacion, u32, u32, &str); 10] = [
    (Orientacion::Horizontal, 1280, 720, "1280 × 720 (HD)"),
    (Orientacion::Horizontal, 1600, 900, "1600 × 900"),
    (Orientacion::Horizontal, 1920, 1080, "1920 × 1080 (Full HD)"),
    (Orientacion::Horizontal, 2560, 1440, "2560 × 1440 (2K)"),
    (Orientacion::Horizontal, 3840, 2160, "3840 × 2160 (4K)"),
    (Orientacion::Vertical, 720, 1280, "720 × 1280"),
    (Orientacion::Vertical, 1080, 1920, "1080 × 1920"),
    (Orientacion::Vertical, 1440, 2560, "1440 × 2560"),
    (Orientacion::Ultra, 2560, 1080, "2560 × 1080 (21:9)"),
    (Orientacion::Ultra, 3440, 1440, "3440 × 1440 (21:9)"),
];
/// 360 en 1920×1080 deja a Android del mismo tamaño que BlueStacks (1280×720 a 240), pero más nítido.
const DPIS: [(u32, &str); 5] =
    [(160, "160 (bajo)"), (240, "240 (medio)"), (320, "320 (alto)"), (360, "360 (como BlueStacks en Full HD)"), (480, "480 (muy alto)")];
const FPS: [u32; 6] = [30, 60, 90, 120, 144, 240];
const ESCALAS: [f32; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];
const REPOSITORIO: &str = "https://github.com/isacmn45/ZYKRO";

/// Tamaño fijo de la ventana de configuración (el contenido se desplaza si no cabe).
pub(crate) const ANCHO_PESTANAS: f32 = 170.0;
pub(crate) const ANCHO_CONTENIDO: f32 = 590.0;
pub(crate) const ALTO_CONTENIDO: f32 = 470.0;

/// Lo que se lee de Android al abrir la ventana.
#[derive(Default)]
struct Leido {
    /// Apps propias (paquete y nombre), para Juegos.
    apps: Vec<(String, String)>,
    iconos: Vec<(String, ColorImage)>,
    abi: String,
}

/// Lo que se está haciendo en Datos del usuario o en Acerca de (en otro hilo).
#[derive(Default)]
struct Trabajo {
    ocupado: bool,
    /// Lo último que pasó (se muestra debajo de los botones).
    aviso: String,
    /// Un respaldo en curso: bytes escritos y total estimado.
    respaldo: Option<(Arc<AtomicU64>, u64)>,
}

pub struct Ajustes {
    pub abierto: bool,
    pestana: Pestana,
    borrador: Config,
    original: Config,
    /// Se está pidiendo la contraseña para el permiso de la GPU.
    pidiendo_permiso: Arc<AtomicBool>,
    orientacion: Orientacion,
    dpi_propio: bool,
    /// Atajo que espera su tecla nueva (su id).
    capturando: Option<&'static str>,
    aviso_atajo: Option<String>,
    leido: Arc<Mutex<Option<Leido>>>,
    apps: Vec<(String, String)>,
    abi: String,
    texturas: HashMap<String, TextureHandle>,
    salidas: Vec<(String, String)>,
    gpus: Vec<(String, String)>,
    trabajo: Arc<Mutex<Trabajo>>,
    /// La búsqueda de actualizaciones (Acerca de), aparte de lo de Datos del usuario.
    actualizacion: Arc<Mutex<Trabajo>>,
    /// Respaldo elegido para restaurar, esperando que se confirme.
    confirmar_restaurar: Option<PathBuf>,
    /// Respaldo a restaurar: la ventana principal se reabre con él.
    pub restaurar: Option<PathBuf>,
}

impl Ajustes {
    pub fn new() -> Self {
        Ajustes {
            abierto: false,
            pestana: Pestana::Rendimiento,
            borrador: Config::default(),
            original: Config::default(),
            pidiendo_permiso: Arc::default(),
            orientacion: Orientacion::Horizontal,
            dpi_propio: false,
            capturando: None,
            aviso_atajo: None,
            leido: Arc::default(),
            apps: Vec::new(),
            abi: String::new(),
            texturas: HashMap::new(),
            salidas: Vec::new(),
            gpus: Vec::new(),
            trabajo: Arc::default(),
            actualizacion: Arc::default(),
            confirmar_restaurar: None,
            restaurar: None,
        }
    }

    pub fn abrir(&mut self, actual: &Config) {
        self.borrador = actual.clone();
        self.original = actual.clone();
        self.abierto = true;
        self.capturando = None;
        self.aviso_atajo = None;
        self.confirmar_restaurar = None;
        let a = &actual.android;
        self.orientacion = RESOLUCIONES.iter().find(|r| r.1 == a.ancho && r.2 == a.alto).map_or(Orientacion::Personalizada, |r| r.0);
        self.dpi_propio = !DPIS.iter().any(|d| d.0 == a.dpi);
        self.salidas = crate::audio::salidas();
        self.gpus = rendimiento::gpus();
        let leido = self.leido.clone();
        thread::spawn(move || *leido.lock().unwrap() = Some(leer()));
    }

    /// Esperando la tecla de un atajo: la ventana principal no debe tomarla como atajo.
    pub fn capturando(&self) -> bool {
        self.abierto && self.capturando.is_some()
    }

    /// Dibuja la ventana si está abierta. Devuelve la configuración nueva al presionar Guardar.
    pub fn mostrar(&mut self, ctx: &egui::Context) -> Option<Config> {
        if !self.abierto {
            return None;
        }
        if let Some(leido) = self.leido.lock().unwrap().take() {
            for (paquete, imagen) in leido.iconos {
                self.texturas.insert(paquete.clone(), ctx.load_texture(format!("ajustes-{paquete}"), imagen, TextureOptions::LINEAR));
            }
            self.apps = leido.apps;
            self.abi = leido.abi;
        }
        // Antes que la ventana: Esc y las teclas son para el atajo, no para cerrarla.
        self.capturar_tecla(ctx);
        let mut guardar = None;
        let mut cerrar = false;
        let marco = egui::Frame::new().fill(FONDO).corner_radius(8.0).stroke(Stroke::new(1.0, FONDO_HOVER));
        // Todas las pestañas con el mismo tamaño; en una ventana baja se achica para que quepa.
        let alto = (ctx.content_rect().height() - 150.0).clamp(240.0, ALTO_CONTENIDO);
        let respuesta = egui::Modal::new(egui::Id::new("ajustes")).frame(marco).show(ctx, |ui| {
            ui.set_width(ANCHO_PESTANAS + ANCHO_CONTENIDO);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

            // Cabecera
            ui.horizontal(|ui| {
                ui.set_height(40.0);
                ui.add_space(16.0);
                ui.label(RichText::new(ic::GEAR).size(16.0).color(ACENTO));
                ui.label(RichText::new("Configuración").size(15.0).strong().color(TEXTO));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(6.0);
                    if crate::icono(ui, ic::X, "Cerrar", Vec2::new(34.0, 30.0), crate::CERRAR_HOVER).clicked() {
                        cerrar = true;
                    }
                });
            });
            linea(ui);

            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // Pestañas
                ui.vertical(|ui| {
                    ui.set_width(ANCHO_PESTANAS);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (p, glifo, nombre) in PESTANAS {
                        if pestana(ui, self.pestana == p, glifo, nombre).clicked() {
                            self.pestana = p;
                        }
                    }
                });
                // Contenido: siempre del mismo tamaño; si una pestaña no cabe, se desplaza.
                ui.vertical(|ui| {
                    ui.set_width(ANCHO_CONTENIDO);
                    ui.set_height(alto);
                    egui::ScrollArea::vertical().id_salt(self.pestana as u8).auto_shrink([false, false]).max_height(alto).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        ui.add_space(6.0);
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 4)).show(ui, |ui| match self.pestana {
                            Pestana::Rendimiento => self.rendimiento(ui),
                            Pestana::Pantalla => self.pantalla(ui),
                            Pestana::Graficos => self.graficos(ui),
                            Pestana::Juegos => self.juegos(ui),
                            Pestana::Dispositivos => self.dispositivos(ui),
                            Pestana::Gamepad => self.gamepad(ui),
                            Pestana::Preferencias => self.preferencias(ui),
                            Pestana::Telefono => self.telefono(ui),
                            Pestana::Atajos => self.atajos(ui),
                            Pestana::Datos => self.datos(ui, &mut cerrar),
                            Pestana::Avanzado => self.avanzado(ui),
                            Pestana::Acerca => self.acerca(ui),
                        });
                    });
                });
            });

            linea(ui);
            ui.horizontal(|ui| {
                ui.set_height(44.0);
                ui.add_space(16.0);
                if self.borrador.android != self.original.android {
                    ui.label(RichText::new(format!("{}  Al guardar se reiniciarán ZYKRO y Android", ic::ARROWS_CLOCKWISE)).size(12.5).color(ACENTO));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(16.0);
                    let boton = egui::Button::new(RichText::new("Guardar").color(egui::Color32::WHITE).strong())
                        .fill(ACENTO)
                        .min_size(Vec2::new(96.0, 30.0));
                    if ui.add(boton).clicked() {
                        guardar = Some(self.borrador.clone());
                        cerrar = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(Vec2::new(96.0, 30.0))).clicked() {
                        cerrar = true;
                    }
                });
            });
        });
        if respuesta.should_close() {
            cerrar = true;
        }
        if cerrar {
            self.abierto = false;
            self.capturando = None;
        }
        guardar
    }

    fn rendimiento(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Modo de rendimiento");
        for modo in Modo::TODOS {
            if opcion_modo(ui, modo, self.borrador.modo == modo).clicked() {
                self.borrador.modo = modo;
            }
        }
        nota(ui, &format!("{}  Cuanto más rendimiento, más se calienta el equipo y más energía gasta (en una laptop, la batería dura menos).", ic::THERMOMETER));
        self.permiso_gpu(ui);
        let total = rendimiento::nucleos_totales();
        fila(ui, "Núcleos de la CPU", |ui| {
            let actual = self.borrador.nucleos.map_or("Según el modo".to_string(), |n| format!("{n} de {total}"));
            egui::ComboBox::from_id_salt("nucleos").selected_text(actual).width(220.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.borrador.nucleos, None, "Según el modo");
                for n in 1..=total {
                    ui.selectable_value(&mut self.borrador.nucleos, Some(n), format!("{n} de {total}"));
                }
            });
        });
        nota(ui, "Cuántos núcleos puede usar Android. «Según el modo» usa la mitad en Ahorro y todos en los demás.");

        ui.add_space(4.0);
        titulo(ui, "Cuadros por segundo");
        let a = &mut self.borrador.android;
        fila(ui, "FPS máximos", |ui| {
            egui::ComboBox::from_id_salt("fps").selected_text(format!("{} FPS", a.fps)).width(220.0).show_ui(ui, |ui| {
                for fps in FPS {
                    ui.selectable_value(&mut a.fps, fps, format!("{fps} FPS"));
                }
            });
        });
        nota(ui, "Más de 60 sirve con un monitor de más hercios o en juegos que lo aprovechan; gasta más. Se aplica reiniciando Android.");
        ui.checkbox(&mut self.borrador.mostrar_fps, "Mostrar los FPS en la barra de título y en pantalla completa");
        ui.checkbox(&mut self.borrador.mostrar_temperatura, "Mostrar la temperatura de la CPU y la GPU");
    }

    /// Para Ultra y los otros modos que suben la velocidad mínima de la GPU: el permiso, una vez.
    fn permiso_gpu(&mut self, ui: &mut egui::Ui) {
        if rendimiento::gpu().is_none() || !rendimiento::gpu_ajustable(self.borrador.modo) || rendimiento::gpu_permitida() {
            return;
        }
        ui.horizontal(|ui| {
            nota(ui, "Para que la GPU no baje tanto de velocidad (si baja, los juegos dan tirones) hace falta tu permiso, una sola vez.");
            let pidiendo = self.pidiendo_permiso.load(Ordering::Relaxed);
            let texto = if pidiendo { "Esperando…" } else { "Dar permiso" };
            if ui.add_enabled(!pidiendo, egui::Button::new(texto)).clicked() {
                let (pidiendo, ctx) = (self.pidiendo_permiso.clone(), ui.ctx().clone());
                // El modo que ya está activo: si se guarda otro, al guardar se aplica el nuevo.
                let modo = self.original.modo;
                pidiendo.store(true, Ordering::Relaxed);
                thread::spawn(move || {
                    if rendimiento::pedir_permiso_gpu() {
                        rendimiento::aplicar_gpu(modo);
                    }
                    pidiendo.store(false, Ordering::Relaxed);
                    ctx.request_repaint();
                });
            }
        });
    }

    fn pantalla(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Pantalla de Android");
        let a = &mut self.borrador.android;
        fila(ui, "Orientación", |ui| {
            for (o, nombre) in ORIENTACIONES {
                if ui.selectable_label(self.orientacion == o, nombre).clicked() && self.orientacion != o {
                    self.orientacion = o;
                    if let Some(r) = RESOLUCIONES.iter().find(|r| r.0 == o) {
                        (a.ancho, a.alto) = (r.1, r.2);
                    }
                }
            }
        });
        fila(ui, "Resolución", |ui| {
            if self.orientacion == Orientacion::Personalizada {
                ui.add(egui::DragValue::new(&mut a.ancho).range(480..=3840).speed(8));
                ui.label("×");
                ui.add(egui::DragValue::new(&mut a.alto).range(480..=3840).speed(8));
                // Android necesita medidas pares.
                a.ancho -= a.ancho % 2;
                a.alto -= a.alto % 2;
                return;
            }
            let actual = RESOLUCIONES.iter().find(|r| r.1 == a.ancho && r.2 == a.alto).map_or(format!("{} × {}", a.ancho, a.alto), |r| r.3.to_string());
            egui::ComboBox::from_id_salt("resolucion").selected_text(actual).width(220.0).show_ui(ui, |ui| {
                for (o, w, h, nombre) in RESOLUCIONES {
                    if o == self.orientacion && ui.selectable_label(a.ancho == w && a.alto == h, nombre).clicked() {
                        (a.ancho, a.alto) = (w, h);
                    }
                }
            });
        });
        fila(ui, "DPI", |ui| {
            let actual = if self.dpi_propio { "Personalizado".to_string() } else { DPIS.iter().find(|d| d.0 == a.dpi).map_or(a.dpi.to_string(), |d| d.1.to_string()) };
            egui::ComboBox::from_id_salt("dpi").selected_text(actual).width(220.0).show_ui(ui, |ui| {
                for (dpi, nombre) in DPIS {
                    if ui.selectable_label(!self.dpi_propio && a.dpi == dpi, nombre).clicked() {
                        a.dpi = dpi;
                        self.dpi_propio = false;
                    }
                }
                if ui.selectable_label(self.dpi_propio, "Personalizado").clicked() {
                    self.dpi_propio = true;
                }
            });
            if self.dpi_propio {
                ui.add(egui::DragValue::new(&mut a.dpi).range(100..=640));
            }
        });
        nota(ui, "Más DPI agranda las letras y los botones de Android. Se aplica reiniciando ZYKRO y Android; tus apps y datos no se borran.");

        ui.add_space(4.0);
        titulo(ui, "Ventana de ZYKRO");
        fila(ui, "Tamaño de la interfaz", |ui| {
            let actual = format!("{:.0} %", self.borrador.escala * 100.0);
            egui::ComboBox::from_id_salt("escala").selected_text(actual).width(220.0).show_ui(ui, |ui| {
                for escala in ESCALAS {
                    ui.selectable_value(&mut self.borrador.escala, escala, format!("{:.0} %", escala * 100.0));
                }
            });
        });
        nota(ui, "Agranda los botones, las letras y las ventanas de ZYKRO (útil en un monitor 2K o 4K); la imagen de Android no cambia.");
        fila(ui, "Cursor sobre la imagen", |ui| {
            ui.selectable_value(&mut self.borrador.cursor_zykro, false, "Del sistema");
            ui.selectable_value(&mut self.borrador.cursor_zykro, true, "De ZYKRO");
        });
        nota(ui, "El de ZYKRO es más grande y morado, con borde blanco: se ve sobre cualquier juego.");
    }

    fn graficos(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Renderizador de Android");
        let a = &mut self.borrador.android;
        ui.radio_value(&mut a.vulkan, true, "Vulkan y OpenGL (recomendado)");
        ui.radio_value(&mut a.vulkan, false, "Solo OpenGL");
        nota(ui, "Cada juego elige con cuál dibujar. Con «Solo OpenGL», los que usarían Vulkan usan OpenGL: sirve si un juego se ve mal o se cierra. Se aplica reiniciando Android.");

        ui.add_space(4.0);
        titulo(ui, "GPU");
        if let Some(gpu) = rendimiento::gpu() {
            let tipo = if gpu.integrada { "integrada" } else { "dedicada" };
            ui.label(RichText::new(format!("{}  {} ({tipo})", ic::GRAPHICS_CARD, gpu.nombre)).size(12.5).color(ICONO));
            if gpu.fabricante == Fabricante::Nvidia {
                nota(ui, "Con el driver propietario de NVIDIA, ZYKRO todavía no puede mostrar la imagen de Android.");
            }
        }
        if self.gpus.len() > 1 {
            fila(ui, "Android usa", |ui| {
                let actual = a.gpu.as_ref().and_then(|n| self.gpus.iter().find(|g| &g.0 == n)).map_or("La primera que sirva".to_string(), |g| g.1.clone());
                egui::ComboBox::from_id_salt("gpu").selected_text(actual).width(300.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut a.gpu, None, "La primera que sirva");
                    for (nodo, nombre) in &self.gpus {
                        ui.selectable_value(&mut a.gpu, Some(nodo.clone()), nombre);
                    }
                });
            });
            nota(ui, "En una laptop con dos GPU, la dedicada da más FPS y la integrada gasta menos batería. Se aplica reiniciando Android.");
        }

        ui.add_space(4.0);
        titulo(ui, "Texturas");
        ui.checkbox(&mut a.lod_activado, "Suavizar texturas");
        ui.add_enabled_ui(a.lod_activado, |ui| {
            fila(ui, "Intensidad", |ui| {
                ui.add(egui::Slider::new(&mut a.lod, 0.0..=4.0).step_by(0.25).fixed_decimals(2));
            });
        });
        nota(ui, "Las texturas (sobre todo las lejanas) se ven un poco más suaves y la GPU trabaja menos: más FPS en juegos pesados. 0.5 es un buen punto medio.");
    }

    fn juegos(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Pantalla completa al abrir");
        nota(ui, "Al abrir estas apps, ZYKRO se pone solo en pantalla completa.");
        if self.apps.is_empty() {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0).color(TENUE));
                nota(ui, "Leyendo las apps de Android…");
            });
            return;
        }
        for (paquete, nombre) in &self.apps {
            ui.horizontal(|ui| {
                ui.set_height(34.0);
                let lado = Vec2::splat(28.0);
                match self.texturas.get(paquete) {
                    Some(t) => {
                        ui.add(egui::Image::new(t).fit_to_exact_size(lado).corner_radius(6.0));
                    }
                    None => {
                        let (rect, _) = ui.allocate_exact_size(lado, Sense::hover());
                        ui.painter().rect_filled(rect, 6.0, FONDO_CONTROL);
                    }
                }
                ui.add_space(6.0);
                let mut activa = self.borrador.pantalla_completa.contains(paquete);
                if ui.checkbox(&mut activa, nombre).changed() {
                    self.borrador.pantalla_completa.retain(|p| p != paquete);
                    if activa {
                        self.borrador.pantalla_completa.push(paquete.clone());
                    }
                }
            });
        }
    }

    fn dispositivos(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Sonido");
        fila(ui, "Salida", |ui| {
            let actual = self.borrador.salida_sonido.as_ref().map_or("La del sistema".to_string(), |s| {
                self.salidas.iter().find(|(n, _)| n == s).map_or(s.clone(), |(_, d)| d.clone())
            });
            egui::ComboBox::from_id_salt("salida").selected_text(actual).width(320.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.borrador.salida_sonido, None, "La del sistema");
                for (nombre, descripcion) in &self.salidas {
                    ui.selectable_value(&mut self.borrador.salida_sonido, Some(nombre.clone()), descripcion);
                }
            });
        });
        nota(ui, "Por dónde suena Android: «La del sistema» sigue a la que elijas en la laptop (por ejemplo, unos audífonos al conectarlos).");
    }

    fn gamepad(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Gamepad");
        ui.checkbox(&mut self.borrador.gamepad, "Usar los mandos de la laptop en Android (USB o Bluetooth)");
        ui.add_enabled_ui(self.borrador.gamepad, |ui| {
            ui.checkbox(&mut self.borrador.vibracion, "Vibración");
        });
        nota(ui, "Los juegos que aceptan mando lo usan como en un teléfono, con los botones en su lugar según el modelo del mando. La vibración se aplica al conectar el mando.");
        ui.add_space(4.0);
        titulo(ui, "Conectados");
        let conectados = crate::gamepad::CONECTADOS.lock().unwrap().clone();
        if conectados.is_empty() {
            nota(ui, "Ningún mando. Conéctalo y aparecerá aquí en unos segundos.");
        }
        for nombre in conectados {
            ui.label(RichText::new(format!("{}  {nombre}", ic::GAME_CONTROLLER)).color(TEXTO));
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
    }

    fn preferencias(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Preferencias");
        let c = &mut self.borrador;
        ui.checkbox(&mut c.ocultar_teclado, "Ocultar el teclado en pantalla de Android");
        nota(ui, "Con el teclado en pantalla oculto, escribe con el teclado de la laptop.");
        ui.checkbox(&mut c.sonido_toques, "Sonido al tocar la pantalla de Android");
        ui.checkbox(&mut c.esc_sale, "Esc sale de la pantalla completa y suelta el mouse");
        nota(ui, "Si no, Esc llega a Android como Atrás (en muchos juegos abre el menú).");
        ui.checkbox(&mut c.notificaciones_escritorio, "Notificaciones de Android en el escritorio");
        nota(ui, "Cuando ZYKRO está minimizado o detrás de otra ventana.");
        ui.checkbox(&mut c.accesos_directos, "Accesos directos de las apps en el menú de aplicaciones");
        nota(ui, "Cada app instalada en Android aparece en el menú de la laptop y se abre en ZYKRO.");

        ui.add_space(4.0);
        titulo(ui, "Carpeta de capturas");
        let carpeta = crate::carpeta_capturas(c);
        ui.label(RichText::new(carpeta.display().to_string()).size(12.5).color(ICONO));
        ui.horizontal(|ui| {
            if ui.button(format!("{}  Cambiar…", ic::FOLDER_OPEN)).clicked() {
                if let Some(nueva) = elegir_carpeta(&carpeta) {
                    c.carpeta_capturas = Some(nueva);
                }
            }
            if ui.button(format!("{}  Abrir", ic::ARROW_SQUARE_OUT)).clicked() {
                crate::abrir_carpeta_capturas(c);
            }
            if c.carpeta_capturas.is_some() && ui.button("Usar la de siempre").clicked() {
                c.carpeta_capturas = None;
            }
        });
    }

    fn telefono(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Perfil del teléfono");
        let a = &mut self.borrador.android;
        fila(ui, "Teléfono", |ui| {
            let actual = a.perfil.and_then(|i| PERFILES.get(i)).map_or("Otro", |p| p.nombre);
            egui::ComboBox::from_id_salt("perfil").selected_text(actual).width(260.0).show_ui(ui, |ui| {
                for (i, perfil) in PERFILES.iter().enumerate() {
                    ui.selectable_value(&mut a.perfil, Some(i), perfil.nombre);
                }
            });
        });
        nota(ui, "El teléfono que Android dice ser (marca, modelo y versión). Algunos juegos dan más gráficos o FPS según el modelo. Se aplica reiniciando Android.");

        ui.add_space(4.0);
        titulo(ui, "Operadora de telefonía");
        let c = &mut self.borrador;
        fila(ui, "Operadora", |ui| {
            let actual = c.operadora.as_ref().and_then(|cod| OPERADORAS.iter().find(|o| &o.codigo == cod)).map_or("Ninguna".to_string(), |o| format!("{} · {}", o.nombre, o.pais));
            egui::ComboBox::from_id_salt("operadora").selected_text(actual).width(260.0).height(320.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut c.operadora, None, "Ninguna");
                for o in &OPERADORAS {
                    ui.selectable_value(&mut c.operadora, Some(o.codigo.to_string()), format!("{} · {}", o.nombre, o.pais));
                }
            });
        });
        nota(ui, "La operadora y el país de la SIM que ven las apps: algunas solo funcionan con una red de cierto país.");
    }

    fn atajos(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            titulo(ui, "Atajos de teclado");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Restablecer").clicked() {
                    self.borrador.atajos.clear();
                    self.capturando = None;
                    self.aviso_atajo = None;
                }
            });
        });
        nota(ui, "Toca un atajo y presiona la tecla nueva (con Ctrl, Alt o Shift si quieres). Retroceso lo quita y Esc cancela. La tecla de la puntería es de cada juego: se cambia en el editor de controles.");
        if let Some(aviso) = &self.aviso_atajo {
            ui.label(RichText::new(aviso).size(12.5).color(crate::AVISO));
        }
        let mut grupo = "";
        for atajo in &ATAJOS {
            if atajo.grupo != grupo {
                grupo = atajo.grupo;
                ui.add_space(2.0);
                ui.label(RichText::new(grupo.to_uppercase()).size(10.5).strong().color(TENUE));
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(atajo.nombre).color(TEXTO));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let esperando = self.capturando == Some(atajo.id);
                    let texto = if esperando { "Presiona una tecla…".to_string() } else { atajos::mostrar(atajos::tecla(&self.borrador.atajos, atajo)) };
                    let boton = egui::Button::new(RichText::new(texto).color(if esperando { ACENTO } else { TEXTO }))
                        .min_size(Vec2::new(190.0, 26.0))
                        .stroke(Stroke::new(1.0, if esperando { ACENTO } else { FONDO_HOVER }));
                    if ui.add(boton).clicked() {
                        self.capturando = Some(atajo.id);
                        self.aviso_atajo = None;
                    }
                });
            });
        }
    }

    /// Con un atajo esperando tecla: la toma (y no la deja pasar a la ventana ni a Android).
    fn capturar_tecla(&mut self, ctx: &egui::Context) {
        let Some(id) = self.capturando else { return };
        let pulsada = ctx.input_mut(|i| {
            let pos = i.events.iter().position(|e| matches!(e, Event::Key { pressed: true, repeat: false, .. }))?;
            match i.events.remove(pos) {
                Event::Key { key, modifiers, .. } => Some((key, modifiers)),
                _ => None,
            }
        });
        let Some((tecla, modificadores)) = pulsada else { return };
        self.capturando = None;
        let sin_mods = !modificadores.ctrl && !modificadores.alt && !modificadores.shift && !modificadores.command;
        let nueva = match tecla {
            Key::Escape if sin_mods => return,
            Key::Backspace if sin_mods => String::new(),
            _ => atajos::texto(modificadores, tecla),
        };
        if let Some(otro) = ATAJOS.iter().find(|a| a.id != id && !nueva.is_empty() && atajos::tecla(&self.borrador.atajos, a) == nueva) {
            self.aviso_atajo = Some(format!("{} ya es el atajo de «{}»", atajos::mostrar(&nueva), otro.nombre));
            return;
        }
        let Some(atajo) = ATAJOS.iter().find(|a| a.id == id) else { return };
        if nueva == atajo.tecla {
            self.borrador.atajos.remove(id);
        } else {
            self.borrador.atajos.insert(id.to_string(), nueva);
        }
    }

    fn datos(&mut self, ui: &mut egui::Ui, cerrar: &mut bool) {
        let (ocupado, aviso, respaldo) = {
            let t = self.trabajo.lock().unwrap();
            (t.ocupado, t.aviso.clone(), t.respaldo.as_ref().map(|(escrito, total)| (escrito.load(Ordering::Relaxed), *total)))
        };
        titulo(ui, "Limpieza del disco");
        nota(ui, "Borra las carpetas que dejaron las apps desinstaladas (sus datos y descargas) y los archivos temporales. La caché de las apps instaladas se borra en los ajustes de Android.");
        if ui.add_enabled(!ocupado, egui::Button::new(format!("{}  Limpiar el disco", ic::BROOM))).clicked() {
            self.trabajar(&self.trabajo, ui.ctx(), |_| datos::limpiar());
        }

        ui.add_space(4.0);
        titulo(ui, "Respaldo");
        nota(ui, "Guarda todo Android (apps, partidas, sesiones y archivos) en un solo archivo, para restaurarlo después o en otra laptop con ZYKRO. Cierra tus juegos antes de respaldar.");
        ui.horizontal(|ui| {
            if ui.add_enabled(!ocupado, egui::Button::new(format!("{}  Respaldar…", ic::DOWNLOAD_SIMPLE))).clicked() {
                if let Some(destino) = elegir_archivo(true) {
                    self.respaldar(ui.ctx(), destino);
                }
            }
            if ui.add_enabled(!ocupado, egui::Button::new(format!("{}  Restaurar…", ic::UPLOAD_SIMPLE))).clicked() {
                self.confirmar_restaurar = elegir_archivo(false);
            }
        });
        if let Some((escrito, total)) = respaldo.filter(|_| ocupado) {
            let avance = if total > 0 { (escrito as f32 / total as f32).min(0.99) } else { 0.0 };
            ui.add(egui::ProgressBar::new(avance).text(format!("{} de unos {}", datos::tamano(escrito), datos::tamano(total))));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        } else if ocupado {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0).color(TENUE));
                nota(ui, "Trabajando…");
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
        if !aviso.is_empty() && !ocupado {
            ui.label(RichText::new(aviso).size(12.5).color(ICONO));
        }
        if let Some(respaldo) = self.confirmar_restaurar.clone() {
            egui::Frame::new().fill(FONDO_CONTROL).corner_radius(6.0).inner_margin(12).show(ui, |ui| {
                ui.label(RichText::new(format!("¿Restaurar {}?", respaldo.file_name().unwrap_or_default().to_string_lossy())).strong().color(TEXTO));
                nota(ui, "Todo lo que tiene Android ahora se reemplaza por lo del respaldo. ZYKRO se reinicia para hacerlo.");
                ui.horizontal(|ui| {
                    let boton = egui::Button::new(RichText::new("Restaurar").color(egui::Color32::WHITE).strong()).fill(crate::CERRAR_HOVER);
                    if ui.add(boton).clicked() {
                        self.restaurar = Some(respaldo.clone());
                        self.confirmar_restaurar = None;
                        *cerrar = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        self.confirmar_restaurar = None;
                    }
                });
            });
        }
    }

    /// Corre `trabajo` en otro hilo con `estado` ocupado; lo que devuelve queda como aviso.
    fn trabajar(&self, estado: &Arc<Mutex<Trabajo>>, ctx: &egui::Context, trabajo: impl FnOnce(&Arc<Mutex<Trabajo>>) -> String + Send + 'static) {
        let (estado, ctx) = (estado.clone(), ctx.clone());
        estado.lock().unwrap().ocupado = true;
        thread::spawn(move || {
            let aviso = trabajo(&estado);
            let mut t = estado.lock().unwrap();
            (t.ocupado, t.aviso, t.respaldo) = (false, aviso, None);
            ctx.request_repaint();
        });
    }

    fn respaldar(&self, ctx: &egui::Context, destino: PathBuf) {
        self.trabajar(&self.trabajo, ctx, move |estado| {
            let escrito = Arc::new(AtomicU64::new(0));
            estado.lock().unwrap().respaldo = Some((escrito.clone(), datos::tamano_respaldo()));
            match datos::respaldar(&destino, &escrito) {
                Ok(()) => format!("Respaldo guardado: {} ({})", destino.display(), datos::tamano(escrito.load(Ordering::Relaxed))),
                Err(e) => format!("No se pudo respaldar: {e}"),
            }
        });
    }

    fn avanzado(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Arquitecturas (ABI)");
        let abi = if self.abi.is_empty() { "…" } else { &self.abi };
        ui.label(RichText::new(abi).size(12.5).color(ICONO));
        nota(ui, "Las apps hechas para teléfonos (ARM) corren con Houdini, el traductor de Intel, sobre el procesador de la laptop.");

        ui.add_space(4.0);
        titulo(ui, "Android Debug Bridge (ADB)");
        ui.checkbox(&mut self.borrador.adb, "Permitir conectarse con ADB");
        nota(ui, "Para programadores y herramientas que usan adb (instalar desde la computadora, ver el registro, scrcpy).");
        if self.borrador.adb {
            ui.horizontal(|ui| {
                let orden = "adb connect 127.0.0.1:5555";
                ui.label(RichText::new(orden).monospace().color(TEXTO));
                if ui.button(format!("{}  Copiar", ic::COPY)).clicked() {
                    ui.ctx().copy_text(orden.into());
                }
            });
        }
    }

    fn acerca(&mut self, ui: &mut egui::Ui) {
        titulo(ui, "Acerca de");
        ui.label(RichText::new(format!("{NOMBRE} {VERSION}")).size(18.0).strong().color(TEXTO));
        ui.label(RichText::new("Emulador de Android para jugar en Linux. Software libre (MIT).").color(ICONO));
        let commit = env!("ZYKRO_COMMIT");
        if !commit.is_empty() {
            nota(ui, &format!("Compilado del commit {}", &commit[..commit.len().min(7)]));
        }
        let (ocupado, aviso) = {
            let t = self.actualizacion.lock().unwrap();
            (t.ocupado, t.aviso.clone())
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(!ocupado && !commit.is_empty(), egui::Button::new(format!("{}  Buscar actualizaciones", ic::ARROWS_CLOCKWISE))).clicked() {
                self.trabajar(&self.actualizacion, ui.ctx(), move |_| buscar_actualizacion(commit));
            }
            if ui.button(format!("{}  Ver en GitHub", ic::ARROW_SQUARE_OUT)).clicked() {
                let _ = Command::new("xdg-open").arg(REPOSITORIO).spawn();
            }
        });
        if ocupado {
            ui.add(egui::Spinner::new().size(14.0).color(TENUE));
        } else if !aviso.is_empty() {
            ui.label(RichText::new(&aviso).size(12.5).color(ICONO));
            if aviso.starts_with("Hay una versión nueva") && carpeta_repositorio().is_some() && ui.button(format!("{}  Actualizar", ic::DOWNLOAD_SIMPLE)).clicked() {
                actualizar();
            }
        }
    }
}

/// Lo que la ventana lee de Android al abrirse: las apps propias con sus íconos y las arquitecturas.
fn leer() -> Leido {
    const EXPORTADO: &str = "/data/media/0/Android/data/com.zykro.inicio/files";
    let mut leido = Leido { abi: android::shell("getprop ro.product.cpu.abilist").unwrap_or_default().trim().replace(',', ", "), ..Default::default() };
    let lista = android::shell(&format!("cat {EXPORTADO}/apps.json")).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    for app in lista.as_ref().and_then(|l| l.as_array()).into_iter().flatten() {
        if let (Some(paquete), Some(nombre), Some(false)) = (app["paquete"].as_str(), app["nombre"].as_str(), app["sistema"].as_bool()) {
            leido.apps.push((paquete.to_string(), nombre.to_string()));
        }
    }
    let carpeta = std::env::temp_dir().join(format!("zykro-ajustes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&carpeta);
    if android::copiar_desde(&format!("{EXPORTADO}/iconos"), &carpeta) {
        for (paquete, _) in &leido.apps {
            if let Some(imagen) = crate::ajustes_android::leer_png(&carpeta.join(format!("{paquete}.png"))) {
                leido.iconos.push((paquete.clone(), imagen));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&carpeta);
    leido
}

/// El diálogo de KDE (o el de GNOME) para elegir una carpeta.
fn elegir_carpeta(desde: &std::path::Path) -> Option<PathBuf> {
    let salida = Command::new("kdialog")
        .args(["--title", "Carpeta de capturas", "--getexistingdirectory"])
        .arg(desde)
        .output()
        .or_else(|_| Command::new("zenity").args(["--file-selection", "--directory", "--title=Carpeta de capturas"]).output())
        .ok()?;
    let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    (salida.status.success() && !ruta.is_empty()).then(|| PathBuf::from(ruta))
}

/// El diálogo para guardar (`guardar`) o abrir un respaldo.
fn elegir_archivo(guardar: bool) -> Option<PathBuf> {
    let inicio = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let fecha = Command::new("date").arg("+%Y-%m-%d").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let sugerido = inicio.join(format!("zykro-respaldo-{fecha}.tar"));
    let (modo, titulo) = if guardar { ("--getsavefilename", "Guardar el respaldo") } else { ("--getopenfilename", "Restaurar un respaldo") };
    let salida = Command::new("kdialog")
        .args(["--title", titulo, modo])
        .arg(if guardar { &sugerido } else { &inicio })
        .arg("*.tar|Respaldos de ZYKRO")
        .output()
        .or_else(|_| {
            let mut zenity = Command::new("zenity");
            zenity.args(["--file-selection", &format!("--title={titulo}"), "--file-filter=Respaldos de ZYKRO | *.tar"]);
            if guardar {
                zenity.args(["--save", "--confirm-overwrite"]).arg(format!("--filename={}", sugerido.display()));
            }
            zenity.output()
        })
        .ok()?;
    let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    (salida.status.success() && !ruta.is_empty()).then(|| PathBuf::from(ruta))
}

/// Compara el commit con el que se compiló ZYKRO con el último de GitHub.
fn buscar_actualizacion(commit: &str) -> String {
    let salida = Command::new("git").args(["ls-remote", REPOSITORIO, "refs/heads/main"]).output();
    match salida.ok().filter(|s| s.status.success()).map(|s| String::from_utf8_lossy(&s.stdout).split_whitespace().next().unwrap_or("").to_string()) {
        Some(ultimo) if ultimo == commit => "Tienes la versión más nueva.".into(),
        Some(ultimo) if !ultimo.is_empty() => "Hay una versión nueva en GitHub.".into(),
        _ => "No se pudo revisar (¿hay internet?).".into(),
    }
}

/// La carpeta del repositorio con el que se compiló ZYKRO, si sigue ahí.
fn carpeta_repositorio() -> Option<PathBuf> {
    let carpeta = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent()?.to_path_buf();
    (carpeta.join(".git").exists() && carpeta.join("instalar.sh").exists()).then_some(carpeta)
}

/// Baja lo nuevo e instala, en una terminal (instalar.sh puede pedir la contraseña).
fn actualizar() {
    let Some(carpeta) = carpeta_repositorio() else { return };
    let orden = format!("cd '{}' && git pull && ./instalar.sh; echo; echo 'Listo: vuelve a abrir ZYKRO.'; read _", carpeta.display());
    if Command::new("konsole").args(["-e", "sh", "-c", &orden]).spawn().is_err() {
        let _ = Command::new("x-terminal-emulator").args(["-e", "sh", "-c", &orden]).spawn();
    }
}

/// Un modo de rendimiento como opción elegible: ícono, nombre y descripción.
fn opcion_modo(ui: &mut egui::Ui, modo: Modo, elegido: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
    let fondo = if elegido { FONDO_HOVER } else if resp.hovered() { FONDO_CONTROL } else { FONDO };
    ui.painter().rect_filled(rect, 6.0, fondo);
    if elegido {
        ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.5, ACENTO), egui::StrokeKind::Inside);
    }
    let glifo = match modo {
        Modo::Ahorro => ic::LEAF,
        Modo::Equilibrado => ic::SCALES,
        Modo::Rendimiento => ic::LIGHTNING,
        Modo::Ultra => ic::FIRE,
    };
    let color = if elegido { ACENTO } else { ICONO };
    let y = rect.center().y;
    ui.painter().text(egui::pos2(rect.min.x + 20.0, y), Align2::CENTER_CENTER, glifo, FontId::proportional(17.0), color);
    ui.painter().text(egui::pos2(rect.min.x + 40.0, y - 8.0), Align2::LEFT_CENTER, modo.nombre(), FontId::proportional(13.5), TEXTO);
    ui.painter().text(egui::pos2(rect.min.x + 40.0, y + 9.0), Align2::LEFT_CENTER, modo.descripcion(), FontId::proportional(11.5), TENUE);
    resp
}

pub(crate) fn titulo(ui: &mut egui::Ui, texto: &str) {
    ui.label(RichText::new(texto).size(16.0).strong().color(TEXTO));
}

pub(crate) fn nota(ui: &mut egui::Ui, texto: &str) {
    ui.label(RichText::new(texto).size(12.0).color(TENUE));
}

pub(crate) fn fila(ui: &mut egui::Ui, etiqueta: &str, contenido: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(150.0, 24.0), egui::Label::new(RichText::new(etiqueta).color(ICONO)));
        contenido(ui);
    });
}

pub(crate) fn linea(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, FONDO_HOVER);
}

pub(crate) fn pestana(ui: &mut egui::Ui, elegida: bool, glifo: &str, nombre: &str) -> egui::Response {
    // 36 de alto: caben las once pestañas sin desplazar.
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ANCHO_PESTANAS, 36.0), Sense::click());
    if elegida || resp.hovered() {
        ui.painter().rect_filled(rect.shrink2(Vec2::new(8.0, 2.0)), 4.0, FONDO_HOVER);
    }
    if elegida {
        let barra = egui::Rect::from_min_size(rect.min + Vec2::new(8.0, 9.0), Vec2::new(3.0, 18.0));
        ui.painter().rect_filled(barra, 1.5, ACENTO);
    }
    let color = if elegida { TEXTO } else { ICONO };
    let y = rect.center().y;
    ui.painter().text(egui::pos2(rect.min.x + 24.0, y), Align2::LEFT_CENTER, glifo, FontId::proportional(15.0), color);
    ui.painter().text(egui::pos2(rect.min.x + 48.0, y), Align2::LEFT_CENTER, nombre, FontId::proportional(13.5), color);
    resp
}
