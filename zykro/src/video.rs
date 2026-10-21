//! Video sin comprimir desde `video.sock`: el servidor comparte los buffers de pantalla de Android
//! (DMA-BUF) y avisa cuál mostrar. Formato en docs/investigacion.md.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, IoSliceMut, Read};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use memmap2::{Mmap, MmapOptions};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::socket::{recvmsg, ControlMessageOwned, MsgFlags};

const DMA_BUF_IOCTL_SYNC: libc::c_ulong = 0x4008_6200;
const SYNC_READ: u64 = 1;
const SYNC_START: u64 = 0;
const SYNC_END: u64 = 4;

/// Último cuadro recibido, listo para subir a la GPU.
pub struct Cuadro {
    pub ancho: usize,
    pub alto: usize,
    pub rgba: Vec<u8>,
}

pub type Compartido = Arc<Mutex<Option<Cuadro>>>;

struct Buffer {
    _archivo: File,
    mapa: Mmap,
    ancho: usize,
    alto: usize,
    stride: usize,
    bgra: bool,
}

/// Datos de la cabecera DMB2.
pub struct Pantalla {
    pub ancho: u32,
    pub alto: u32,
}

fn be32(d: &[u8], i: usize) -> u32 {
    u32::from_be_bytes(d[i..i + 4].try_into().unwrap())
}

fn sync(fd: RawFd, flags: u64) {
    unsafe {
        libc::ioctl(fd, DMA_BUF_IOCTL_SYNC, &flags as *const u64);
    }
}

fn recibir(socket: &UnixStream, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
    let mut iov = [IoSliceMut::new(buf)];
    let mut espacio = nix::cmsg_space!([RawFd; 4]);
    let msg = recvmsg::<()>(socket.as_raw_fd(), &mut iov, Some(&mut espacio), MsgFlags::MSG_CMSG_CLOEXEC)
        .map_err(io::Error::from)?;
    let mut fds = Vec::new();
    for c in msg.cmsgs().map_err(io::Error::from)? {
        if let ControlMessageOwned::ScmRights(v) = c {
            fds.extend(v.into_iter().map(|fd| unsafe { OwnedFd::from_raw_fd(fd) }));
        }
    }
    Ok((msg.bytes, fds))
}

/// Lee la cabecera. Bloquea hasta que Android redibuja la pantalla por primera vez,
/// así que hay que forzar un redibujado después de conectar.
pub fn leer_cabecera(socket: &mut UnixStream) -> io::Result<Pantalla> {
    let mut cab = [0u8; 21];
    socket.read_exact(&mut cab)?;
    if cab[0] != 0xFD || &cab[1..5] != b"DMB2" {
        return Err(io::Error::other(format!("el servidor no está en modo DMA-BUF ({:02x})", cab[0])));
    }
    Ok(Pantalla { ancho: be32(&cab, 5), alto: be32(&cab, 9) })
}

/// Recibe cuadros hasta que se cierre la conexión. Llama a `nuevo` con cada cuadro.
pub fn recibir_cuadros(socket: UnixStream, destino: Compartido, mut nuevo: impl FnMut()) -> io::Result<()> {
    let mut buffers: HashMap<u32, Buffer> = HashMap::new();
    let mut paquete = [0u8; 64];
    loop {
        let (n, mut fds) = recibir(&socket, &mut paquete)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Android cerró el video"));
        }
        if n < 8 {
            continue;
        }
        let d = &paquete[..n];
        let id = be32(d, 4);
        match &d[..4] {
            b"DMBR" if n >= 36 && !fds.is_empty() => {
                let (ancho, alto, stride, tam, formato) =
                    (be32(d, 8) as usize, be32(d, 12) as usize, be32(d, 16) as usize, be32(d, 20) as usize, be32(d, 24));
                let archivo = File::from(fds.remove(0));
                let mapa = unsafe { MmapOptions::new().len(tam).map(&archivo)? };
                buffers.insert(id, Buffer { _archivo: archivo, mapa, ancho, alto, stride, bgra: formato == 5 });
            }
            b"DMBX" => {
                // Si trae una fence, esperar (máx. 50 ms) a que Android termine de dibujar.
                if let Some(fence) = fds.first() {
                    let mut pfd = [PollFd::new(fence.as_fd(), PollFlags::POLLIN)];
                    let _ = poll(&mut pfd, PollTimeout::from(50u8));
                }
                let Some(b) = buffers.get(&id) else { continue };
                let fila = b.ancho * 4;
                let mut rgba = Vec::with_capacity(fila * b.alto);
                let fd = b._archivo.as_raw_fd();
                sync(fd, SYNC_START | SYNC_READ);
                for y in 0..b.alto {
                    rgba.extend_from_slice(&b.mapa[y * b.stride..y * b.stride + fila]);
                }
                sync(fd, SYNC_END | SYNC_READ);
                if b.bgra {
                    for px in rgba.chunks_exact_mut(4) {
                        px.swap(0, 2);
                    }
                }
                *destino.lock().unwrap() = Some(Cuadro { ancho: b.ancho, alto: b.alto, rgba });
                nuevo();
            }
            _ => {}
        }
    }
}
