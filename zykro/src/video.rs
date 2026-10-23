//! Video sin comprimir desde `video.sock`: el servidor comparte los buffers de pantalla de Android
//! (DMA-BUF) y avisa cuál mostrar. Formato en docs/investigacion.md. La ventana los muestra directo
//! desde la GPU (`textura.rs`), sin copiarlos.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, IoSliceMut, Read};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{Color32, ColorImage};
use memmap2::{Mmap, MmapOptions};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::socket::{recvmsg, ControlMessageOwned, MsgFlags};

const DMA_BUF_IOCTL_SYNC: libc::c_ulong = 0x4008_6200;
const SYNC_READ: u64 = 1;
const SYNC_START: u64 = 0;
const SYNC_END: u64 = 4;

static SERIE: AtomicU64 = AtomicU64::new(1);

/// Buffer de pantalla de Android (DMA-BUF). Android lo vuelve a usar unos 5 cuadros después.
pub struct Buffer {
    /// Número único: un buffer nuevo nunca se confunde con uno anterior del mismo id.
    pub serie: u64,
    archivo: File,
    mapa: Mmap,
    pub ancho: usize,
    pub alto: usize,
    pub stride: usize,
    pub bgra: bool,
    /// Modificador DRM (0 = lineal).
    pub modificador: u64,
}

impl Buffer {
    pub fn fd(&self) -> RawFd {
        self.archivo.as_raw_fd()
    }

    /// Copia los píxeles a la CPU en RGBA (opaco). Solo para capturas y el camino sin GPU.
    pub fn leer(&self) -> Vec<Color32> {
        let mut pixeles = Vec::with_capacity(self.ancho * self.alto);
        self.con_mapa(|mapa| {
            for y in 0..self.alto {
                let fila = &mapa[y * self.stride..y * self.stride + self.ancho * 4];
                pixeles.extend(fila.chunks_exact(4).map(|p| self.color(p)));
            }
        });
        pixeles
    }

    /// Copia reducida a `ancho` píxeles de ancho (miniaturas): lee solo los píxeles que usa.
    pub fn reducir(&self, ancho: usize) -> ColorImage {
        let (w, h) = (ancho, (ancho * self.alto / self.ancho.max(1)).max(1));
        let mut pixeles = Vec::with_capacity(w * h);
        self.con_mapa(|mapa| {
            for y in 0..h {
                let fila = (y * self.alto / h) * self.stride;
                pixeles.extend((0..w).map(|x| {
                    let i = fila + (x * self.ancho / w) * 4;
                    self.color(&mapa[i..i + 4])
                }));
            }
        });
        ColorImage::new([w, h], pixeles)
    }

    fn color(&self, p: &[u8]) -> Color32 {
        // La pantalla es opaca: el cuarto byte no se usa.
        if self.bgra {
            Color32::from_rgb(p[2], p[1], p[0])
        } else {
            Color32::from_rgb(p[0], p[1], p[2])
        }
    }

    fn con_mapa(&self, f: impl FnOnce(&[u8])) {
        sync(self.fd(), SYNC_START | SYNC_READ);
        f(&self.mapa);
        sync(self.fd(), SYNC_END | SYNC_READ);
    }
}

/// Cuadro recibido: el buffer donde está y, si la ventana no puede leerlo desde la GPU, la copia.
#[derive(Clone)]
pub struct Cuadro {
    pub buffer: Arc<Buffer>,
    pub copia: Option<ColorImage>,
    pub llegada: Instant,
    /// Cuándo llegó el aviso del servidor, antes de esperar a que Android termine de dibujarlo.
    pub recibido: Instant,
}

impl Cuadro {
    pub fn ancho(&self) -> usize {
        self.buffer.ancho
    }

    pub fn alto(&self) -> usize {
        self.buffer.alto
    }

    /// Copia reducida a `ancho` píxeles de ancho (miniaturas del selector de apps).
    pub fn miniatura(&self, ancho: usize) -> ColorImage {
        match &self.copia {
            Some(c) => {
                let (w, h) = (ancho, (ancho * c.size[1] / c.size[0].max(1)).max(1));
                let pixeles = (0..w * h).map(|i| c.pixels[(i / w * c.size[1] / h) * c.size[0] + (i % w) * c.size[0] / w]);
                ColorImage::new([w, h], pixeles.collect())
            }
            None => self.buffer.reducir(ancho),
        }
    }

    /// Los píxeles en la CPU (capturas de pantalla).
    pub fn imagen(&self) -> ColorImage {
        self.copia.clone().unwrap_or_else(|| ColorImage::new([self.ancho(), self.alto()], self.buffer.leer()))
    }
}

/// La ventana lo activa si no puede mostrar los buffers directo desde la GPU: entonces el hilo de
/// video copia cada cuadro (como antes, más lento).
pub static COPIAR: AtomicBool = AtomicBool::new(false);

/// Cuadros recibidos que la ventana todavía no mostró, del más viejo al más nuevo.
///
/// Android (60 Hz) y el monitor (60.02 Hz aquí) no van sincronizados y los cuadros llegan con
/// irregularidad. Cuando su desfase cae justo donde la ventana toma el cuadro (unos 20 s de cada
/// ~47), llegan dos dentro del mismo refresco y luego ninguno. Guardando solo el último se perdía
/// uno de cada par: hasta 20 por segundo. Con la cola, el segundo se muestra en el refresco siguiente.
pub type Compartido = Arc<Mutex<VecDeque<Cuadro>>>;

/// Hay cuadros en la cola que la ventana todavía no mostró (se cambia con la cola tomada).
pub static NUEVO: AtomicBool = AtomicBool::new(false);

/// Cuadros como máximo en la cola: si se acumulan más, se tiran los viejos para no sumar retraso.
const MAX_COLA: usize = 2;

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
    let mut buffers: HashMap<u32, Arc<Buffer>> = HashMap::new();
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
                let modificador = if n >= 44 { u64::from_be_bytes(d[36..44].try_into().unwrap()) } else { 0 };
                let archivo = File::from(fds.remove(0));
                let mapa = unsafe { MmapOptions::new().len(tam).map(&archivo)? };
                let serie = SERIE.fetch_add(1, Ordering::Relaxed);
                let buffer = Buffer { serie, archivo, mapa, ancho, alto, stride, bgra: formato == 5, modificador };
                buffers.insert(id, Arc::new(buffer));
            }
            b"DMBX" => {
                let recibido = Instant::now();
                // Si trae una fence, esperar (máx. 50 ms) a que Android termine de dibujar.
                if let Some(fence) = fds.first() {
                    let mut pfd = [PollFd::new(fence.as_fd(), PollFlags::POLLIN)];
                    let _ = poll(&mut pfd, PollTimeout::from(50u8));
                }
                let Some(b) = buffers.get(&id) else { continue };
                let copia = COPIAR.load(Ordering::Relaxed).then(|| ColorImage::new([b.ancho, b.alto], b.leer()));
                let mut cola = destino.lock().unwrap();
                cola.push_back(Cuadro { buffer: b.clone(), copia, llegada: Instant::now(), recibido });
                while cola.len() > MAX_COLA {
                    cola.pop_front();
                }
                NUEVO.store(true, Ordering::Relaxed);
                drop(cola);
                nuevo();
            }
            _ => {}
        }
    }
}
