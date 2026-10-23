//! Cuándo dibujar el siguiente cuadro.
//!
//! Con vsync, Mesa hace esperar a cada cuadro hasta que KWin use el anterior. Cada vez que algo
//! además del video redibujaba la ventana (una tecla, un clic, la repetición de W al caminar), el
//! cuadro de video siguiente pagaba esa espera y salía un refresco tarde: el juego se veía a tirones.
//! Sin vsync ningún cuadro espera, pero hay que saber por cuenta propia cuándo KWin ya tomó lo último
//! que se le mandó, para no tapar un cuadro de video que todavía no salió en pantalla y para no
//! redibujar mil veces por segundo al mover el mouse. Eso lo dice el aviso `wl_surface.frame` que se
//! pide con cada cuadro.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};

/// Si KWin no avisa en este tiempo (ventana minimizada o tapada), el cuadro se da por visto.
const ESPERA_MAX: Duration = Duration::from_millis(40);

struct Avisos {
    /// Último cuadro que KWin ya usó.
    visto: u64,
    medida: Option<Medida>,
}

/// Con `ZYKRO_RITMO=1`, cada segundo se anota en el registro cuántos cuadros de video mostró KWin,
/// cuántos se taparon antes de salir (dos con la misma hora de KWin) y cuántos refrescos repitieron el
/// cuadro anterior.
struct Medida {
    desde: Instant,
    /// Hora de KWin (ms) del último cuadro de video mostrado.
    ultimo: Option<u32>,
    mostrados: u32,
    tapados: u32,
    repetidos: u32,
    esperas: u32,
    /// Desde que llega cada cuadro de Android hasta que se entrega a KWin.
    retraso_total: Duration,
    retraso_max: Duration,
    entregados: u32,
    /// Desde que se entrega cada cuadro de video hasta el aviso de KWin.
    kwin_total: Duration,
    avisos: u32,
    /// Veces que un cuadro no esperó para salir del atraso (ver `actualizar_textura`).
    saltos: u32,
    rafagas: u32,
    /// Intervalos entre cuadros de Android (llegada a llegada).
    llegada_antes: Option<Instant>,
    intervalo_min: Duration,
    intervalo_max: Duration,
    irregulares: u32,
    /// Lo mismo contando desde el aviso del servidor, y la espera a que Android termine de dibujar.
    recibido_antes: Option<Instant>,
    irregulares_servidor: u32,
    dibujo_total: Duration,
    dibujo_max: Duration,
}

impl Medida {
    fn nueva(ultimo: Option<u32>) -> Medida {
        Medida {
            desde: Instant::now(),
            ultimo,
            mostrados: 0,
            tapados: 0,
            repetidos: 0,
            esperas: 0,
            retraso_total: Duration::ZERO,
            retraso_max: Duration::ZERO,
            entregados: 0,
            kwin_total: Duration::ZERO,
            avisos: 0,
            saltos: 0,
            rafagas: 0,
            llegada_antes: None,
            intervalo_min: Duration::MAX,
            intervalo_max: Duration::ZERO,
            irregulares: 0,
            recibido_antes: None,
            irregulares_servidor: 0,
            dibujo_total: Duration::ZERO,
            dibujo_max: Duration::ZERO,
        }
    }

    fn anotar(&mut self, hora: u32) {
        match self.ultimo {
            Some(u) if u == hora => self.tapados += 1,
            Some(u) => {
                // 16.7 ms por refresco: un salto de dos o más refrescos repitió el cuadro anterior.
                let refrescos = (hora.wrapping_sub(u) * 3 + 25) / 50;
                self.repetidos += refrescos.saturating_sub(1);
            }
            None => {}
        }
        if self.ultimo != Some(hora) {
            self.mostrados += 1;
        }
        self.ultimo = Some(hora);
        if self.desde.elapsed() >= Duration::from_secs(1) {
            let medio = self.retraso_total.as_secs_f64() * 1000.0 / self.entregados.max(1) as f64;
            let kwin = self.kwin_total.as_secs_f64() * 1000.0 / self.avisos.max(1) as f64;
            eprintln!(
                "[ritmo] {} cuadros de video, {} tapados, {} repetidos, {} esperas, {} saltos, {} ráfagas, retraso {:.1} ms (máx. {:.1}), aviso de KWin a los {:.1} ms",
                self.mostrados,
                self.tapados,
                self.repetidos,
                self.esperas,
                self.saltos,
                self.rafagas,
                medio,
                self.retraso_max.as_secs_f64() * 1000.0,
                kwin
            );
            eprintln!(
                "[ritmo]   llegadas cada {:.1}–{:.1} ms, {} fuera de 14–19.5 ms ({} desde el servidor), espera del dibujo de Android {:.1} ms (máx. {:.1})",
                self.intervalo_min.as_secs_f64() * 1000.0,
                self.intervalo_max.as_secs_f64() * 1000.0,
                self.irregulares,
                self.irregulares_servidor,
                self.dibujo_total.as_secs_f64() * 1000.0 / self.entregados.max(1) as f64,
                self.dibujo_max.as_secs_f64() * 1000.0
            );
            let (antes, recibido) = (self.llegada_antes, self.recibido_antes);
            *self = Medida::nueva(self.ultimo);
            self.llegada_antes = antes;
            self.recibido_antes = recibido;
        }
    }
}

/// Datos de cada aviso: el número de cuadro y, si traía video nuevo, cuándo se entregó.
impl Dispatch<WlCallback, (u64, Option<Instant>)> for Avisos {
    fn event(
        avisos: &mut Self,
        _: &WlCallback,
        evento: wl_callback::Event,
        &(serie, video): &(u64, Option<Instant>),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data } = evento {
            avisos.visto = avisos.visto.max(serie);
            if let (Some(m), Some(entregado)) = (avisos.medida.as_mut(), video) {
                m.kwin_total += entregado.elapsed();
                m.avisos += 1;
                m.anotar(callback_data);
            }
        }
    }
}

struct Ritmo {
    cola: EventQueue<Avisos>,
    superficie: WlSurface,
    avisos: Avisos,
    /// Último cuadro mandado y cuándo; lo mismo para el último que traía video nuevo.
    serie: u64,
    enviado: Instant,
    serie_video: u64,
    enviado_video: Instant,
}

impl Ritmo {
    fn crear(frame: &eframe::Frame) -> Option<Ritmo> {
        let RawDisplayHandle::Wayland(pantalla) = frame.display_handle().ok()?.as_raw() else { return None };
        let RawWindowHandle::Wayland(ventana) = frame.window_handle().ok()?.as_raw() else { return None };
        // La conexión y la superficie son las de winit: aquí solo se agrega una cola de eventos propia.
        let conexion = Connection::from_backend(unsafe { Backend::from_foreign_display(pantalla.display.as_ptr().cast()) });
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), ventana.surface.as_ptr().cast()) }.ok()?;
        let superficie = WlSurface::from_id(&conexion, id).ok()?;
        let ahora = Instant::now();
        Some(Ritmo {
            cola: conexion.new_event_queue(),
            superficie,
            avisos: Avisos {
                visto: 0,
                medida: std::env::var_os("ZYKRO_RITMO").map(|_| Medida::nueva(None)),
            },
            serie: 0,
            enviado: ahora,
            serie_video: 0,
            enviado_video: ahora,
        })
    }

    fn visto(&mut self, serie: u64, enviado: Instant) -> bool {
        // winit lee el socket de Wayland; los avisos para esta cola quedan esperando hasta aquí.
        let _ = self.cola.dispatch_pending(&mut self.avisos);
        self.avisos.visto >= serie || enviado.elapsed() >= ESPERA_MAX
    }
}

thread_local! {
    static RITMO: RefCell<Option<Ritmo>> = const { RefCell::new(None) };
    static INTENTADO: Cell<bool> = const { Cell::new(false) };
    static APLAZADO: Cell<bool> = const { Cell::new(false) };
    static ESPERAR_VIDEO: Cell<bool> = const { Cell::new(true) };
    static RAFAGA: Cell<bool> = const { Cell::new(false) };
    static FLUIDA: Cell<bool> = const { Cell::new(false) };
}

/// Empieza a seguir los cuadros de la ventana (solo la primera vez). Devuelve `false` si no se
/// puede (no es Wayland): entonces se deja el vsync.
pub fn iniciar(frame: &eframe::Frame) -> bool {
    if !INTENTADO.replace(true) {
        let ritmo = Ritmo::crear(frame);
        if ritmo.is_none() {
            eprintln!("[ritmo] sin avisos de Wayland: se queda el vsync");
        }
        RITMO.set(ritmo);
    }
    RITMO.with_borrow(Option::is_some)
}

/// Pide el aviso de KWin para el cuadro que se está dibujando. Va en cada cuadro, antes de entregarlo;
/// `tiempos` dice, si trae un cuadro nuevo de Android, cuándo lo avisó el servidor y cuándo quedó listo.
pub fn enviar(tiempos: Option<(Instant, Instant)>) {
    RITMO.with_borrow_mut(|r| {
        let Some(r) = r else { return };
        let video = tiempos.is_some();
        if let (Some(m), Some((recibido, llegada))) = (r.avisos.medida.as_mut(), tiempos) {
            let dibujo = llegada.saturating_duration_since(recibido);
            m.dibujo_total += dibujo;
            m.dibujo_max = m.dibujo_max.max(dibujo);
            if let Some(antes) = m.recibido_antes.replace(recibido) {
                let d = recibido.saturating_duration_since(antes);
                if d < Duration::from_millis(14) || d > Duration::from_micros(19500) {
                    m.irregulares_servidor += 1;
                }
            }
            let retraso = llegada.elapsed();
            m.retraso_total += retraso;
            m.retraso_max = m.retraso_max.max(retraso);
            m.entregados += 1;
            if let Some(antes) = m.llegada_antes.replace(llegada) {
                let d = llegada.saturating_duration_since(antes);
                m.intervalo_min = m.intervalo_min.min(d);
                m.intervalo_max = m.intervalo_max.max(d);
                if d < Duration::from_millis(14) || d > Duration::from_micros(19500) {
                    m.irregulares += 1;
                }
            }
        }
        r.serie += 1;
        r.enviado = Instant::now();
        r.superficie.frame(&r.cola.handle(), (r.serie, video.then_some(r.enviado)));
        if video {
            r.serie_video = r.serie;
            r.enviado_video = r.enviado;
        }
    });
}

/// KWin ya tomó el último cuadro.
pub fn libre() -> bool {
    RITMO.with_borrow_mut(|r| r.as_mut().is_none_or(|r| r.visto(r.serie, r.enviado)))
}

/// El siguiente cuadro de video puede salir ya: no tiene que esperar (ver `fluida` y `rafaga`), o KWin
/// ya tomó el anterior (así no lo tapa).
pub fn video_libre() -> bool {
    !(FLUIDA.get() || RAFAGA.get()) || !ESPERAR_VIDEO.get() || RITMO.with_borrow_mut(|r| r.as_mut().is_none_or(|r| r.visto(r.serie_video, r.enviado_video)))
}

/// Imagen fluida (ajuste): cada cuadro de video espera a que KWin tome el anterior, uno por refresco.
/// Free Fire entrega los cuadros cada 6 a 28 ms (lo que tarda su GPU en cada uno, no ZYKRO): sin
/// esperar salen apenas llegan pero 3–5 por segundo se tapan o se repiten; esperando casi no hay
/// tirones, pero la imagen va 4–8 ms más atrasada en promedio (medido en el entrenamiento).
pub fn fluida(si: bool) {
    FLUIDA.set(si);
}

/// Llegaron dos cuadros de Android juntos (quedó otro en la cola al mostrar uno): el siguiente
/// espera a que KWin tome el que se acaba de mostrar, aunque la imagen fluida esté apagada.
pub fn rafaga(hay: bool) {
    if hay && !RAFAGA.get() {
        RITMO.with_borrow_mut(|r| {
            if let Some(m) = r.as_mut().and_then(|r| r.avisos.medida.as_mut()) {
                m.rafagas += 1;
            }
        });
    }
    RAFAGA.set(hay);
}

/// Con `false`, el siguiente cuadro de video se dibuja apenas llega aunque tape al anterior.
pub fn esperar_video(esperar: bool) {
    if ESPERAR_VIDEO.replace(esperar) && !esperar {
        RITMO.with_borrow_mut(|r| {
            if let Some(m) = r.as_mut().and_then(|r| r.avisos.medida.as_mut()) {
                m.saltos += 1;
            }
        });
    }
}

/// Algo quedó sin dibujar hasta que KWin avise.
pub fn aplazar() {
    if !APLAZADO.replace(true) {
        RITMO.with_borrow_mut(|r| {
            if let Some(m) = r.as_mut().and_then(|r| r.avisos.medida.as_mut()) {
                m.esperas += 1;
            }
        });
    }
}

/// Devuelve si había algo aplazado, y lo quita.
pub fn tomar_aplazado() -> bool {
    APLAZADO.replace(false)
}

pub fn aplazado() -> bool {
    APLAZADO.get()
}

/// Hasta cuándo esperar el aviso de KWin, si hay algo aplazado.
pub fn limite() -> Option<Instant> {
    if !APLAZADO.get() {
        return None;
    }
    RITMO.with_borrow(|r| r.as_ref().map(|r| r.enviado + ESPERA_MAX))
}
