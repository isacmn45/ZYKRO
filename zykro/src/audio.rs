//! Sonido de Android. swag-rust-server lo pone en un anillo de memoria compartida («ASHM»):
//!
//! - `audio.sock` manda un byte 0xFC con el fd de un memfd (`swag-audio-ring`) y luego 20 bytes en
//!   big-endian: «ASHM», frecuencia, canales, bytes por cuadro y tamaño del anillo.
//! - En el memfd, la cabecera (little-endian, 1024 bytes) tiene los mismos datos y, en 0x1c, un
//!   contador que sube de 2 en 2 por bloque (impar mientras escribe). En 0x40 hay 32 entradas de
//!   16 bytes {contador, desplazamiento, largo, 0}: la del bloque `n` está en 0x40 + (n & 0x3e) * 8.
//! - Los datos (PCM s16 intercalado) empiezan en 0x400 y dan la vuelta al llegar al final.
//!
//! Se reproduce con un flujo de PulseAudio (pipewire-pulse). Detalles en docs/investigacion.md.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use libpulse_binding::def::BufferAttr;
use libpulse_binding::error::PAErr;
use libpulse_binding::sample::{Format as SampleFormat, Spec};
use libpulse_binding::stream::Direction;
use libpulse_binding::time::MicroSeconds;
use libpulse_simple_binding::Simple;
use memmap2::{Mmap, MmapOptions};
use nix::errno::Errno;
use nix::sys::socket::{connect, recvmsg, socket, AddressFamily, ControlMessageOwned, MsgFlags, SockFlag, SockType, UnixAddr};

use crate::android::CONTENEDOR;

const SOCKET: &str = "/tmp/swag_sockets/audio.sock";
const CABECERA: usize = 0x400;
const ENTRADAS: usize = 0x40;
/// Sonido guardado en el servidor: lo justo para no cortarse.
const BUFER_MS: u32 = 60;
/// Si el retraso total pasa de esto, se tiran bloques.
const RETRASO_MAX_MS: u64 = 120;

struct Formato {
    frecuencia: u32,
    canales: u32,
}

pub fn iniciar() {
    thread::Builder::new()
        .name("zykro-audio".into())
        .spawn(|| {
            let mut ultimo_reinicio: Option<Instant> = None;
            loop {
                match sesion(&mut ultimo_reinicio) {
                    Ok(()) => {}
                    // Mientras Android arranca, el socket existe pero nadie escucha todavía.
                    Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {}
                    Err(e) => eprintln!("[audio] {e}"),
                }
                thread::sleep(Duration::from_secs(1));
            }
        })
        .expect("no se pudo crear el hilo de audio");
}

/// Una conexión: dura hasta que se cierra el socket (Android se apaga o se reinicia el servidor).
fn sesion(ultimo_reinicio: &mut Option<Instant>) -> io::Result<()> {
    while !Path::new(SOCKET).exists() {
        thread::sleep(Duration::from_millis(500));
    }
    // El servidor atiende a un solo cliente de audio y no se entera de que uno anterior murió (no
    // vuelve a escribir en su socket): si no acepta o no saluda, se reinicia, como mucho una vez por minuto.
    let saludado = conectar().and_then(|socket| {
        socket.set_read_timeout(Some(Duration::from_secs(4)))?;
        saludo(&socket).map(|(anillo, formato)| (socket, anillo, formato))
    });
    let (socket, anillo, formato) = match saludado {
        Ok(r) => r,
        Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
            if ultimo_reinicio.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
                *ultimo_reinicio = Some(Instant::now());
                let _ = Command::new("docker").args(["exec", CONTENEDOR, "setprop", "ctl.restart", "swag-rust-server"]).status();
                return Err(io::Error::other("el servidor de audio no contestaba: se reinició"));
            }
            return Err(e);
        }
        Err(e) => return Err(e),
    };
    socket.set_nonblocking(true)?;
    let mut salida: Option<Simple> = None;
    let mut reintentar = Instant::now();
    let mut leido = contador(&anillo) & !1;
    let mut bloque = Vec::new();
    loop {
        // ¿Se cerró el socket? (se apagó Android o se reinició el servidor)
        let mut b = [0u8; 64];
        if let Ok(0) = io::Read::read(&mut &socket, &mut b) {
            return Ok(());
        }
        // Sin salida de sonido se sigue leyendo (y tirando) el anillo: si se cerrara el socket, el
        // servidor se quedaría esperando a un cliente muerto.
        if salida.is_none() && Instant::now() >= reintentar {
            match abrir_salida(&formato) {
                Ok(s) => salida = Some(s),
                Err(e) => {
                    eprintln!("[audio] no se pudo abrir la salida de sonido: {e}");
                    reintentar = Instant::now() + Duration::from_secs(5);
                }
            }
        }
        let actual = contador(&anillo) & !1;
        if actual.wrapping_sub(leido) > 60 {
            // Muy atrasados (o el contador volvió a empezar): seguir desde el bloque más nuevo.
            leido = actual;
        }
        while leido != actual {
            let siguiente = leido.wrapping_add(2);
            if !copiar_bloque(&anillo, siguiente, &mut bloque) {
                leido = actual;
                break;
            }
            leido = siguiente;
            let Some(s) = &salida else { continue };
            // Los relojes de Android y de la tarjeta de sonido no van exactos: si el sonido se va
            // acumulando, se tira un bloque para que no se retrase.
            if s.get_latency().is_ok_and(|l| l.0 > RETRASO_MAX_MS * 1000) {
                continue;
            }
            if let Err(e) = s.write(&bloque) {
                eprintln!("[audio] se perdió la salida de sonido: {e}");
                salida = None;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Flujo de PulseAudio (PipeWire): el servidor guarda unos milisegundos de sonido para que los
/// tirones de Android no se oigan como cortes.
fn abrir_salida(f: &Formato) -> Result<Simple, PAErr> {
    let spec = Spec { format: SampleFormat::S16le, rate: f.frecuencia, channels: f.canales as u8 };
    let ms = |n: u32| spec.usec_to_bytes(MicroSeconds(n as u64 * 1000)) as u32;
    let atributos = BufferAttr { maxlength: u32::MAX, tlength: ms(BUFER_MS), prebuf: ms(BUFER_MS * 2 / 3), minreq: ms(10), fragsize: u32::MAX };
    Simple::new(None, "ZYKRO", Direction::Playback, None, "Android", &spec, None, Some(&atributos))
}

/// Conecta sin quedarse colgado: si el servidor no acepta (cola llena), connect() bloquearía para siempre.
fn conectar() -> io::Result<UnixStream> {
    let direccion = UnixAddr::new(SOCKET).map_err(io::Error::from)?;
    let inicio = Instant::now();
    loop {
        let fd = socket(AddressFamily::Unix, SockType::Stream, SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC, None)
            .map_err(io::Error::from)?;
        match connect(fd.as_raw_fd(), &direccion) {
            Ok(()) => {
                let socket = UnixStream::from(fd);
                socket.set_nonblocking(false)?;
                return Ok(socket);
            }
            Err(Errno::EAGAIN) if inicio.elapsed() < Duration::from_secs(4) => thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(io::Error::from(e)),
        }
    }
}

fn saludo(socket: &UnixStream) -> io::Result<(Mmap, Formato)> {
    let mut primero = [0u8; 1];
    let (n, fd) = recibir_fd(socket.as_raw_fd(), &mut primero)?;
    if n != 1 || primero[0] != 0xFC {
        return Err(io::Error::other(format!("el servidor no usa el anillo de audio ({:02x})", primero[0])));
    }
    let fd = fd.ok_or_else(|| io::Error::other("el saludo de audio no trajo el fd"))?;
    let mut meta = [0u8; 20];
    io::Read::read_exact(&mut &*socket, &mut meta)?;
    let be = |i: usize| u32::from_be_bytes(meta[i..i + 4].try_into().unwrap());
    if &meta[..4] != b"ASHM" || be(16) as usize <= CABECERA {
        return Err(io::Error::other("cabecera de audio inválida"));
    }
    let formato = Formato { frecuencia: be(4).max(1), canales: be(8).clamp(1, 2) };
    let archivo = std::fs::File::from(fd);
    let anillo = unsafe { MmapOptions::new().len(be(16) as usize).map(&archivo)? };
    Ok((anillo, formato))
}

fn recibir_fd(socket: RawFd, buf: &mut [u8]) -> io::Result<(usize, Option<OwnedFd>)> {
    let mut iov = [io::IoSliceMut::new(buf)];
    let mut espacio = nix::cmsg_space!([RawFd; 2]);
    let msg = recvmsg::<()>(socket, &mut iov, Some(&mut espacio), MsgFlags::MSG_CMSG_CLOEXEC).map_err(io::Error::from)?;
    let mut fd = None;
    for c in msg.cmsgs().map_err(io::Error::from)? {
        if let ControlMessageOwned::ScmRights(v) = c {
            fd = v.first().map(|&f| unsafe { OwnedFd::from_raw_fd(f) });
        }
    }
    Ok((msg.bytes, fd))
}

fn u32_en(anillo: &Mmap, i: usize) -> u32 {
    // El servidor escribe mientras se lee: lectura volátil para no quedarse con un valor viejo.
    unsafe { std::ptr::read_volatile(anillo.as_ptr().add(i) as *const u32) }.to_le()
}

fn contador(anillo: &Mmap) -> u32 {
    u32_en(anillo, 0x1c)
}

/// Copia el bloque `n` a `destino`. Falso si ya se sobrescribió (la entrada es de otro bloque).
fn copiar_bloque(anillo: &Mmap, n: u32, destino: &mut Vec<u8>) -> bool {
    let capacidad = u32_en(anillo, 0x10) as usize;
    let entrada = ENTRADAS + ((n & 0x3e) as usize) * 8;
    if u32_en(anillo, entrada) != n || capacidad == 0 || CABECERA + capacidad > anillo.len() {
        return false;
    }
    let (inicio, largo) = (u32_en(anillo, entrada + 4) as usize % capacidad, (u32_en(anillo, entrada + 8) as usize).min(capacidad));
    let datos = &anillo[CABECERA..CABECERA + capacidad];
    destino.clear();
    let primera = largo.min(capacidad - inicio);
    destino.extend_from_slice(&datos[inicio..inicio + primera]);
    destino.extend_from_slice(&datos[..largo - primera]);
    // Si el servidor reescribió el bloque mientras se copiaba, se descarta.
    u32_en(anillo, entrada) == n
}
