//! Mandos (pestaña Gamepad): los de la laptop, por USB o Bluetooth, llegan a Android como mandos
//! propios y los juegos que aceptan mando los usan como en un teléfono. ZYKRO lee el mando (evdev) y
//! lo repite en uno virtual (uinput) con los mismos botones, ejes, fabricante y modelo (Android elige
//! la distribución de los botones por fabricante y modelo). Redroid no crea los nodos de entrada, así
//! que el del mando virtual se crea a mano dentro de Android, como el del teclado (`teclado-fisico.c`).
//! La vibración que pide Android (uinput la recibe como subidas de efectos) se pasa al mando real.
//!
//! Si el mapa del juego usa el mando (botones en sus controles, el joystick del D-pad o la pestaña
//! «Gamepad» de la puntería, como en BlueStacks), el mando es del mapeo: sus botones hacen lo de las
//! teclas y los joysticks mueven al personaje y la cámara. Mientras tanto Android ve el mando quieto.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::Vec2;

use crate::android;
use crate::mapeo::Motor;

/// Detectar mandos (Configuración › Gamepad).
pub static ACTIVO: AtomicBool = AtomicBool::new(true);
/// Pasar la vibración al mando (se aplica al conectarlo).
pub static VIBRACION: AtomicBool = AtomicBool::new(true);
/// Los mandos que están llegando a Android (sus nombres), para la pestaña Gamepad.
pub static CONECTADOS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Los `/dev/input/eventN` reales que ya se están repitiendo.
static EN_USO: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// Así se marcan los mandos de ZYKRO, para no repetirlos a ellos mismos.
const FISICO: &str = "zykro-mando";
/// El mapeo del juego está activo (lo prende la ventana): si su mapa usa el mando, el mando es suyo.
pub static MAPEO: AtomicBool = AtomicBool::new(false);
/// El motor de controles, para que el mando haga lo del mapa.
pub static MOTOR: OnceLock<Arc<Mutex<Motor>>> = OnceLock::new();
/// La última combinación del mando que se presionó y soltó (para asignarla en el editor).
static ULTIMO: Mutex<Option<String>> = Mutex::new(None);

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const EV_MSC: u16 = 0x04;
const EV_FF: u16 = 0x15;
const EV_UINPUT: u16 = 0x0101;
const UI_FF_UPLOAD: u16 = 1;
const UI_FF_ERASE: u16 = 2;
const BTN_JOYSTICK: usize = 0x120;
const BTN_SOUTH: usize = 0x130;
const ABS_X: usize = 0x00;
const ABS_Y: usize = 0x01;
const ABS_Z: usize = 0x02;
const ABS_RX: usize = 0x03;
const ABS_RY: usize = 0x04;
const ABS_RZ: usize = 0x05;
const ABS_GAS: usize = 0x09;
const ABS_BRAKE: usize = 0x0a;
const ABS_HAT0X: usize = 0x10;
const ABS_HAT0Y: usize = 0x11;
const KEY_MAX: usize = 0x2ff;
const ABS_MAX: usize = 0x3f;
const FF_MAX: usize = 0x7f;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Evento {
    segundos: i64,
    microsegundos: i64,
    tipo: u16,
    codigo: u16,
    valor: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Id {
    bus: u16,
    fabricante: u16,
    producto: u16,
    version: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct InfoEje {
    valor: i32,
    minimo: i32,
    maximo: i32,
    ruido: i32,
    plano: i32,
    resolucion: i32,
}

#[repr(C)]
struct Configuracion {
    id: Id,
    nombre: [u8; 80],
    efectos_max: u32,
}

#[repr(C)]
struct ConfEje {
    codigo: u16,
    _relleno: u16,
    info: InfoEje,
}

/// `struct ff_effect`: solo hace falta su `id` (lo demás se pasa tal cual al mando).
#[repr(C)]
#[derive(Clone, Copy)]
struct Efecto {
    tipo: u16,
    id: i16,
    resto: [u8; 44],
}

#[repr(C)]
struct Subida {
    pedido: u32,
    resultado: i32,
    efecto: Efecto,
    viejo: Efecto,
}

#[repr(C)]
struct Borrado {
    pedido: u32,
    resultado: i32,
    efecto: u32,
}

const _: () = assert!(std::mem::size_of::<Evento>() == 24);
const _: () = assert!(std::mem::size_of::<Configuracion>() == 92);
const _: () = assert!(std::mem::size_of::<ConfEje>() == 28);
const _: () = assert!(std::mem::size_of::<Efecto>() == 48);
const _: () = assert!(std::mem::size_of::<Subida>() == 104);

const fn ioc(direccion: u64, tipo: u8, numero: u8, tamano: usize) -> u64 {
    (direccion << 30) | ((tamano as u64) << 16) | ((tipo as u64) << 8) | numero as u64
}
const LEE: u64 = 2;
const ESCRIBE: u64 = 1;
const UI_DEV_CREATE: u64 = ioc(0, b'U', 1, 0);
const UI_DEV_SETUP: u64 = ioc(ESCRIBE, b'U', 3, 92);
const UI_ABS_SETUP: u64 = ioc(ESCRIBE, b'U', 4, 28);
const UI_SET_EVBIT: u64 = ioc(ESCRIBE, b'U', 100, 4);
const UI_SET_KEYBIT: u64 = ioc(ESCRIBE, b'U', 101, 4);
const UI_SET_ABSBIT: u64 = ioc(ESCRIBE, b'U', 103, 4);
const UI_SET_FFBIT: u64 = ioc(ESCRIBE, b'U', 107, 4);
const UI_SET_PHYS: u64 = ioc(ESCRIBE, b'U', 108, 8);
const UI_BEGIN_FF_UPLOAD: u64 = ioc(LEE | ESCRIBE, b'U', 200, 104);
const UI_END_FF_UPLOAD: u64 = ioc(ESCRIBE, b'U', 201, 104);
const UI_BEGIN_FF_ERASE: u64 = ioc(LEE | ESCRIBE, b'U', 202, 12);
const UI_END_FF_ERASE: u64 = ioc(ESCRIBE, b'U', 203, 12);
const EVIOCGID: u64 = ioc(LEE, b'E', 0x02, 8);
const EVIOCSFF: u64 = ioc(ESCRIBE, b'E', 0x80, 48);
const EVIOCRMFF: u64 = ioc(ESCRIBE, b'E', 0x81, 4);
const fn ui_get_sysname(tamano: usize) -> u64 {
    ioc(LEE, b'U', 44, tamano)
}
const fn eviocgname(tamano: usize) -> u64 {
    ioc(LEE, b'E', 0x06, tamano)
}
const fn eviocgphys(tamano: usize) -> u64 {
    ioc(LEE, b'E', 0x07, tamano)
}
const fn eviocgbit(tipo: u16, tamano: usize) -> u64 {
    ioc(LEE, b'E', 0x20 + tipo as u8, tamano)
}
const fn eviocgabs(eje: usize) -> u64 {
    ioc(LEE, b'E', 0x40 + eje as u8, 24)
}

fn ioctl<T>(fd: RawFd, pedido: u64, dato: *mut T) -> i32 {
    unsafe { libc::ioctl(fd, pedido as libc::c_ulong, dato) }
}

/// Los bits de un tipo de evento que tiene el dispositivo.
fn bits(fd: RawFd, tipo: u16, maximo: usize) -> Vec<usize> {
    let mut mapa = vec![0u8; maximo / 8 + 1];
    if ioctl(fd, eviocgbit(tipo, mapa.len()), mapa.as_mut_ptr()) < 0 {
        return Vec::new();
    }
    (0..=maximo).filter(|b| mapa[b / 8] & (1 << (b % 8)) != 0).collect()
}

fn texto(fd: RawFd, pedido: fn(usize) -> u64) -> String {
    let mut buf = [0u8; 128];
    if ioctl(fd, pedido(buf.len()), buf.as_mut_ptr()) < 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..buf.iter().position(|&b| b == 0).unwrap_or(buf.len())]).into_owned()
}

pub fn iniciar() {
    thread::Builder::new()
        .name("zykro-mandos".into())
        .spawn(|| {
            limpiar_nodos();
            loop {
                if ACTIVO.load(Ordering::Relaxed) {
                    buscar();
                }
                thread::sleep(Duration::from_secs(2));
            }
        })
        .expect("no se pudo crear el hilo de los mandos");
}

/// Busca mandos nuevos y repite cada uno en su hilo.
fn buscar() {
    let nodos = std::fs::read_dir("/dev/input").into_iter().flatten().flatten().map(|e| e.path());
    for ruta in nodos.filter(|r| r.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("event"))) {
        if EN_USO.lock().unwrap().contains(&ruta) {
            continue;
        }
        let Ok(real) = File::open(&ruta) else { continue };
        let fd = real.as_raw_fd();
        let teclas = bits(fd, EV_KEY, KEY_MAX);
        let es_mando = teclas.contains(&BTN_SOUTH) || teclas.contains(&BTN_JOYSTICK);
        if !es_mando || !bits(fd, EV_ABS, ABS_MAX).contains(&ABS_X) || texto(fd, eviocgphys) == FISICO {
            continue;
        }
        drop(real);
        EN_USO.lock().unwrap().push(ruta.clone());
        thread::spawn(move || {
            if let Err(e) = repetir(&ruta) {
                eprintln!("[mandos] {}: {e}", ruta.display());
            }
            EN_USO.lock().unwrap().retain(|r| r != &ruta);
        });
    }
}

/// Repite el mando `ruta` en uno virtual dentro de Android hasta que se desconecta o se apaga la opción.
fn repetir(ruta: &PathBuf) -> std::io::Result<()> {
    // Con escritura para la vibración; si no se puede, solo lectura.
    let mut real = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open(ruta).or_else(|_| {
        OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(ruta)
    })?;
    let fd = real.as_raw_fd();
    let nombre = texto(fd, eviocgname);
    let mut id = Id::default();
    ioctl(fd, EVIOCGID, &mut id);
    // Sony: los botones se muestran como en un mando de PlayStation.
    crate::mapeo::ESTILO_PLAYSTATION.store(id.fabricante == 0x054c, Ordering::Relaxed);
    let teclas = bits(fd, EV_KEY, KEY_MAX);
    let ejes = bits(fd, EV_ABS, ABS_MAX);
    let vibra = VIBRACION.load(Ordering::Relaxed) && !bits(fd, EV_FF, FF_MAX).is_empty();
    let (disposicion, estado) = Disposicion::de(fd, &ejes);

    let mut virtual_ = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open("/dev/uinput")?;
    let v = virtual_.as_raw_fd();
    for tipo in [EV_KEY, EV_ABS, EV_SYN, EV_MSC] {
        ioctl(v, UI_SET_EVBIT, tipo as usize as *mut libc::c_void);
    }
    for t in &teclas {
        ioctl(v, UI_SET_KEYBIT, *t as *mut libc::c_void);
    }
    for e in &ejes {
        ioctl(v, UI_SET_ABSBIT, *e as *mut libc::c_void);
        let mut conf = ConfEje { codigo: *e as u16, _relleno: 0, info: InfoEje::default() };
        ioctl(fd, eviocgabs(*e), &mut conf.info);
        ioctl(v, UI_ABS_SETUP, &mut conf);
    }
    if vibra {
        ioctl(v, UI_SET_EVBIT, EV_FF as usize as *mut libc::c_void);
        for f in bits(fd, EV_FF, FF_MAX) {
            ioctl(v, UI_SET_FFBIT, f as *mut libc::c_void);
        }
    }
    let fisico = CString::new(FISICO).unwrap();
    ioctl(v, UI_SET_PHYS, fisico.as_ptr() as *mut libc::c_char);
    let mut conf = Configuracion { id, nombre: [0; 80], efectos_max: if vibra { 16 } else { 0 } };
    for (destino, b) in conf.nombre.iter_mut().zip(nombre.bytes().take(79)) {
        *destino = b;
    }
    if ioctl(v, UI_DEV_SETUP, &mut conf) < 0 || ioctl(v, UI_DEV_CREATE, std::ptr::null_mut::<u8>()) < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let nodo = crear_nodo(v).ok_or_else(|| std::io::Error::other("no se pudo crear el nodo en Android"))?;
    CONECTADOS.lock().unwrap().push(nombre.clone());
    eprintln!("[mandos] {nombre} → Android ({nodo}){}", if vibra { ", con vibración" } else { "" });

    let resultado = bucle(&mut real, &mut virtual_, &disposicion, estado);

    CONECTADOS.lock().unwrap().retain(|n| n != &nombre);
    borrar_nodo(&nodo);
    resultado
}

/// La combinación del mando que se presionó y soltó desde la última vez (y la olvida): «GamepadA», o
/// «GamepadLeftShoulder + GamepadX» si se presionaron juntos (en el orden en que se presionaron).
pub fn tomar_boton() -> Option<String> {
    ULTIMO.lock().unwrap().take()
}

/// El nombre de BlueStacks de cada botón (códigos de evdev; los gatillos analógicos y la cruceta de
/// ejes se ven en `Estado::botones`).
fn nombre_boton(codigo: u16) -> Option<&'static str> {
    Some(match codigo {
        0x130 => "GamepadA",
        0x131 => "GamepadB",
        0x133 => "GamepadY",
        0x134 => "GamepadX",
        0x136 => "GamepadLeftShoulder",
        0x137 => "GamepadRightShoulder",
        0x138 => "GamepadLTrigger",
        0x139 => "GamepadRTrigger",
        0x13a => "GamepadBack",
        0x13b => "GamepadStart",
        0x13d => "GamepadLeftThumb",
        0x13e => "GamepadRightThumb",
        0x220 => "GamepadDpadUp",
        0x221 => "GamepadDpadDown",
        0x222 => "GamepadDpadLeft",
        0x223 => "GamepadDpadRight",
        _ => return None,
    })
}

/// Dónde trae el mando sus joysticks y gatillos analógicos, y el rango de cada eje.
struct Disposicion {
    izquierdo: (u16, u16),
    derecho: Option<(u16, u16)>,
    gatillos: Option<(u16, u16)>,
    rangos: HashMap<u16, (i32, i32)>,
}

impl Disposicion {
    /// También devuelve dónde están los ejes ahora.
    fn de(fd: RawFd, ejes: &[usize]) -> (Disposicion, Estado) {
        let mut estado = Estado { teclas: HashSet::new(), ejes: HashMap::new() };
        let mut rangos = HashMap::new();
        for &e in ejes {
            let mut info = InfoEje::default();
            ioctl(fd, eviocgabs(e), &mut info);
            rangos.insert(e as u16, (info.minimo, info.maximo));
            estado.ejes.insert(e as u16, info.valor);
        }
        let tiene = |a: usize, b: usize| (ejes.contains(&a) && ejes.contains(&b)).then_some((a as u16, b as u16));
        // Casi todos (xpad, DualShock, Switch) traen el derecho en RX/RY y los gatillos en Z/RZ; otros,
        // el derecho en Z/RZ y los gatillos en freno y acelerador.
        let (derecho, gatillos) = match tiene(ABS_RX, ABS_RY) {
            Some(rx) => (Some(rx), tiene(ABS_Z, ABS_RZ)),
            None => (tiene(ABS_Z, ABS_RZ), tiene(ABS_BRAKE, ABS_GAS)),
        };
        (Disposicion { izquierdo: (ABS_X as u16, ABS_Y as u16), derecho, gatillos, rangos }, estado)
    }

    /// El eje de −1 a 1.
    fn normal(&self, eje: u16, valor: i32) -> f32 {
        match self.rangos.get(&eje) {
            Some(&(min, max)) if max > min => (2.0 * (valor - min) as f32 / (max - min) as f32 - 1.0).clamp(-1.0, 1.0),
            _ => 0.0,
        }
    }

    /// Donde Android tiene que ver el eje mientras el mando es del mapeo: suelto.
    fn reposo(&self, eje: u16) -> i32 {
        let (min, max) = self.rangos.get(&eje).copied().unwrap_or((0, 0));
        let gatillo = self.gatillos.is_some_and(|(a, b)| eje == a || eje == b);
        if (ABS_HAT0X as u16..=0x17).contains(&eje) {
            0
        } else if gatillo {
            min
        } else {
            (min + max) / 2
        }
    }
}

/// Lo que tiene el mando presionado y dónde están sus ejes.
struct Estado {
    teclas: HashSet<u16>,
    ejes: HashMap<u16, i32>,
}

impl Estado {
    /// Los botones presionados, con los nombres de BlueStacks.
    fn botones(&self, d: &Disposicion) -> HashSet<&'static str> {
        let mut botones: HashSet<&'static str> = self.teclas.iter().filter_map(|&c| nombre_boton(c)).collect();
        // Los gatillos analógicos cuentan pasado un tercio del recorrido.
        if let Some((lt, rt)) = d.gatillos {
            for (eje, nombre) in [(lt, "GamepadLTrigger"), (rt, "GamepadRTrigger")] {
                if self.ejes.get(&eje).is_some_and(|&v| d.normal(eje, v) > -1.0 / 3.0) {
                    botones.insert(nombre);
                }
            }
        }
        let cruceta = |eje: usize| self.ejes.get(&(eje as u16)).copied().unwrap_or(0).signum();
        for (valor, negativo, positivo) in [(cruceta(ABS_HAT0X), "GamepadDpadLeft", "GamepadDpadRight"), (cruceta(ABS_HAT0Y), "GamepadDpadUp", "GamepadDpadDown")] {
            match valor {
                -1 => botones.insert(negativo),
                1 => botones.insert(positivo),
                _ => false,
            };
        }
        // Las direcciones de los joysticks, pasada la mitad (como `GamepadLStickUp` de BlueStacks).
        for (p, prefijo) in [(self.palanca(d, true), "L"), (self.palanca(d, false), "R")] {
            let direcciones: [(bool, &'static str, &'static str); 4] = [
                (p.y < -0.5, "GamepadLStickUp", "GamepadRStickUp"),
                (p.y > 0.5, "GamepadLStickDown", "GamepadRStickDown"),
                (p.x < -0.5, "GamepadLStickLeft", "GamepadRStickLeft"),
                (p.x > 0.5, "GamepadLStickRight", "GamepadRStickRight"),
            ];
            for (inclinado, izquierdo, derecho) in direcciones {
                if inclinado {
                    botones.insert(if prefijo == "L" { izquierdo } else { derecho });
                }
            }
        }
        botones
    }

    /// El joystick izquierdo o el derecho, de −1 a 1 (Y hacia abajo).
    fn palanca(&self, d: &Disposicion, izquierdo: bool) -> Vec2 {
        let Some((ex, ey)) = (if izquierdo { Some(d.izquierdo) } else { d.derecho }) else { return Vec2::ZERO };
        let eje = |e: u16| self.ejes.get(&e).map_or(0.0, |&v| d.normal(e, v));
        Vec2::new(eje(ex), eje(ey))
    }
}

/// Le pasa a Android el mando entero: suelto (`reposo`) o como está.
fn escribir_estado(virtual_: &mut File, estado: &Estado, d: &Disposicion, reposo: bool) {
    for &codigo in &estado.teclas {
        escribir(virtual_, &Evento { tipo: EV_KEY, codigo, valor: !reposo as i32, ..Default::default() });
    }
    for (&codigo, &valor) in &estado.ejes {
        let valor = if reposo { d.reposo(codigo) } else { valor };
        escribir(virtual_, &Evento { tipo: EV_ABS, codigo, valor, ..Default::default() });
    }
    escribir(virtual_, &Evento { tipo: EV_SYN, ..Default::default() });
}

/// Pasa los eventos del mando al virtual (o al mapeo) y la vibración del virtual al mando.
fn bucle(real: &mut File, virtual_: &mut File, disposicion: &Disposicion, mut estado: Estado) -> std::io::Result<()> {
    let (fd, v) = (real.as_raw_fd(), virtual_.as_raw_fd());
    // Los efectos de vibración de Android (id del mando virtual) → los del mando real.
    let mut efectos: HashMap<i16, i16> = HashMap::new();
    let mut buf = [Evento::default(); 64];
    let mut presionados = estado.botones(disposicion);
    // Los botones presionados juntos (para el editor), en orden, hasta que se suelten todos.
    let mut grupo: Vec<&'static str> = Vec::new();
    let mut del_mapeo = false;
    let mut proximo = Instant::now();
    loop {
        if !ACTIVO.load(Ordering::Relaxed) {
            return Ok(());
        }
        let motor = MOTOR.get();
        let (usar, joystick) = motor.map_or((false, false), |m| {
            let m = m.lock().unwrap();
            (MAPEO.load(Ordering::Relaxed) && m.usa_mando(), m.usa_joystick())
        });
        if usar != del_mapeo {
            // Que Android no se quede con botones presionados o el joystick movido (ni al revés).
            escribir_estado(virtual_, &estado, disposicion, usar);
            del_mapeo = usar;
        }
        // Si un joystick mueve algo, se despierta cada 10 ms.
        let espera = if joystick && del_mapeo { proximo.saturating_duration_since(Instant::now()).as_millis() as i32 } else { 500 };
        let mut espera_fd = [libc::pollfd { fd, events: libc::POLLIN, revents: 0 }, libc::pollfd { fd: v, events: libc::POLLIN, revents: 0 }];
        unsafe { libc::poll(espera_fd.as_mut_ptr(), 2, espera) };
        if espera_fd[0].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Ok(()); // se desconectó
        }
        for evento in leer(real, &mut buf)? {
            match evento.tipo {
                // 2 es una repetición.
                EV_KEY if evento.valor == 1 => {
                    estado.teclas.insert(evento.codigo);
                }
                EV_KEY if evento.valor == 0 => {
                    estado.teclas.remove(&evento.codigo);
                }
                EV_ABS => {
                    estado.ejes.insert(evento.codigo, evento.valor);
                }
                _ => {}
            }
            if !del_mapeo && matches!(evento.tipo, EV_KEY | EV_ABS | EV_SYN | EV_MSC) {
                escribir(virtual_, evento);
            }
            if evento.tipo == EV_SYN {
                let ahora = estado.botones(disposicion);
                for &nombre in ahora.difference(&presionados) {
                    if !grupo.contains(&nombre) {
                        grupo.push(nombre);
                    }
                }
                if ahora.is_empty() && !grupo.is_empty() {
                    *ULTIMO.lock().unwrap() = Some(grupo.join(" + "));
                    grupo.clear();
                }
                if let Some(m) = motor.filter(|_| del_mapeo) {
                    let mut m = m.lock().unwrap();
                    for &nombre in ahora.difference(&presionados) {
                        m.boton_mando(nombre, true);
                    }
                    for &nombre in presionados.difference(&ahora) {
                        m.boton_mando(nombre, false);
                    }
                }
                presionados = ahora;
            }
        }
        if let (Some(m), true, true) = (motor, joystick, del_mapeo) {
            if Instant::now() >= proximo {
                m.lock().unwrap().joysticks(estado.palanca(disposicion, true), estado.palanca(disposicion, false));
                proximo = Instant::now() + Duration::from_millis(10);
            }
        }
        for evento in leer(virtual_, &mut buf)? {
            match (evento.tipo, evento.codigo) {
                (EV_UINPUT, UI_FF_UPLOAD) => {
                    let mut subida: Subida = unsafe { std::mem::zeroed() };
                    subida.pedido = evento.valor as u32;
                    ioctl(v, UI_BEGIN_FF_UPLOAD, &mut subida);
                    let virtual_id = subida.efecto.id;
                    let mut efecto = subida.efecto;
                    efecto.id = efectos.get(&virtual_id).copied().unwrap_or(-1);
                    subida.resultado = if ioctl(fd, EVIOCSFF, &mut efecto) == 0 {
                        efectos.insert(virtual_id, efecto.id);
                        0
                    } else {
                        -std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO)
                    };
                    ioctl(v, UI_END_FF_UPLOAD, &mut subida);
                }
                (EV_UINPUT, UI_FF_ERASE) => {
                    let mut borrado = Borrado { pedido: evento.valor as u32, resultado: 0, efecto: 0 };
                    ioctl(v, UI_BEGIN_FF_ERASE, &mut borrado);
                    if let Some(real_id) = efectos.remove(&(borrado.efecto as i16)) {
                        ioctl(fd, EVIOCRMFF, real_id as usize as *mut libc::c_void);
                    }
                    ioctl(v, UI_END_FF_ERASE, &mut borrado);
                }
                // Reproducir o parar un efecto (o la fuerza general, que no tiene id).
                (EV_FF, codigo) => {
                    let codigo = efectos.get(&(codigo as i16)).map_or(codigo, |&r| r as u16);
                    escribir(real, &Evento { codigo, ..*evento });
                }
                _ => {}
            }
        }
    }
}

fn leer<'a>(archivo: &mut File, buf: &'a mut [Evento; 64]) -> std::io::Result<&'a [Evento]> {
    let bytes = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, std::mem::size_of_val(buf)) };
    match archivo.read(bytes) {
        Ok(n) => Ok(&buf[..n / std::mem::size_of::<Evento>()]),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(&buf[..0]),
        Err(e) => Err(e), // ENODEV: se desconectó
    }
}

fn escribir(archivo: &mut File, evento: &Evento) {
    let bytes = unsafe { std::slice::from_raw_parts(evento as *const Evento as *const u8, std::mem::size_of::<Evento>()) };
    let _ = archivo.write_all(bytes);
}

/// Crea dentro de Android el `/dev/input/eventN` del mando virtual; devuelve su nombre.
fn crear_nodo(v: RawFd) -> Option<String> {
    let mut nombre = [0u8; 64];
    if ioctl(v, ui_get_sysname(nombre.len()), nombre.as_mut_ptr()) < 0 {
        return None;
    }
    let sistema = String::from_utf8_lossy(&nombre[..nombre.iter().position(|&b| b == 0)?]).into_owned();
    let carpeta = PathBuf::from("/sys/devices/virtual/input").join(&sistema);
    for _ in 0..30 {
        let evento = std::fs::read_dir(&carpeta).into_iter().flatten().flatten().find(|e| e.file_name().to_string_lossy().starts_with("event"));
        if let Some(evento) = evento {
            let nodo = evento.file_name().to_string_lossy().into_owned();
            let numeros = std::fs::read_to_string(evento.path().join("dev")).ok()?;
            let (mayor, menor) = numeros.trim().split_once(':')?;
            let orden = format!("mkdir -p /dev/input; rm -f /dev/input/{nodo}; mknod -m 666 /dev/input/{nodo} c {mayor} {menor}");
            android::shell(&orden)?;
            recordar_nodo(&nodo, true);
            return Some(nodo);
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

fn borrar_nodo(nodo: &str) {
    android::shell(&format!("rm -f /dev/input/{nodo}"));
    recordar_nodo(nodo, false);
}

/// Los nodos que ZYKRO creó en Android (por si se cierra sin borrarlos: otro dispositivo de la
/// laptop podría tomar ese número y Android lo vería).
fn archivo_nodos() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"))
        .join("zykro/mandos")
}

fn recordar_nodo(nodo: &str, agregar: bool) {
    let archivo = archivo_nodos();
    let mut nodos: HashSet<String> = std::fs::read_to_string(&archivo).unwrap_or_default().lines().map(String::from).collect();
    if agregar {
        nodos.insert(nodo.to_string());
    } else {
        nodos.remove(nodo);
    }
    let _ = std::fs::write(archivo, nodos.into_iter().collect::<Vec<_>>().join("\n"));
}

/// Borra de Android los nodos de mandos que quedaron de antes.
pub fn limpiar_nodos() {
    let nodos = std::fs::read_to_string(archivo_nodos()).unwrap_or_default();
    let nodos: Vec<&str> = nodos.lines().filter(|n| n.starts_with("event") && n[5..].chars().all(|c| c.is_ascii_digit())).collect();
    if !nodos.is_empty() {
        android::shell(&nodos.iter().map(|n| format!("rm -f /dev/input/{n}")).collect::<Vec<_>>().join("; "));
    }
    let _ = std::fs::remove_file(archivo_nodos());
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un mando como el de Xbox (xpad): joysticks de −32768 a 32767, gatillos de 0 a 1023 y cruceta.
    fn xbox() -> (Disposicion, Estado) {
        let mut rangos = HashMap::new();
        for eje in [ABS_X, ABS_Y, ABS_RX, ABS_RY] {
            rangos.insert(eje as u16, (-32768, 32767));
        }
        for eje in [ABS_Z, ABS_RZ] {
            rangos.insert(eje as u16, (0, 1023));
        }
        for eje in [ABS_HAT0X, ABS_HAT0Y] {
            rangos.insert(eje as u16, (-1, 1));
        }
        let d = Disposicion {
            izquierdo: (ABS_X as u16, ABS_Y as u16),
            derecho: Some((ABS_RX as u16, ABS_RY as u16)),
            gatillos: Some((ABS_Z as u16, ABS_RZ as u16)),
            rangos,
        };
        let ejes = [(ABS_X, 0), (ABS_Y, 0), (ABS_RX, 0), (ABS_RY, 0), (ABS_Z, 0), (ABS_RZ, 0), (ABS_HAT0X, 0), (ABS_HAT0Y, 0)];
        (d, Estado { teclas: HashSet::new(), ejes: ejes.into_iter().map(|(e, v)| (e as u16, v)).collect() })
    }

    #[test]
    fn botones_gatillos_y_cruceta() {
        let (d, mut e) = xbox();
        assert!(e.botones(&d).is_empty());
        e.teclas.insert(0x13e);
        e.ejes.insert(ABS_RZ as u16, 600);
        e.ejes.insert(ABS_HAT0Y as u16, -1);
        let mut botones: Vec<_> = e.botones(&d).into_iter().collect();
        botones.sort();
        assert_eq!(botones, vec!["GamepadDpadUp", "GamepadRTrigger", "GamepadRightThumb"]);
        // Un gatillo apenas presionado no cuenta; el joystick inclinado a la izquierda, sí.
        e.ejes.insert(ABS_RZ as u16, 200);
        e.ejes.insert(ABS_X as u16, -30000);
        let botones = e.botones(&d);
        assert!(!botones.contains("GamepadRTrigger") && botones.contains("GamepadLStickLeft"), "{botones:?}");
        // Cómo lo ve Android mientras es del mapeo: suelto.
        assert_eq!((d.reposo(ABS_RX as u16), d.reposo(ABS_RZ as u16), d.reposo(ABS_HAT0X as u16)), (0, 0, 0));
    }

    /// El bucle con un mando falso (una tubería) y la puntería usando el mando: R3 entra a la
    /// puntería, el joystick derecho mueve la cámara cada 10 ms y a Android solo le llega el mando
    /// suelto, no los botones.
    #[test]
    fn bucle_le_da_el_mando_al_mapeo() {
        use std::os::unix::net::{UnixListener, UnixStream};
        use crate::control::{Control, BAJAR, MOVER};
        use crate::mapeo::{Mapa, Punto, Raton};

        let ruta = std::env::temp_dir().join(format!("zykro-mando-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        let servidor = UnixListener::bind(&ruta).unwrap();
        let ctl = Control::conectar(ruta.to_str().unwrap()).unwrap();
        let (mut toques, _) = servidor.accept().unwrap();
        let _ = std::fs::remove_file(&ruta);

        let mut r = Raton::nuevo(Punto::nuevo(0.75, 0.5));
        (r.sens, r.tweaks) = (1.0, 2);
        r.mando.stick = "RightStick".into();
        r.mando.iniciar = "GamepadRightThumb".into();
        let motor = Arc::new(Mutex::new(Motor::new()));
        {
            let mut m = motor.lock().unwrap();
            let mut mapa = Mapa::default();
            mapa.raton = Some(r);
            m.mapa = Some(mapa);
            m.fijar_salida(Some(ctl), 1280.0, 720.0);
        }
        assert!(MOTOR.set(motor.clone()).is_ok());
        MAPEO.store(true, Ordering::Relaxed);

        let (lectura, escritura) = nix::unistd::pipe().unwrap();
        unsafe { libc::fcntl(lectura.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
        let mut real = File::from(lectura);
        let mut escritura = File::from(escritura);
        let (a, mut android_lado) = UnixStream::pair().unwrap();
        a.set_nonblocking(true).unwrap();
        android_lado.set_nonblocking(true).unwrap();
        let mut virtual_ = File::from(std::os::fd::OwnedFd::from(a));
        let (d, estado) = xbox();
        let hilo = thread::spawn(move || bucle(&mut real, &mut virtual_, &d, estado));

        let mandar = |escritura: &mut File, tipo: u16, codigo: u16, valor: i32| {
            escribir(escritura, &Evento { tipo, codigo, valor, ..Default::default() });
            escribir(escritura, &Evento { tipo: EV_SYN, ..Default::default() });
        };
        thread::sleep(Duration::from_millis(30));
        mandar(&mut escritura, EV_KEY, 0x13e, 1);
        mandar(&mut escritura, EV_KEY, 0x13e, 0);
        mandar(&mut escritura, EV_ABS, ABS_RX as u16, 32767);
        thread::sleep(Duration::from_millis(55));
        mandar(&mut escritura, EV_ABS, ABS_RX as u16, 0);
        thread::sleep(Duration::from_millis(30));
        drop(escritura);
        hilo.join().unwrap().unwrap();
        motor.lock().unwrap().fijar_salida(None, 1280.0, 720.0);
        MAPEO.store(false, Ordering::Relaxed);

        // La cámara: baja en el ancla (960) y se mueve 20 cuentas por vuelta (1.125 % = 14.4 px).
        let mut datos = Vec::new();
        let _ = toques.read_to_end(&mut datos);
        let xs: Vec<(u8, i32)> = datos.chunks_exact(32).filter(|m| m[0] == 0x02).map(|m| (m[1], i32::from_be_bytes(m[10..14].try_into().unwrap()))).collect();
        assert_eq!(xs.first(), Some(&(BAJAR, 960)), "{xs:?}");
        assert!(xs.len() >= 4 && xs[1..].iter().all(|&(a, _)| a == MOVER), "{xs:?}");
        assert_eq!(xs[1].1, 974, "{xs:?}");
        assert!(xs.windows(2).all(|w| w[1].1 > w[0].1), "siempre hacia la derecha: {xs:?}");

        // Android: solo el mando suelto (ningún botón presionado).
        let mut bytes = Vec::new();
        let _ = android_lado.read_to_end(&mut bytes);
        let eventos: Vec<(u16, u16, i32)> = bytes
            .chunks_exact(std::mem::size_of::<Evento>())
            .map(|b| (u16::from_ne_bytes([b[16], b[17]]), u16::from_ne_bytes([b[18], b[19]]), i32::from_ne_bytes([b[20], b[21], b[22], b[23]])))
            .collect();
        assert!(!eventos.is_empty() && eventos.iter().all(|&(tipo, _, valor)| tipo != EV_KEY || valor == 0), "{eventos:?}");
        assert!(eventos.contains(&(EV_ABS, ABS_RX as u16, 0)), "{eventos:?}");
    }

    #[test]
    fn joysticks_de_menos_uno_a_uno() {
        let (d, mut e) = xbox();
        e.ejes.insert(ABS_RX as u16, 32767);
        e.ejes.insert(ABS_Y as u16, -32768);
        let (izquierdo, derecho) = (e.palanca(&d, true), e.palanca(&d, false));
        assert!((derecho.x - 1.0).abs() < 1e-4 && derecho.y.abs() < 1e-3, "{derecho:?}");
        assert!((izquierdo.y + 1.0).abs() < 1e-4 && izquierdo.x.abs() < 1e-3, "{izquierdo:?}");
    }
}
