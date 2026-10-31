//! Mandos (pestaña Gamepad): los de la laptop, por USB o Bluetooth, llegan a Android como mandos
//! propios y los juegos que aceptan mando los usan como en un teléfono. ZYKRO lee el mando (evdev) y
//! lo repite en uno virtual (uinput) con los mismos botones, ejes, fabricante y modelo (Android elige
//! la distribución de los botones por fabricante y modelo). Redroid no crea los nodos de entrada, así
//! que el del mando virtual se crea a mano dentro de Android, como el del teclado (`teclado-fisico.c`).
//! La vibración que pide Android (uinput la recibe como subidas de efectos) se pasa al mando real.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crate::android;

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
    let teclas = bits(fd, EV_KEY, KEY_MAX);
    let ejes = bits(fd, EV_ABS, ABS_MAX);
    let vibra = VIBRACION.load(Ordering::Relaxed) && !bits(fd, EV_FF, FF_MAX).is_empty();

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

    let resultado = bucle(&mut real, &mut virtual_);

    CONECTADOS.lock().unwrap().retain(|n| n != &nombre);
    borrar_nodo(&nodo);
    resultado
}

/// Pasa los eventos del mando al virtual, y la vibración del virtual al mando.
fn bucle(real: &mut File, virtual_: &mut File) -> std::io::Result<()> {
    let (fd, v) = (real.as_raw_fd(), virtual_.as_raw_fd());
    // Los efectos de vibración de Android (id del mando virtual) → los del mando real.
    let mut efectos: HashMap<i16, i16> = HashMap::new();
    let mut buf = [Evento::default(); 64];
    loop {
        if !ACTIVO.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut espera = [libc::pollfd { fd, events: libc::POLLIN, revents: 0 }, libc::pollfd { fd: v, events: libc::POLLIN, revents: 0 }];
        unsafe { libc::poll(espera.as_mut_ptr(), 2, 500) };
        if espera[0].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Ok(()); // se desconectó
        }
        for evento in leer(real, &mut buf)? {
            if matches!(evento.tipo, EV_KEY | EV_ABS | EV_SYN | EV_MSC) {
                escribir(virtual_, &evento);
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
