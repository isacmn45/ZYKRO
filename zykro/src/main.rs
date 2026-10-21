//! ZYKRO: ventana propia para el Android de SwagPlayer (redroid + swag-rust-server).
//! Diseño inspirado en LDPlayer: barra de título oscura y barra lateral delgada con herramientas.

mod ajustes;
mod android;
mod carga;
mod config;
mod control;
mod curva;
mod editor;
mod mapeo;
mod menu;
mod raton;
mod video;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Event, FontId, Layout, MouseWheelUnit, PointerButton, Pos2, Rect,
    ResizeDirection, RichText, Sense, Shape, Stroke, TextureOptions, Vec2, ViewportCommand,
};
use egui_phosphor::regular as ic;

use control::{Control, BAJAR, META_ALT, META_CTRL, META_SHIFT, MOVER, SOLTAR};

const NOMBRE: &str = "ZYKRO";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const ALTO_TITULO: f32 = 30.0;
const ANCHO_LATERAL: f32 = 34.0;
/// Refrescos seguidos con un cuadro de retraso antes de saltar uno (medio segundo a 60 Hz).
const MAX_ATRASADOS: u32 = 30;
const BORDE: f32 = 5.0; // zona en la orilla para cambiar el tamaño

const FONDO_OSCURO: Color32 = Color32::from_rgb(0x12, 0x13, 0x22);
const FONDO_CONTROL: Color32 = Color32::from_rgb(0x26, 0x28, 0x3e);
const FONDO: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x2d);
const FONDO_HOVER: Color32 = Color32::from_rgb(0x2e, 0x30, 0x4a);
const CERRAR_HOVER: Color32 = Color32::from_rgb(0xe0, 0x3e, 0x3e);
const ICONO: Color32 = Color32::from_rgb(0xc4, 0xc6, 0xd4);
const TENUE: Color32 = Color32::from_rgb(0x80, 0x83, 0x99);
const TEXTO: Color32 = Color32::from_rgb(0xe8, 0xe9, 0xf2);
const ACENTO: Color32 = Color32::from_rgb(0xf6, 0xc3, 0x1c);

fn main() -> eframe::Result {
    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(NOMBRE)
            .with_app_id("zykro")
            .with_decorations(false)
            .with_inner_size([1280.0 + ANCHO_LATERAL, 720.0 + ALTO_TITULO])
            .with_min_inner_size([480.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native(NOMBRE, opciones, Box::new(|cc| Ok(Box::new(Zykro::new(cc)))))
}

struct Zykro {
    estado: android::EstadoCompartido,
    video: video::Compartido,
    textura: Option<egui::TextureHandle>,
    /// Último cuadro mostrado (para las capturas de pantalla).
    cuadro: Option<video::Cuadro>,
    /// Refrescos seguidos mostrando el cuadro con uno de retraso (ver `actualizar_textura`).
    atrasados: u32,
    lateral: bool,
    tocando: Option<Pos2>,
    fps: u32,
    cuadros_seg: u32,
    inicio_seg: Instant,
    ajustes: ajustes::Ajustes,
    motor: Arc<Mutex<mapeo::Motor>>,
    /// El hilo de raton.rs manda la puntería directo (la ventana no procesa el mouse).
    raton_directo: Arc<AtomicBool>,
    raton_listo: Arc<AtomicBool>,
    editor: editor::Editor,
    /// Menú rápido del botón de controles (sensibilidad), como el de BlueStacks.
    menu: menu::Menu,
    /// El mapeo está convirtiendo teclas en toques en este momento.
    mapeo_activo: bool,
    cursor_bloqueado: bool,
    apuntando_antes: bool,
    mods_previos: egui::Modifiers,
    revision_app: Instant,
}

impl Zykro {
    fn new(cc: &eframe::CreationContext) -> Self {
        let mut fuentes = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fuentes, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fuentes);
        cc.egui_ctx.set_visuals(visuales());

        let estado: android::EstadoCompartido = Arc::new(Mutex::new(android::Estado {
            config: config::Config::cargar(),
            ..Default::default()
        }));
        let video: video::Compartido = Arc::default();
        android::iniciar(cc.egui_ctx.clone(), estado.clone(), video.clone());
        android::vigilar_texto(cc.egui_ctx.clone(), estado.clone());
        let motor = Arc::new(Mutex::new(mapeo::Motor::new()));
        let raton_directo = Arc::new(AtomicBool::new(false));
        let raton_listo = Arc::new(AtomicBool::new(false));
        raton::iniciar(motor.clone(), raton_directo.clone(), raton_listo.clone());
        Zykro {
            estado,
            video,
            textura: None,
            cuadro: None,
            atrasados: 0,
            lateral: true,
            tocando: None,
            fps: 0,
            cuadros_seg: 0,
            inicio_seg: Instant::now(),
            ajustes: ajustes::Ajustes::new(),
            motor,
            raton_directo,
            raton_listo,
            editor: editor::Editor::new(),
            menu: menu::Menu::default(),
            mapeo_activo: false,
            cursor_bloqueado: false,
            apuntando_antes: false,
            mods_previos: egui::Modifiers::NONE,
            revision_app: Instant::now() - Duration::from_secs(5),
        }
    }

    fn config(&self) -> config::Config {
        self.estado.lock().unwrap().config.clone()
    }

    fn abrir_ajustes(&mut self) {
        let actual = self.config();
        self.ajustes.abrir(&actual);
    }

    fn aplicar_config(&mut self, nueva: config::Config) {
        nueva.guardar();
        let anterior = std::mem::replace(&mut self.estado.lock().unwrap().config, nueva.clone());
        if nueva.android != anterior.android {
            android::pedir_reinicio(&self.estado, nueva.android);
        } else {
            android::avisar(&self.estado, "Configuración guardada");
        }
    }

    fn motor(&self) -> MutexGuard<'_, mapeo::Motor> {
        self.motor.lock().unwrap()
    }

    fn dimensiones(&self) -> (f32, f32) {
        let (w, h) = self.estado.lock().unwrap().pantalla.unwrap_or((1280, 720));
        (w as f32, h as f32)
    }

    fn abrir_editor(&mut self) {
        if self.editor.abierto {
            self.editor.abierto = false;
            return;
        }
        let (paquete, mapa) = {
            let m = self.motor();
            (m.paquete.clone(), m.mapa.clone())
        };
        self.editor.abrir(&paquete, mapa);
    }

    /// Botón de controles: abre o cierra el menú rápido (o cierra el editor si está abierto).
    fn boton_controles(&mut self, boton: &egui::Response, abajo: bool) {
        if !boton.clicked() {
            return;
        }
        if self.editor.abierto {
            self.editor.abierto = false;
        } else if self.menu.abierto() {
            self.cerrar_menu();
        } else {
            self.menu.abrir(boton.rect, abajo);
        }
    }

    /// Cierra el menú rápido y guarda la sensibilidad si cambió.
    fn cerrar_menu(&mut self) {
        if !self.menu.cerrar() {
            return;
        }
        let (paquete, mapa) = {
            let m = self.motor();
            (m.paquete.clone(), m.mapa.clone())
        };
        if let Some(mapa) = mapa.filter(|_| !paquete.is_empty()) {
            let ok = mapa.guardar(&paquete).is_ok();
            android::avisar(&self.estado, if ok { "Sensibilidad guardada" } else { "No se pudo guardar la sensibilidad" });
        }
    }

    /// Decide si el mapeo está activo, bloquea el cursor en modo puntería y corre los temporizadores.
    fn actualizar_mapeo(&mut self, ctx: &egui::Context) {
        let (w, h) = self.dimensiones();
        let control = self.control();
        if self.revision_app.elapsed() > Duration::from_secs(1) {
            if let Some(app) = android::app_activa() {
                self.motor().cambiar_app(&app, control.as_ref(), w, h);
            }
            self.revision_app = Instant::now();
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        let escribiendo = self.estado.lock().unwrap().escribiendo;
        let enfocada = ctx.input(|i| i.focused);
        let mut motor = self.motor.lock().unwrap();
        motor.fijar_salida(control.clone(), w, h);
        let activo = motor.mapa.is_some()
            && !escribiendo
            && !self.editor.abierto
            && !self.ajustes.abierto
            && !self.menu.abierto()
            && enfocada;
        if self.mapeo_activo && !activo {
            if let Some(c) = &control {
                motor.soltar_todo(c, w, h);
            }
        }
        self.mapeo_activo = activo;

        let bloquear = activo && motor.cursor_bloqueado();
        self.raton_directo.store(bloquear && self.raton_listo.load(Ordering::Relaxed), Ordering::Relaxed);
        if bloquear != self.cursor_bloqueado {
            let modo = if bloquear { egui::CursorGrab::Locked } else { egui::CursorGrab::None };
            ctx.send_viewport_cmd(ViewportCommand::CursorGrab(modo));
            ctx.send_viewport_cmd(ViewportCommand::CursorVisible(!bloquear));
            self.cursor_bloqueado = bloquear;
        }
        if motor.apuntando != self.apuntando_antes {
            self.apuntando_antes = motor.apuntando;
            let tecla = motor.mapa.as_ref().map(|m| mapeo::etiqueta(&m.cambio)).unwrap_or_default();
            let texto = if motor.apuntando {
                format!("Puntería activada · {tecla} para soltar el cursor")
            } else {
                format!("Cursor libre · {tecla} para apuntar")
            };
            android::avisar(&self.estado, texto);
        }
        if activo {
            if let Some(c) = &control {
                motor.tick(c, w, h);
            }
            if motor.ocupado() {
                ctx.request_repaint_after(Duration::from_millis(8));
            }
            // Sin el hilo del mouse, aquí se levanta el dedo de la cámara cuando se queda quieto.
            if let Some(falta) = motor.espera_quieto() {
                ctx.request_repaint_after(falta);
            }
        }
    }

    fn panel_editor(&mut self, raiz: &mut egui::Ui) {
        let salida = egui::Panel::right("editor")
            .exact_size(272.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(FONDO))
            .show_separator_line(false)
            .show(raiz, |ui| self.editor.panel(ui))
            .inner;
        if let editor::Salida::Guardar(mapa) = salida {
            let paquete = self.motor().paquete.clone();
            if paquete.is_empty() {
                android::avisar(&self.estado, "Abre un juego antes de guardar sus controles");
            } else if mapa.guardar(&paquete).is_ok() {
                self.motor().mapa = Some(mapa);
                android::avisar(&self.estado, "Controles guardados");
            } else {
                android::avisar(&self.estado, "No se pudieron guardar los controles");
            }
        }
    }

    /// Con el editor abierto, las teclas se asignan a los controles en vez de ir a Android.
    fn teclas_editor(&mut self, ctx: &egui::Context) {
        let (eventos, mods) = ctx.input(|i| (i.events.clone(), i.modifiers));
        for (antes, ahora, nombre) in [
            (self.mods_previos.shift, mods.shift, "Key_Shift"),
            (self.mods_previos.ctrl, mods.ctrl, "Key_Control"),
            (self.mods_previos.alt, mods.alt, "Key_Alt"),
        ] {
            if ahora && !antes && self.editor.esperando_tecla() {
                self.editor.tecla(nombre);
            }
        }
        self.mods_previos = mods;
        for evento in eventos {
            match evento {
                Event::Key { key, pressed: true, repeat: false, .. } => {
                    if let Some(nombre) = mapeo::nombre_tecla(key) {
                        self.editor.tecla(&nombre);
                    }
                }
                Event::PointerButton { button, pressed: true, .. } if button != PointerButton::Primary && self.editor.esperando_tecla() => {
                    self.editor.tecla(mapeo::nombre_boton(button));
                }
                _ => {}
            }
        }
    }

    fn control(&self) -> Option<Control> {
        self.estado.lock().unwrap().control.clone()
    }

    fn pulsar(&self, codigo: u32) {
        if let Some(c) = self.control() {
            c.pulsar(codigo);
        }
    }

    /// Sube a la GPU el cuadro nuevo, si llegó uno.
    fn actualizar_textura(&mut self, ctx: &egui::Context) {
        // Un cuadro por refresco, en orden; si queda otro en la cola, se muestra en el siguiente.
        // Si eso dura medio segundo (cada cuadro sale un refresco tarde, ~17 ms), se salta uno para
        // no arrastrar ese retraso.
        let (cuadro, quedan) = {
            let mut cola = self.video.lock().unwrap();
            if cola.len() > 1 && self.atrasados >= MAX_ATRASADOS {
                cola.pop_front();
            }
            (cola.pop_front(), !cola.is_empty())
        };
        self.atrasados = if quedan { (self.atrasados + 1).min(MAX_ATRASADOS) } else { 0 };
        let Some(cuadro) = cuadro else { return };
        if quedan {
            ctx.request_repaint();
        }
        let imagen = egui::ColorImage::from_rgba_unmultiplied([cuadro.ancho, cuadro.alto], &cuadro.rgba);
        match &mut self.textura {
            Some(t) => t.set(imagen, TextureOptions::LINEAR),
            None => self.textura = Some(ctx.load_texture("android", imagen, TextureOptions::LINEAR)),
        }
        self.cuadro = Some(cuadro);
        self.cuadros_seg += 1;
    }

    fn contar_fps(&mut self) {
        if self.inicio_seg.elapsed() >= Duration::from_secs(1) {
            self.fps = self.cuadros_seg;
            self.cuadros_seg = 0;
            self.inicio_seg = Instant::now();
        }
    }

    fn captura(&self) {
        let Some(c) = &self.cuadro else { return };
        let (ancho, alto, rgba) = (c.ancho as u32, c.alto as u32, c.rgba.clone());
        let estado = self.estado.clone();
        thread::spawn(move || {
            let carpeta = carpeta_capturas();
            let _ = std::fs::create_dir_all(&carpeta);
            let fecha = Command::new("date").arg("+%Y%m%d-%H%M%S").output();
            let fecha = fecha.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
            let ruta = carpeta.join(format!("captura-{fecha}.png"));
            let ok = guardar_png(&ruta, ancho, alto, &rgba).is_ok();
            android::avisar(&estado, if ok { "Captura guardada en Imágenes" } else { "No se pudo guardar la captura" });
        });
    }

    fn barra_titulo(&mut self, raiz: &mut egui::Ui, en_borde: bool) {
        let ctx = raiz.ctx().clone();
        let ctx = &ctx;
        let maximizado = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        egui::Panel::top("titulo")
            .exact_size(ALTO_TITULO)
            .frame(egui::Frame::new().fill(FONDO))
            .show_separator_line(false)
            .show(raiz, |ui| {
                // Arrastrar la barra mueve la ventana; doble clic la maximiza.
                let arrastre = ui.interact(ui.max_rect(), ui.id().with("arrastre"), Sense::click_and_drag());
                if arrastre.drag_started_by(PointerButton::Primary) && !en_borde {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if arrastre.double_clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximizado));
                }

                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    logo(ui);
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("{NOMBRE} {VERSION}")).color(TEXTO).size(12.5));
                    if self.config().mostrar_fps {
                        ui.add_space(6.0);
                        ui.label(RichText::new(format!("{} FPS", self.fps)).color(TENUE).size(11.0));
                        // Refrescar el contador aunque Android no esté dibujando.
                        ctx.request_repaint_after(Duration::from_secs(1));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let t = Vec2::new(34.0, ALTO_TITULO);
                        let lado = if self.lateral { ic::CARET_DOUBLE_RIGHT } else { ic::CARET_DOUBLE_LEFT };
                        if icono(ui, lado, "Barra lateral", t, FONDO_HOVER).clicked() {
                            self.lateral = !self.lateral;
                        }
                        ui.add_space(6.0);
                        if icono(ui, ic::X, "Cerrar", t, CERRAR_HOVER).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        let max = if maximizado { ic::COPY_SIMPLE } else { ic::SQUARE };
                        if icono(ui, max, "Maximizar", t, FONDO_HOVER).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximizado));
                        }
                        if icono(ui, ic::MINUS, "Minimizar", t, FONDO_HOVER).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        if icono(ui, ic::LIST, "Configuración", t, FONDO_HOVER).clicked() {
                            self.abrir_ajustes();
                        }
                        let boton = icono(ui, ic::GAME_CONTROLLER, "Controles del juego", t, FONDO_HOVER);
                        self.boton_controles(&boton, true);
                    });
                });
            });
    }

    fn barra_lateral(&mut self, raiz: &mut egui::Ui) {
        let ctx = raiz.ctx().clone();
        let ctx = &ctx;
        egui::Panel::right("lateral")
            .exact_size(ANCHO_LATERAL)
            .resizable(false)
            .frame(egui::Frame::new().fill(FONDO))
            .show_separator_line(false)
            .show(raiz, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let t = Vec2::new(ANCHO_LATERAL, 30.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    if icono(ui, ic::GEAR, "Configuración", t, FONDO_HOVER).clicked() {
                        self.abrir_ajustes();
                    }
                    let boton = icono(ui, ic::KEYBOARD, "Controles del juego", t, FONDO_HOVER);
                    self.boton_controles(&boton, false);
                    if icono(ui, ic::SPEAKER_HIGH, "Subir volumen", t, FONDO_HOVER).clicked() {
                        self.pulsar(24);
                    }
                    if icono(ui, ic::SPEAKER_LOW, "Bajar volumen", t, FONDO_HOVER).clicked() {
                        self.pulsar(25);
                    }
                    if icono(ui, ic::CORNERS_OUT, "Pantalla completa (F11)", t, FONDO_HOVER).clicked() {
                        alternar_pantalla_completa(ctx);
                    }
                    if icono(ui, ic::CAMERA, "Captura de pantalla", t, FONDO_HOVER).clicked() {
                        self.captura();
                    }
                    if icono(ui, ic::PACKAGE, "Instalar APK", t, FONDO_HOVER).clicked() {
                        android::instalar_apk(self.estado.clone());
                    }
                    if icono(ui, ic::FOLDER_SIMPLE, "Abrir carpeta de capturas", t, FONDO_HOVER).clicked() {
                        let carpeta = carpeta_capturas();
                        let _ = std::fs::create_dir_all(&carpeta);
                        let _ = Command::new("xdg-open").arg(carpeta).spawn();
                    }
                });
                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.add_space(8.0);
                    let t = Vec2::new(ANCHO_LATERAL, 34.0);
                    if boton_nav(ui, Nav::Recientes, t).clicked() {
                        self.pulsar(187);
                    }
                    if boton_nav(ui, Nav::Inicio, t).clicked() {
                        self.pulsar(3);
                    }
                    if boton_nav(ui, Nav::Atras, t).clicked() {
                        self.pulsar(4);
                    }
                });
            });
    }

    fn pantalla(&mut self, raiz: &mut egui::Ui, en_borde: bool) {
        let ctx = raiz.ctx().clone();
        let ctx = &ctx;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::BLACK)).show(raiz, |ui| {
            let area = ui.max_rect();
            let (ancho, alto, carga, aviso) = {
                let s = self.estado.lock().unwrap();
                let (w, h) = s.pantalla.unwrap_or((1280, 720));
                let aviso = s.aviso.as_ref().filter(|(_, t)| t.elapsed() < Duration::from_millis(2500)).map(|a| a.0.clone());
                let carga = carga::Vista {
                    etapa: s.etapa,
                    desde: s.etapa_desde,
                    mensaje: s.mensaje.clone(),
                    error: s.error,
                    sobre_video: self.textura.is_some(),
                };
                (w as f32, h as f32, carga, aviso)
            };
            let escala = (area.width() / ancho).min(area.height() / alto);
            let rect = Rect::from_center_size(area.center(), Vec2::new(ancho * escala, alto * escala));

            if let Some(t) = &self.textura {
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                ui.painter().image(t.id(), rect, uv, Color32::WHITE);
            }
            let escribiendo = self.estado.lock().unwrap().escribiendo;
            if self.editor.abierto {
                self.editor.pantalla(ui, rect);
            } else if let Some(mapa) = &self.motor().mapa {
                if mapa.overlay.enabled && !escribiendo {
                    editor::dibujar(&ui.painter_at(rect), rect, mapa, mapa.overlay.opacity, None, false);
                }
            }
            if escribiendo && self.motor().mapa.is_some() {
                let texto = format!("{}  Escribiendo · controles en pausa", ic::KEYBOARD);
                let galeria = ui.painter().layout_no_wrap(texto, FontId::proportional(13.0), TEXTO);
                let caja = Rect::from_center_size(rect.center_top() + Vec2::new(0.0, 22.0), galeria.size() + Vec2::new(22.0, 12.0));
                ui.painter().rect_filled(caja, 12.0, FONDO.gamma_multiply(0.92));
                ui.painter().galley(caja.center() - galeria.size() / 2.0, galeria, TEXTO);
            }
            if self.textura.is_none() || !carga.mensaje.is_empty() {
                carga::dibujar(ui, area, &carga);
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            if let Some(texto) = aviso {
                let pos = area.right_bottom() - Vec2::new(12.0, 12.0);
                let galeria = ui.painter().layout_no_wrap(texto, FontId::proportional(13.0), TEXTO);
                let caja = Rect::from_min_size(pos - galeria.size() - Vec2::splat(12.0), galeria.size() + Vec2::splat(12.0));
                ui.painter().rect_filled(caja, 6.0, FONDO.gamma_multiply(0.92));
                ui.painter().galley(caja.min + Vec2::splat(6.0), galeria, TEXTO);
                ctx.request_repaint_after(Duration::from_millis(200));
            }

            if self.editor.abierto {
                self.teclas_editor(ctx);
            } else if !en_borde && !self.ajustes.abierto && !self.menu.abierto() {
                self.entrada(ctx, rect, ancho, alto);
            }
        });
    }

    /// Mouse como dedo, rueda como scroll y teclado como teclado de Android.
    fn entrada(&mut self, ctx: &egui::Context, rect: Rect, ancho: f32, alto: f32) {
        let Some(control) = self.control() else { return };
        let (w, h) = (ancho as u16, alto as u16);
        let a_android = |p: Pos2| {
            let x = ((p.x - rect.min.x) / rect.width() * ancho).clamp(0.0, ancho - 1.0);
            let y = ((p.y - rect.min.y) / rect.height() * alto).clamp(0.0, alto - 1.0);
            (x as i32, y as i32)
        };
        let (eventos, puntero, mods) = ctx.input(|i| (i.events.clone(), i.pointer.hover_pos(), i.modifiers));
        let (activo, bloqueado) = (self.mapeo_activo, self.cursor_bloqueado);
        // Con la puntería activa, el hilo de raton.rs ya manda el mouse directo a Android.
        let directo = self.raton_directo.load(Ordering::Relaxed);
        // egui no avisa de Shift/Ctrl/Alt solos: se detectan comparando con el cuadro anterior.
        for (antes, ahora, nombre) in [
            (self.mods_previos.shift, mods.shift, "Key_Shift"),
            (self.mods_previos.ctrl, mods.ctrl, "Key_Control"),
            (self.mods_previos.alt, mods.alt, "Key_Alt"),
        ] {
            if activo && antes != ahora {
                self.motor().tecla(nombre, ahora, false, &control, ancho, alto);
            }
        }
        self.mods_previos = mods;
        for evento in eventos {
            match evento {
                Event::MouseMoved(delta) if activo && bloqueado => {
                    if !directo {
                        self.motor().mover(delta, &control, ancho, alto);
                    }
                }
                Event::PointerButton { pos, button, pressed, .. } if activo && (bloqueado || button != PointerButton::Primary) => {
                    if !(bloqueado && directo) && (bloqueado || !pressed || rect.contains(pos)) {
                        self.motor().boton(button, pressed, &control, ancho, alto);
                    }
                }
                Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } if rect.contains(pos) => {
                    let (x, y) = a_android(pos);
                    control.toque(BAJAR, 0, x, y, w, h);
                    self.tocando = Some(pos);
                }
                Event::PointerMoved(pos) if self.tocando.is_some() => {
                    let (x, y) = a_android(pos);
                    control.toque(MOVER, 0, x, y, w, h);
                    self.tocando = Some(pos);
                }
                Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, .. } if self.tocando.is_some() => {
                    let (x, y) = a_android(pos);
                    control.toque(SOLTAR, 0, x, y, w, h);
                    self.tocando = None;
                }
                Event::PointerGone => {
                    if let Some(pos) = self.tocando.take() {
                        let (x, y) = a_android(pos);
                        control.toque(SOLTAR, 0, x, y, w, h);
                    }
                }
                Event::MouseWheel { unit, delta, .. } if !bloqueado => {
                    let Some(pos) = puntero.filter(|p| rect.contains(*p)) else { continue };
                    let factor = match unit {
                        MouseWheelUnit::Line => 1.0,
                        MouseWheelUnit::Point => 1.0 / 40.0,
                        MouseWheelUnit::Page => 3.0,
                    };
                    let (x, y) = a_android(pos);
                    control.scroll(x, y, w, h, delta.x * factor, delta.y * factor);
                }
                Event::Key { key, pressed, repeat, modifiers, .. } => {
                    if key == egui::Key::F11 {
                        if pressed {
                            alternar_pantalla_completa(ctx);
                        }
                        continue;
                    }
                    if activo {
                        if let Some(nombre) = mapeo::nombre_tecla(key) {
                            if self.motor().tecla(&nombre, pressed, repeat, &control, ancho, alto) {
                                continue;
                            }
                        }
                    }
                    let Some(codigo) = android::tecla_android(key) else { continue };
                    let mut meta = 0;
                    if modifiers.shift {
                        meta |= META_SHIFT;
                    }
                    if modifiers.alt {
                        meta |= META_ALT;
                    }
                    if modifiers.ctrl {
                        meta |= META_CTRL;
                    }
                    control.tecla(if pressed { BAJAR } else { SOLTAR }, codigo, meta);
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for Zykro {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        // Las teclas son para Android: que ningún botón se quede con el foco del teclado.
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        self.actualizar_textura(ctx);
        self.contar_fps();

        let pantalla_completa = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let en_borde = cambiar_tamano(ctx);
        self.actualizar_mapeo(ctx);
        if !pantalla_completa {
            self.barra_titulo(ui, en_borde);
            if self.editor.abierto {
                self.panel_editor(ui);
            } else if self.lateral {
                self.barra_lateral(ui);
            }
        }
        self.pantalla(ui, en_borde);
        let salida = {
            let mut motor = self.motor.lock().unwrap();
            self.menu.mostrar(ctx, motor.mapa.as_mut())
        };
        match salida {
            menu::Salida::Nada => {}
            menu::Salida::Cerrar => self.cerrar_menu(),
            menu::Salida::Editor => {
                self.cerrar_menu();
                self.abrir_editor();
            }
        }
        if let Some(nueva) = self.ajustes.mostrar(ctx) {
            self.aplicar_config(nueva);
        }
    }

    fn on_exit(&mut self) {
        self.cerrar_menu();
        if self.config().apagar_al_cerrar {
            android::apagar();
        }
    }
}

/// Tema oscuro para los controles (casillas, listas, deslizadores) a juego con la ventana.
fn visuales() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = FONDO;
    v.window_fill = FONDO;
    v.extreme_bg_color = FONDO_OSCURO;
    v.widgets.inactive.bg_fill = FONDO_CONTROL;
    v.widgets.inactive.weak_bg_fill = FONDO_CONTROL;
    v.widgets.hovered.bg_fill = FONDO_HOVER;
    v.widgets.hovered.weak_bg_fill = FONDO_HOVER;
    v.selection.bg_fill = ACENTO.gamma_multiply(0.85);
    v.selection.stroke = Stroke::new(1.0, FONDO);
    v.slider_trailing_fill = true;
    v
}

/// Botón cuadrado con un ícono, que se ilumina al pasar el mouse.
fn icono(ui: &mut egui::Ui, glifo: &str, ayuda: &str, tam: Vec2, fondo_hover: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(tam, Sense::click());
    let encima = resp.hovered();
    if encima {
        ui.painter().rect_filled(rect.shrink(1.0), 3.0, fondo_hover);
    }
    let color = if encima { Color32::WHITE } else { ICONO };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glifo, FontId::proportional(15.0), color);
    resp.on_hover_text(ayuda)
}

enum Nav {
    Atras,
    Inicio,
    Recientes,
}

/// Botones de navegación de Android dibujados a mano: triángulo, círculo y cuadrado.
fn boton_nav(ui: &mut egui::Ui, forma: Nav, tam: Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(tam, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink(2.0), 3.0, FONDO_HOVER);
    }
    let color = if resp.hovered() { Color32::WHITE } else { ICONO };
    let trazo = Stroke::new(1.4, color);
    let c = rect.center();
    let r = 6.0;
    let (forma, ayuda) = match forma {
        Nav::Atras => (
            Shape::closed_line(vec![c + Vec2::new(-r, 0.0), c + Vec2::new(r * 0.8, -r), c + Vec2::new(r * 0.8, r)], trazo),
            "Atrás",
        ),
        Nav::Inicio => (Shape::circle_stroke(c, r, trazo), "Inicio"),
        Nav::Recientes => (Shape::rect_stroke(Rect::from_center_size(c, Vec2::splat(r * 1.7)), 1.0, trazo, egui::StrokeKind::Middle), "Recientes"),
    };
    ui.painter().add(forma);
    resp.on_hover_text(ayuda)
}

/// Logo propio: círculo amarillo con una "Z".
fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 8.0, ACENTO);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Z", FontId::proportional(11.5), FONDO);
}

fn alternar_pantalla_completa(ctx: &egui::Context) {
    let actual = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!actual));
}

/// La ventana no tiene bordes del sistema: las orillas cambian el tamaño a mano.
/// Devuelve `true` si el mouse está sobre una orilla.
fn cambiar_tamano(ctx: &egui::Context) -> bool {
    let (maximizado, completa) = ctx.input(|i| {
        (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false))
    });
    if maximizado || completa {
        return false;
    }
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return false };
    let r = ctx.content_rect();
    let (izq, der) = (p.x < r.min.x + BORDE, p.x > r.max.x - BORDE);
    let (arr, aba) = (p.y < r.min.y + BORDE, p.y > r.max.y - BORDE);
    let (dir, cursor) = match (izq, der, arr, aba) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::ResizeNorthWest),
        (_, true, true, _) => (ResizeDirection::NorthEast, CursorIcon::ResizeNorthEast),
        (true, _, _, true) => (ResizeDirection::SouthWest, CursorIcon::ResizeSouthWest),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::ResizeSouthEast),
        (true, ..) => (ResizeDirection::West, CursorIcon::ResizeWest),
        (_, true, ..) => (ResizeDirection::East, CursorIcon::ResizeEast),
        (_, _, true, _) => (ResizeDirection::North, CursorIcon::ResizeNorth),
        (_, _, _, true) => (ResizeDirection::South, CursorIcon::ResizeSouth),
        _ => return false,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
    true
}

fn carpeta_capturas() -> PathBuf {
    let imagenes = Command::new("xdg-user-dir")
        .arg("PICTURES")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Pictures"));
    imagenes.join(NOMBRE)
}

fn guardar_png(ruta: &PathBuf, ancho: u32, alto: u32, rgba: &[u8]) -> Result<(), png::EncodingError> {
    let archivo = std::io::BufWriter::new(std::fs::File::create(ruta)?);
    let mut codificador = png::Encoder::new(archivo, ancho, alto);
    codificador.set_color(png::ColorType::Rgba);
    codificador.set_depth(png::BitDepth::Eight);
    codificador.write_header()?.write_image_data(rgba)
}
