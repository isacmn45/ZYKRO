//! ZYKRO: ventana propia para el Android de SwagPlayer (redroid + swag-rust-server).
//! Diseño inspirado en LDPlayer: barra de título oscura y barra lateral delgada con herramientas.

mod ajustes;
mod ajustes_android;
mod android;
mod audio;
mod carga;
mod cierre;
mod config;
mod control;
mod curva;
mod editor;
mod mapeo;
mod menu;
mod raton;
mod recientes;
mod rendimiento;
mod ritmo;
mod rueda;
mod sensores;
mod textura;
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
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

const NOMBRE: &str = "ZYKRO";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const ALTO_TITULO: f32 = 30.0;
const ANCHO_LATERAL: f32 = 34.0;
/// Refrescos seguidos con un cuadro de retraso antes de saltar uno (medio segundo a 60 Hz).
const MAX_ATRASADOS: u32 = 30;
const BORDE: f32 = 5.0; // zona en la orilla para cambiar el tamaño
const TAMANO_MINIMO: Vec2 = Vec2::new(480.0, 300.0);
/// Tras cambiar el tamaño de la ventana, cuánto tiene que quedarse quieto antes de ajustarlo.
const QUIETO: Duration = Duration::from_millis(200);
/// Cuánto se puede estirar la imagen para llenar la zona sin dejar bordes negros (maximizada no
/// se puede cambiar el tamaño de la ventana). Más que esto ya se nota deformada.
const ESTIRAR_MAX: f32 = 0.08;

// Paleta morada, a juego con el ícono (desktop/zykro.svg).
const FONDO_OSCURO: Color32 = Color32::from_rgb(0x11, 0x0e, 0x1b);
const FONDO_CONTROL: Color32 = Color32::from_rgb(0x28, 0x22, 0x39);
const FONDO: Color32 = Color32::from_rgb(0x1b, 0x17, 0x29);
const FONDO_HOVER: Color32 = Color32::from_rgb(0x34, 0x2c, 0x4b);
const CERRAR_HOVER: Color32 = Color32::from_rgb(0xe0, 0x3e, 0x3e);
const ICONO: Color32 = Color32::from_rgb(0xcb, 0xc5, 0xdc);
const TENUE: Color32 = Color32::from_rgb(0x8a, 0x84, 0xa3);
const TEXTO: Color32 = Color32::from_rgb(0xee, 0xeb, 0xf6);
const ACENTO: Color32 = Color32::from_rgb(0xa8, 0x55, 0xf7);
/// Avisos que deben verse cálidos aunque la app sea morada (temperatura alta).
const AVISO: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);
/// Argumento con el que ZYKRO se vuelve a abrir para recrear Android con la configuración guardada.
const APLICAR_ANDROID: &str = "--aplicar-android";
/// Ícono de la app, generado desde desktop/zykro.svg con `rsvg-convert -w 256 -h 256`.
const ICONO_PNG: &[u8] = include_bytes!("../assets/icono.png");

fn main() -> eframe::Result {
    registrar_en_archivo();
    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(NOMBRE)
            .with_app_id("zykro")
            .with_decorations(false)
            .with_inner_size([1280.0 + ANCHO_LATERAL, 720.0 + ALTO_TITULO])
            .with_min_inner_size(TAMANO_MINIMO)
            .with_icon({
                let (rgba, width, height) = decodificar_png(ICONO_PNG);
                egui::IconData { rgba, width, height }
            }),
        ..Default::default()
    };
    let bucle = EventLoop::<eframe::UserEvent>::with_user_event().build().map_err(eframe::Error::WinitEventLoop)?;
    let app = eframe::create_native(NOMBRE, opciones, Box::new(|cc| Ok(Box::new(Zykro::new(cc)))), &bucle);
    let mut ventana = Ventana { app, id: None, urgente: false };
    bucle.run_app(&mut ventana).map_err(eframe::Error::WinitEventLoop)
}

/// El bucle de eframe, con dos cambios:
/// - Sin el movimiento crudo del mouse mientras raton.rs manda la puntería directo a Android: con él,
///   egui se redibujaba con cada reporte (1000 por segundo con un mouse gamer). En Wayland winit no
///   deja apagar esos eventos.
/// - Los redibujos que no traen video ni una tecla o clic esperan a que KWin tome el cuadro anterior
///   (ver ritmo.rs): sin vsync, si no, la ventana se redibujaría sin parar al mover el mouse.
struct Ventana<'a> {
    app: eframe::EframeWinitApplication<'a>,
    id: Option<WindowId>,
    /// Llegó una tecla o un clic desde el último cuadro: se dibuja ya, sin esperar a KWin.
    urgente: bool,
}

/// Se puede dibujar ya: KWin tomó el último cuadro, o hay video nuevo y ya tomó el video anterior.
fn puede_dibujar() -> bool {
    ritmo::libre() || (video::NUEVO.load(Ordering::Relaxed) && ritmo::video_libre())
}

impl ApplicationHandler<eframe::UserEvent> for Ventana<'_> {
    fn resumed(&mut self, bucle: &ActiveEventLoop) {
        self.app.resumed(bucle);
    }

    fn window_event(&mut self, bucle: &ActiveEventLoop, id: WindowId, evento: WindowEvent) {
        self.id = Some(id);
        match &evento {
            WindowEvent::KeyboardInput { event, .. } if !event.repeat => self.urgente = true,
            WindowEvent::MouseInput { .. } => self.urgente = true,
            WindowEvent::Focused(enfocada) => {
                eprintln!("[ventana] {:.3} {}", segundos(), if *enfocada { "con foco" } else { "sin foco" });
                self.urgente = true;
            }
            WindowEvent::CursorEntered { .. } => eprintln!("[ventana] {:.3} entra el puntero", segundos()),
            WindowEvent::CursorLeft { .. } => eprintln!("[ventana] {:.3} sale el puntero", segundos()),
            WindowEvent::RedrawRequested => {
                if !self.urgente && !puede_dibujar() {
                    ritmo::aplazar();
                    return;
                }
                self.urgente = false;
                ritmo::tomar_aplazado();
            }
            _ => {}
        }
        self.app.window_event(bucle, id, evento);
    }

    fn new_events(&mut self, bucle: &ActiveEventLoop, causa: StartCause) {
        self.app.new_events(bucle, causa);
    }

    fn user_event(&mut self, bucle: &ActiveEventLoop, evento: eframe::UserEvent) {
        self.app.user_event(bucle, evento);
    }

    fn device_event(&mut self, bucle: &ActiveEventLoop, id: DeviceId, evento: DeviceEvent) {
        if matches!(evento, DeviceEvent::MouseMotion { .. }) && raton::DIRECTO.load(Ordering::Relaxed) {
            return;
        }
        self.app.device_event(bucle, id, evento);
    }

    fn about_to_wait(&mut self, bucle: &ActiveEventLoop) {
        if let Some(id) = self.id.filter(|_| ritmo::aplazado() && puede_dibujar()) {
            ritmo::tomar_aplazado();
            self.app.window_event(bucle, id, WindowEvent::RedrawRequested);
        }
        self.app.about_to_wait(bucle);
        // Despertar a más tardar cuando venza la espera del aviso de KWin.
        if let Some(limite) = ritmo::limite() {
            let flujo = match bucle.control_flow() {
                ControlFlow::WaitUntil(t) if t < limite => ControlFlow::WaitUntil(t),
                ControlFlow::Poll => ControlFlow::Poll,
                _ => ControlFlow::WaitUntil(limite),
            };
            bucle.set_control_flow(flujo);
        }
    }

    fn suspended(&mut self, bucle: &ActiveEventLoop) {
        self.app.suspended(bucle);
    }

    fn exiting(&mut self, bucle: &ActiveEventLoop) {
        self.app.exiting(bucle);
    }

    fn memory_warning(&mut self, bucle: &ActiveEventLoop) {
        self.app.memory_warning(bucle);
    }
}

/// Segundos desde que abrió ZYKRO, para el registro.
fn segundos() -> f64 {
    static INICIO: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    INICIO.get_or_init(Instant::now).elapsed().as_secs_f64()
}

struct Zykro {
    /// Ícono de la app (barra de título y pantalla de carga).
    logo: egui::TextureHandle,
    estado: android::EstadoCompartido,
    video: video::Compartido,
    pantalla: textura::Pantalla,
    /// Último cuadro mostrado (para las capturas de pantalla).
    cuadro: Option<video::Cuadro>,
    /// Refrescos seguidos mostrando el cuadro con uno de retraso (ver `actualizar_textura`).
    atrasados: u32,
    lateral: bool,
    tocando: Option<Pos2>,
    /// Scroll con la rueda o el touchpad, imitado arrastrando un dedo.
    rueda: rueda::Rueda,
    fps: u32,
    cuadros_seg: u32,
    inicio_seg: Instant,
    ajustes: ajustes::Ajustes,
    /// Ajustes de Android de ZYKRO (el engrane de la pantalla de inicio).
    ajustes_android: ajustes_android::AjustesAndroid,
    motor: Arc<Mutex<mapeo::Motor>>,
    /// El hilo de raton.rs manda la puntería directo (la ventana no procesa el mouse).
    raton_directo: Arc<AtomicBool>,
    raton_listo: Arc<AtomicBool>,
    editor: editor::Editor,
    /// Menú rápido del botón de controles (sensibilidad), como el de BlueStacks.
    menu: menu::Menu,
    /// Apps abiertas (el botón Recientes), propias porque Android no tiene vista de recientes.
    recientes: recientes::Recientes,
    /// Temperatura de CPU y GPU (la lee un hilo cada 2 s).
    temperaturas: sensores::Compartidas,
    /// Se está mostrando la confirmación para cerrar ZYKRO.
    confirmar_cierre: bool,
    /// El usuario ya confirmó: el próximo pedido de cierre se deja pasar.
    cierre_confirmado: bool,
    /// Al cerrar, volver a abrir ZYKRO para aplicar la configuración de Android.
    reabrir: bool,
    /// El mapeo está convirtiendo teclas en toques en este momento.
    mapeo_activo: bool,
    cursor_bloqueado: bool,
    apuntando_antes: bool,
    mods_previos: egui::Modifiers,
    revision_app: Instant,
    proporcion: Proporcion,
}

/// Seguimiento del tamaño de la ventana para ajustarlo a la proporción de Android.
#[derive(Default)]
struct Proporcion {
    /// Último tamaño visto y el que tenía antes de empezar a cambiar.
    visto: Vec2,
    antes: Vec2,
    /// Desde cuándo está cambiando (None: quieta).
    cambiando: Option<Instant>,
    /// Orilla que tomó el usuario para cambiar el tamaño.
    borde: Option<ResizeDirection>,
    /// Resolución de Android con la que se ajustó.
    android: (u32, u32),
    /// Tamaño justo que se pidió: hasta que llegue (o pase el momento) la imagen sigue estirada.
    pedido: Option<(Vec2, Instant)>,
    /// Orilla que ZYKRO está arrastrando por su cuenta.
    arrastre: Option<Arrastre>,
}

/// Cambio de tamaño desde la orilla derecha, la de abajo o la esquina de abajo a la derecha. Lo
/// hace ZYKRO y no KWin, que en Wayland no deja mantener la proporción mientras se arrastra: así la
/// ventana tiene siempre la forma de la imagen. Arriba y a la izquierda hay que mover la ventana, y
/// eso solo lo puede hacer KWin.
#[derive(Clone, Copy)]
struct Arrastre {
    dir: ResizeDirection,
    cursor: CursorIcon,
    /// Dónde se tomó la orilla y el tamaño que tenía la ventana.
    puntero: Pos2,
    tam: Vec2,
}

impl Proporcion {
    /// La ventana está cambiando de tamaño y todavía no tiene la proporción de Android.
    fn ajustando(&self) -> bool {
        self.cambiando.is_some() || self.pedido.is_some_and(|(_, hasta)| Instant::now() < hasta)
    }
}

impl Zykro {
    fn new(cc: &eframe::CreationContext) -> Self {
        let mut fuentes = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fuentes, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fuentes);
        cc.egui_ctx.set_visuals(visuales());

        // Reabierto tras cambiar la configuración de Android: lo primero es recrearlo con ella.
        let aplicar = std::env::args().any(|a| a == APLICAR_ANDROID);
        let config = config::Config::cargar(aplicar);
        let estado: android::EstadoCompartido = Arc::new(Mutex::new(android::Estado {
            reinicio: aplicar.then(|| config.android.clone()),
            config,
            ..Default::default()
        }));
        let video: video::Compartido = Arc::default();
        android::iniciar(cc.egui_ctx.clone(), estado.clone(), video.clone());
        android::vigilar_texto(cc.egui_ctx.clone(), estado.clone());
        android::vigilar_peticiones(cc.egui_ctx.clone(), estado.clone());
        audio::iniciar();
        aplicar_modo(&estado, estado.lock().unwrap().config.modo, false);
        let motor = Arc::new(Mutex::new(mapeo::Motor::new()));
        let raton_directo = Arc::new(AtomicBool::new(false));
        let raton_listo = Arc::new(AtomicBool::new(false));
        raton::iniciar(motor.clone(), raton_directo.clone(), raton_listo.clone());
        let logo = {
            let (rgba, w, h) = decodificar_png(ICONO_PNG);
            let imagen = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            cc.egui_ctx.load_texture("logo", imagen, TextureOptions::LINEAR)
        };
        Zykro {
            logo,
            estado,
            video,
            pantalla: textura::Pantalla::new(),
            cuadro: None,
            atrasados: 0,
            lateral: true,
            tocando: None,
            rueda: rueda::Rueda::default(),
            fps: 0,
            cuadros_seg: 0,
            inicio_seg: Instant::now(),
            ajustes: ajustes::Ajustes::new(),
            ajustes_android: ajustes_android::AjustesAndroid::new(),
            motor,
            raton_directo,
            raton_listo,
            editor: editor::Editor::new(),
            menu: menu::Menu::default(),
            recientes: recientes::Recientes::default(),
            temperaturas: sensores::iniciar(),
            confirmar_cierre: false,
            cierre_confirmado: false,
            reabrir: false,
            mapeo_activo: false,
            cursor_bloqueado: false,
            apuntando_antes: false,
            mods_previos: egui::Modifiers::NONE,
            revision_app: Instant::now() - Duration::from_secs(5),
            proporcion: Proporcion::default(),
        }
    }

    fn config(&self) -> config::Config {
        self.estado.lock().unwrap().config.clone()
    }

    fn abrir_ajustes(&mut self) {
        let actual = self.config();
        self.ajustes.abrir(&actual);
    }

    fn aplicar_config(&mut self, ctx: &egui::Context, nueva: config::Config) {
        nueva.guardar();
        let anterior = std::mem::replace(&mut self.estado.lock().unwrap().config, nueva.clone());
        if nueva.android != anterior.android {
            // ZYKRO se cierra y se vuelve a abrir, y la ventana nueva recrea Android: así la
            // pantalla de carga sale limpia y no sobre el último cuadro del juego.
            self.reabrir = true;
            self.cierre_confirmado = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        if nueva.modo != anterior.modo {
            aplicar_modo(&self.estado, nueva.modo, true);
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

    /// Texto de la temperatura (si está activada y hay sensores) y su color de aviso: ámbar desde
    /// 80 °C y rojo desde 90 °C (`None` = color normal).
    fn temperatura(&self) -> Option<(String, Option<Color32>)> {
        if !self.config().mostrar_temperatura {
            return None;
        }
        let t = *self.temperaturas.lock().unwrap();
        let grados = |v: f32| format!("{v:.0} °C");
        let texto = match (t.cpu, t.gpu) {
            (Some(c), _) if t.gpu_en_cpu => format!("CPU/GPU {}", grados(c)),
            (Some(c), Some(g)) => format!("CPU {}  ·  GPU {}", grados(c), grados(g)),
            (Some(c), None) => format!("CPU {}", grados(c)),
            (None, Some(g)) => format!("GPU {}", grados(g)),
            (None, None) => return None,
        };
        let maxima = t.cpu.unwrap_or(0.0).max(t.gpu.unwrap_or(0.0));
        let color = if maxima >= 90.0 { Some(CERRAR_HOVER) } else if maxima >= 80.0 { Some(AVISO) } else { None };
        Some((texto, color))
    }

    /// En pantalla completa no hay barra de título: FPS y temperatura van en una cajita pegada a la
    /// esquina de abajo a la izquierda, como en BlueStacks.
    fn indicadores(&self, ui: &egui::Ui, area: Rect) {
        let mut partes: Vec<(String, Color32)> = Vec::new();
        if self.config().mostrar_fps {
            partes.push((format!("{} FPS", self.fps), TEXTO));
        }
        if let Some((texto, color)) = self.temperatura() {
            partes.push((format!("{}  {texto}", ic::THERMOMETER_SIMPLE), color.unwrap_or(TEXTO)));
        }
        if partes.is_empty() {
            return;
        }
        let mut trabajo = egui::text::LayoutJob::default();
        for (i, (texto, color)) in partes.into_iter().enumerate() {
            let separador = if i > 0 { "  ·  " } else { "" };
            trabajo.append(separador, 0.0, egui::TextFormat { font_id: FontId::proportional(11.0), color: TENUE, ..Default::default() });
            trabajo.append(&texto, 0.0, egui::TextFormat { font_id: FontId::proportional(11.0), color, ..Default::default() });
        }
        let galeria = ui.painter().layout_job(trabajo);
        let margen = Vec2::new(5.0, 1.0);
        let tam = galeria.size() + margen * 2.0;
        // Sin separación del borde: solo se redondea la punta de adentro.
        let caja = Rect::from_min_size(Pos2::new(area.min.x, area.max.y - tam.y), tam);
        ui.painter().rect_filled(caja, egui::CornerRadius { ne: 4, ..Default::default() }, FONDO_OSCURO.gamma_multiply(0.7));
        ui.painter().galley(caja.min + margen, galeria, TEXTO);
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }

    /// Ajusta el ancho de la ventana a la imagen de Android para que no queden bordes negros a los
    /// lados: con la barra lateral plegada la imagen ocupa todo el ancho, y desplegada se suma la
    /// barra. Maximizada o en pantalla completa no se puede cambiar el tamaño.
    fn ajustar_ancho(&self, ctx: &egui::Context) {
        let (maximizada, completa) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
        if maximizada || completa {
            return;
        }
        // En Wayland no hay `inner_rect` (no se conoce la posición): el tamaño sale del contenido.
        let tam = self.tamano_justo(ctx.content_rect().size(), false, None);
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(tam));
    }

    /// Tamaño de ventana en el que la imagen de Android cabe justa, conservando el ancho
    /// (`por_ancho`) o el alto de `tam`, sin bajar del mínimo ni pasar de `maximo`.
    fn tamano_justo(&self, tam: Vec2, por_ancho: bool, maximo: Option<Vec2>) -> Vec2 {
        let (w, h) = self.dimensiones();
        let lateral = if self.lateral { ANCHO_LATERAL } else { 0.0 };
        let desde_alto = |alto: f32| Vec2::new(((alto - ALTO_TITULO) * w / h).round() + lateral, alto);
        let desde_ancho = |ancho: f32| Vec2::new(ancho, ((ancho - lateral) * h / w).round() + ALTO_TITULO);
        let mut t = if por_ancho { desde_ancho(tam.x) } else { desde_alto(tam.y) };
        if t.y < TAMANO_MINIMO.y {
            t = desde_alto(TAMANO_MINIMO.y);
        }
        if t.x < TAMANO_MINIMO.x {
            t = desde_ancho(TAMANO_MINIMO.x);
        }
        if let Some(m) = maximo {
            if t.y > m.y {
                t = desde_alto(m.y);
            }
            if t.x > m.x {
                t = desde_ancho(m.x);
            }
        }
        t
    }

    /// Cuando el usuario cambia el tamaño de la ventana (o KWin la acomoda), la ajusta a la
    /// proporción de Android para que no queden bordes negros. Espera a que el tamaño se quede
    /// quieto: mientras se arrastra la orilla, el tamaño lo pone KWin. Si se arrastró un lado se
    /// respeta el ancho elegido; arriba o abajo, el alto; una esquina, lo que más cambió.
    fn ajustar_proporcion(&mut self, ctx: &egui::Context) {
        let (maximizada, completa, monitor) = ctx.input(|i| {
            let v = i.viewport();
            (v.maximized.unwrap_or(false), v.fullscreen.unwrap_or(false), v.monitor_size)
        });
        let tam = ctx.content_rect().size();
        let android = self.estado.lock().unwrap().pantalla.unwrap_or((1280, 720));
        let p = &mut self.proporcion;
        if p.arrastre.is_some() {
            // Lo ajusta el arrastre mismo.
            p.visto = tam;
            p.cambiando = None;
            return;
        }
        if maximizada || completa {
            *p = Proporcion { visto: tam, android, ..Default::default() };
            return;
        }
        if tam != p.visto || android != p.android {
            if p.pedido.take().is_some_and(|(pedido, _)| (pedido - tam).length() < 2.0) {
                // Llegó el tamaño que se pidió.
                p.visto = tam;
                return;
            }
            if p.cambiando.is_none() {
                p.antes = p.visto;
            }
            p.visto = tam;
            p.android = android;
            p.cambiando = Some(Instant::now());
        }
        let Some(desde) = p.cambiando else { return };
        if desde.elapsed() < QUIETO {
            ctx.request_repaint_after(QUIETO - desde.elapsed());
            return;
        }
        p.cambiando = None;
        let cambio = |nuevo: f32, viejo: f32| if viejo > 0.0 { (nuevo - viejo).abs() / viejo } else { 0.0 };
        let por_ancho = match p.borde.take() {
            Some(ResizeDirection::East | ResizeDirection::West) => true,
            Some(ResizeDirection::North | ResizeDirection::South) => false,
            _ => cambio(tam.x, p.antes.x) > cambio(tam.y, p.antes.y),
        };
        let justo = self.tamano_justo(tam, por_ancho, monitor);
        if (justo - tam).length() >= 1.0 {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(justo));
            self.proporcion.pedido = Some((justo, Instant::now() + Duration::from_millis(500)));
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    /// Orillas de la ventana para cambiar el tamaño. Devuelve si el mouse está sobre una (o
    /// arrastrándola), para no mandar ese clic a Android.
    fn orillas(&mut self, ctx: &egui::Context) -> bool {
        if self.seguir_arrastre(ctx) {
            return true;
        }
        let (en_borde, tomada) = cambiar_tamano(ctx);
        if let Some((dir, cursor)) = tomada {
            let puntero = ctx.input(|i| i.pointer.press_origin());
            match (dir, puntero) {
                (ResizeDirection::East | ResizeDirection::South | ResizeDirection::SouthEast, Some(puntero)) => {
                    let tam = ctx.content_rect().size();
                    self.proporcion.arrastre = Some(Arrastre { dir, cursor, puntero, tam });
                }
                _ => {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                    self.proporcion.borde = Some(dir);
                }
            }
        }
        en_borde
    }

    /// Sigue el arrastre propio de una orilla: la ventana toma el tamaño justo en cada movimiento.
    fn seguir_arrastre(&mut self, ctx: &egui::Context) -> bool {
        let Some(a) = self.proporcion.arrastre else { return false };
        let (abajo, puntero, monitor) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos(), i.viewport().monitor_size));
        if !abajo {
            self.proporcion.arrastre = None;
            return false;
        }
        ctx.set_cursor_icon(a.cursor);
        if let Some(p) = puntero {
            let d = p - a.puntero;
            let crudo = a.tam
                + match a.dir {
                    ResizeDirection::East => Vec2::new(d.x, 0.0),
                    ResizeDirection::South => Vec2::new(0.0, d.y),
                    _ => d,
                };
            let por_ancho = match a.dir {
                ResizeDirection::East => true,
                ResizeDirection::South => false,
                // En la esquina manda el lado que deja la ventana más grande: así sigue al puntero
                // sin saltos.
                _ => self.tamano_justo(crudo, true, None).y >= self.tamano_justo(crudo, false, None).y,
            };
            let justo = self.tamano_justo(crudo, por_ancho, monitor);
            if (justo - ctx.content_rect().size()).length() >= 1.0 {
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(justo));
            }
        }
        true
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
                // Miniatura de la app al frente para el selector de apps abiertas.
                if let Some(cuadro) = &self.cuadro {
                    self.recientes.guardar_miniatura(&app, cuadro);
                }
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
            && !self.ajustes_android.abierto
            && !self.menu.abierto()
            && !self.confirmar_cierre
            && !self.recientes.abierto()
            && enfocada;
        if self.mapeo_activo && !activo {
            if let Some(c) = &control {
                motor.soltar_todo(c, w, h);
            }
        }
        self.mapeo_activo = activo;

        let bloquear = activo && motor.cursor_bloqueado();
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
        }
        drop(motor);
        self.bloquear_cursor(ctx, bloquear);
    }

    /// Bloquea el cursor (y manda el mouse directo a Android) o lo suelta.
    fn bloquear_cursor(&mut self, ctx: &egui::Context, bloquear: bool) {
        let directo = bloquear && self.raton_listo.load(Ordering::Relaxed);
        self.raton_directo.store(directo, Ordering::Relaxed);
        raton::DIRECTO.store(directo, Ordering::Relaxed);
        if bloquear != self.cursor_bloqueado {
            eprintln!("[ventana] {:.3} cursor {}", segundos(), if bloquear { "bloqueado" } else { "libre" });
            let modo = if bloquear { egui::CursorGrab::Locked } else { egui::CursorGrab::None };
            ctx.send_viewport_cmd(ViewportCommand::CursorGrab(modo));
            ctx.send_viewport_cmd(ViewportCommand::CursorVisible(!bloquear));
            self.cursor_bloqueado = bloquear;
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
    /// Devuelve si se mostró un cuadro nuevo.
    /// Muestra el siguiente cuadro de Android, si toca. Devuelve cuándo lo avisó el servidor y cuándo
    /// quedó listo.
    fn actualizar_textura(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) -> Option<(Instant, Instant)> {
        // Cada cuadro sale apenas llega. Si llegan dos juntos, el segundo espera a que KWin tome el
        // primero (si no, lo taparía antes de salir en pantalla). Si los cuadros salen tarde medio
        // segundo seguido (~17 ms cada uno), uno no espera (tapa al anterior) para no arrastrar ese
        // retraso.
        let atrasado = self.atrasados >= MAX_ATRASADOS;
        let libre = atrasado || ritmo::video_libre();
        let (cuadro, quedan) = {
            let mut cola = self.video.lock().unwrap();
            if !cola.is_empty() && !libre {
                ritmo::aplazar();
                return None;
            }
            if cola.len() > 1 && atrasado {
                cola.pop_front();
            }
            let cuadro = cola.pop_front();
            video::NUEVO.store(!cola.is_empty(), Ordering::Relaxed);
            (cuadro, !cola.is_empty())
        };
        let mut cuadro = cuadro?;
        // Sin esperar, un cuadro se dibuja ~0.2 ms después de llegar.
        let tarde = quedan || cuadro.llegada.elapsed() > Duration::from_millis(2);
        self.atrasados = if tarde && !atrasado { self.atrasados + 1 } else { 0 };
        ritmo::esperar_video(self.atrasados < MAX_ATRASADOS);
        ritmo::rafaga(quedan);
        if quedan {
            ritmo::aplazar();
        }
        self.pantalla.mostrar(ctx, frame, &mut cuadro);
        let tiempos = (cuadro.recibido, cuadro.llegada);
        self.cuadro = Some(cuadro);
        self.cuadros_seg += 1;
        Some(tiempos)
    }

    fn contar_fps(&mut self) {
        if self.inicio_seg.elapsed() >= Duration::from_secs(1) {
            self.fps = self.cuadros_seg;
            self.cuadros_seg = 0;
            self.inicio_seg = Instant::now();
        }
    }

    fn captura(&self) {
        let Some(c) = self.cuadro.clone() else { return };
        let estado = self.estado.clone();
        thread::spawn(move || {
            let imagen = c.imagen();
            let (ancho, alto, rgba) = (imagen.size[0] as u32, imagen.size[1] as u32, imagen.as_raw());
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
                    ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(20.0)));
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("{NOMBRE} {VERSION}")).color(TEXTO).size(12.5));
                    if self.config().mostrar_fps {
                        ui.add_space(6.0);
                        ui.label(RichText::new(format!("{} FPS", self.fps)).color(TENUE).size(11.0));
                        // Refrescar el contador aunque Android no esté dibujando.
                        ctx.request_repaint_after(Duration::from_secs(1));
                    }
                    if let Some((texto, color)) = self.temperatura() {
                        ui.add_space(6.0);
                        ui.label(RichText::new(format!("{}  {texto}", ic::THERMOMETER_SIMPLE)).color(color.unwrap_or(TENUE)).size(11.0));
                        ctx.request_repaint_after(Duration::from_secs(2));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let t = Vec2::new(34.0, ALTO_TITULO);
                        let lado = if self.lateral { ic::CARET_DOUBLE_RIGHT } else { ic::CARET_DOUBLE_LEFT };
                        if icono(ui, lado, "Barra lateral", t, FONDO_HOVER).clicked() {
                            self.lateral = !self.lateral;
                            self.ajustar_ancho(ctx);
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
                    if icono(ui, ic::CORNERS_OUT, "Pantalla completa (F11)", t, FONDO_HOVER).clicked() {
                        alternar_pantalla_completa(ctx);
                    }
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
                    if boton_nav(ui, Nav::Recientes, t).clicked() && self.control().is_some() {
                        // Antes de abrir, la miniatura más nueva de la app que está al frente.
                        if !self.recientes.abierto() {
                            if let (Some(app), Some(cuadro)) = (android::app_activa(), &self.cuadro) {
                                self.recientes.guardar_miniatura(&app, cuadro);
                            }
                        }
                        self.recientes.alternar(ctx);
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
                    sobre_video: self.cuadro.is_some(),
                    logo: self.logo.id(),
                };
                (w as f32, h as f32, carga, aviso)
            };
            // Si a la zona le falta poco para la proporción de Android (maximizada, la barra de
            // título y la lateral se comen un poco), la imagen se estira en vez de dejar bordes.
            // También mientras KWin cambia el tamaño (orillas de arriba o la izquierda), hasta que
            // la ventana se ajuste a su proporción: así se ve entera, sin bordes ni recortes.
            let desfase = (area.width() / area.height()) / (ancho / alto);
            let rect = if (desfase - 1.0).abs() <= ESTIRAR_MAX || self.proporcion.ajustando() {
                area
            } else {
                let escala = (area.width() / ancho).min(area.height() / alto);
                Rect::from_center_size(area.center(), Vec2::new(ancho * escala, alto * escala))
            };

            if let Some(id) = self.pantalla.id() {
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                ui.painter().image(id, rect, uv, Color32::WHITE);
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
            if self.cuadro.is_none() || !carga.mensaje.is_empty() {
                carga::dibujar(ui, area, &carga);
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                self.indicadores(ui, area);
            }
            if let Some(texto) = aviso {
                // Pegado a la esquina de abajo a la derecha, como la cajita de FPS en la otra.
                let galeria = ui.painter().layout_no_wrap(texto, FontId::proportional(13.0), TEXTO);
                let margen = Vec2::new(7.0, 3.0);
                let tam = galeria.size() + margen * 2.0;
                let caja = Rect::from_min_size(area.right_bottom() - tam, tam);
                ui.painter().rect_filled(caja, egui::CornerRadius { nw: 6, ..Default::default() }, FONDO.gamma_multiply(0.92));
                ui.painter().galley(caja.min + margen, galeria, TEXTO);
                ctx.request_repaint_after(Duration::from_millis(200));
            }

            if self.editor.abierto {
                self.teclas_editor(ctx);
            } else if !en_borde && !self.ajustes.abierto && !self.ajustes_android.abierto && !self.menu.abierto() && !self.confirmar_cierre && !self.recientes.abierto() {
                self.entrada(ctx, rect, ancho, alto);
            }
            if let Some(recientes::Accion::Inicio) = self.recientes.mostrar(ctx, area, rect, self.pantalla.id()) {
                self.pulsar(3);
            }
            // Fuera de `entrada` también: que el dedo del scroll nunca se quede abajo.
            if let Some(c) = self.control() {
                if let Some(falta) = self.rueda.revisar(&c, ancho, alto) {
                    ctx.request_repaint_after(falta);
                }
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
                Event::MouseWheel { unit, delta, .. } if !bloqueado && self.tocando.is_none() => {
                    let Some(pos) = puntero.filter(|p| rect.contains(*p)) else { continue };
                    // El dedo se mueve hacia donde va el contenido: rueda hacia abajo = dedo hacia arriba.
                    let d = match unit {
                        MouseWheelUnit::Point => delta * (ancho / rect.width()),
                        MouseWheelUnit::Line => delta * rueda::PASO_RUEDA,
                        MouseWheelUnit::Page => delta * alto * 0.8,
                    };
                    // Margen de Android para distinguir arrastre de toque: 8 dp, con un poco de sobra.
                    let umbral = 8.0 * self.config().android.dpi as f32 / 160.0 * 1.5;
                    let (x, y) = a_android(pos);
                    self.rueda.mover(&control, (x as f32, y as f32), (d.x, d.y), umbral, ancho, alto);
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
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        // Las teclas son para Android: que ningún botón se quede con el foco del teclado.
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        if ritmo::iniciar(frame) {
            textura::quitar_vsync();
        }
        let tiempos = self.actualizar_textura(ctx, frame);
        ritmo::enviar(tiempos);
        self.contar_fps();

        let pantalla_completa = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let en_borde = self.orillas(ctx);
        self.ajustar_proporcion(ctx);
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
        // Si una tecla acaba de prender o apagar la puntería, el cursor cambia ya, no en el próximo cuadro.
        let bloquear = self.mapeo_activo && self.motor().cursor_bloqueado();
        self.bloquear_cursor(ctx, bloquear);
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
        // Cerrar la ventana (X, Alt+F4, barra de tareas) primero pide confirmación.
        if ctx.input(|i| i.viewport().close_requested()) && !self.cierre_confirmado {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirmar_cierre = true;
        }
        if self.confirmar_cierre {
            match cierre::mostrar(ctx) {
                Some(cierre::Respuesta::Cerrar) => {
                    self.confirmar_cierre = false;
                    self.cierre_confirmado = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                Some(cierre::Respuesta::Cancelar) => self.confirmar_cierre = false,
                None => {}
            }
        }
        if std::mem::take(&mut self.estado.lock().unwrap().pedir_ajustes) {
            self.ajustes_android.abrir(ctx);
        }
        self.ajustes_android.mostrar(ctx, &self.estado);
        if let Some(nueva) = self.ajustes.mostrar(ctx) {
            self.aplicar_config(ctx, nueva);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cerrar_menu();
        rendimiento::restaurar();
        if self.reabrir {
            // La ventana nueva apaga Android antes de recrearlo.
            reabrir();
        } else {
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
/// PNG RGBA de 8 bits (el ícono) a píxeles RGBA.
fn decodificar_png(datos: &[u8]) -> (Vec<u8>, u32, u32) {
    let mut lector = png::Decoder::new(std::io::Cursor::new(datos)).read_info().expect("ícono PNG inválido");
    let mut rgba = vec![0; lector.output_buffer_size().expect("ícono PNG demasiado grande")];
    let info = lector.next_frame(&mut rgba).expect("ícono PNG inválido");
    rgba.truncate(info.buffer_size());
    (rgba, info.width, info.height)
}

/// Aplica un modo de rendimiento en segundo plano. `avisar_ok`: avisar también si salió bien.
fn aplicar_modo(estado: &android::EstadoCompartido, modo: rendimiento::Modo, avisar_ok: bool) {
    let estado = estado.clone();
    thread::spawn(move || {
        let fallos = rendimiento::aplicar(modo);
        if !fallos.is_empty() {
            android::avisar(&estado, format!("{}: {}", modo.nombre(), fallos.join("; ")));
        } else if avisar_ok {
            android::avisar(&estado, format!("Modo {} activado", modo.nombre().to_lowercase()));
        }
    });
}

fn alternar_pantalla_completa(ctx: &egui::Context) {
    let actual = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    eprintln!("[ventana] {:.3} pantalla completa: {}", segundos(), if actual { "sale" } else { "entra" });
    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!actual));
}

/// La ventana no tiene bordes del sistema: las orillas cambian el tamaño a mano.
/// Devuelve si el mouse está sobre una orilla y, si se acaba de tomar, cuál (y su cursor).
fn cambiar_tamano(ctx: &egui::Context) -> (bool, Option<(ResizeDirection, CursorIcon)>) {
    let (maximizado, completa) = ctx.input(|i| {
        (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false))
    });
    if maximizado || completa {
        return (false, None);
    }
    // Al presionar cuenta dónde se presionó: si el cuadro llega tarde, el puntero ya se movió.
    let (presionado, origen, encima) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.press_origin(), i.pointer.hover_pos()));
    let Some(p) = origen.filter(|_| presionado).or(encima) else { return (false, None) };
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
        _ => return (false, None),
    };
    ctx.set_cursor_icon(cursor);
    let tomada = ctx.input(|i| i.pointer.primary_pressed()).then_some((dir, cursor));
    (true, tomada)
}

/// Abierto desde el ícono, los mensajes de ZYKRO se perderían: van a ~/.cache/zykro/registro.log
/// (uno por sesión). Desde una terminal se siguen viendo ahí.
fn registrar_en_archivo() {
    use std::os::fd::AsRawFd;
    if unsafe { libc::isatty(2) } == 1 {
        return;
    }
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    let carpeta = base.join("zykro");
    let _ = std::fs::create_dir_all(&carpeta);
    if let Ok(archivo) = std::fs::File::create(carpeta.join("registro.log")) {
        unsafe { libc::dup2(archivo.as_raw_fd(), 2) };
    }
}

/// Abre otra vez ZYKRO para que recree Android con la configuración guardada.
fn reabrir() {
    use std::os::unix::process::CommandExt;
    // Si ZYKRO se reinstaló mientras corría, el ejecutable viejo figura como «(deleted)».
    let exe = std::env::current_exe().map(|e| PathBuf::from(e.to_string_lossy().trim_end_matches(" (deleted)")));
    let exe = exe.unwrap_or_else(|_| PathBuf::from("zykro"));
    if let Err(e) = Command::new(&exe).arg(APLICAR_ANDROID).process_group(0).spawn() {
        eprintln!("[zykro] no se pudo volver a abrir {}: {e}", exe.display());
    }
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
