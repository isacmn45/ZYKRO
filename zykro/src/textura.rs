//! Textura con la pantalla de Android. Cada buffer DMA-BUF se importa una vez a OpenGL (EGL
//! `EGL_EXT_image_dma_buf_import`) y se dibuja directo, sin copiar nada por la CPU; así lo hace
//! también SwagPlayer. Si el sistema no lo permite (X11 con GLX, drivers sin la extensión), se
//! vuelve a copiar cada cuadro y subirlo como textura normal.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use eframe::egui::{self, TextureId, TextureOptions};
use eframe::glow::{self, HasContext};

use crate::video::{self, Buffer, Cuadro};

type Ptr = *mut c_void;

const EGL_EXTENSIONS: i32 = 0x3055;
const EGL_NONE: i32 = 0x3038;
const EGL_WIDTH: i32 = 0x3057;
const EGL_HEIGHT: i32 = 0x3056;
const EGL_LINUX_DMA_BUF: u32 = 0x3270;
const EGL_LINUX_DRM_FOURCC: i32 = 0x3271;
const EGL_DMA_BUF_PLANE0_FD: i32 = 0x3272;
const EGL_DMA_BUF_PLANE0_OFFSET: i32 = 0x3273;
const EGL_DMA_BUF_PLANE0_PITCH: i32 = 0x3274;
const EGL_DMA_BUF_PLANE0_MODIFIER_LO: i32 = 0x3443;
const EGL_DMA_BUF_PLANE0_MODIFIER_HI: i32 = 0x3444;
const DRM_FORMAT_MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

/// Formatos DRM sin canal alfa (la pantalla es opaca): RGBA_8888 de Android es XBGR8888.
const fn fourcc(c: &[u8; 4]) -> i32 {
    (c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16 | (c[3] as u32) << 24) as i32
}
const XBGR8888: i32 = fourcc(b"XB24");
const XRGB8888: i32 = fourcc(b"XR24");

pub struct Pantalla {
    gpu: Option<Importador>,
    probado: bool,
    cpu: Option<egui::TextureHandle>,
    id: Option<TextureId>,
}

impl Pantalla {
    pub fn new() -> Self {
        Pantalla { gpu: None, probado: false, cpu: None, id: None }
    }

    /// Textura lista para dibujar (ninguna hasta el primer cuadro).
    pub fn id(&self) -> Option<TextureId> {
        self.id
    }

    pub fn mostrar(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame, cuadro: &mut Cuadro) {
        if !self.probado {
            self.probado = true;
            self.gpu = Importador::nuevo(frame);
            if self.gpu.is_none() {
                eprintln!("[video] sin importación DMA-BUF en OpenGL: se copia cada cuadro");
                video::COPIAR.store(true, Ordering::Relaxed);
            }
        }
        if let (Some(gpu), Some(gl)) = (&mut self.gpu, frame.gl().cloned()) {
            match gpu.textura(&gl, frame, &cuadro.buffer) {
                Ok(id) => {
                    self.id = Some(id);
                    return;
                }
                Err(e) => {
                    eprintln!("[video] no se pudo importar el buffer ({e}): se copia cada cuadro");
                    self.gpu = None;
                    video::COPIAR.store(true, Ordering::Relaxed);
                }
            }
        }
        let imagen = cuadro.copia.take().unwrap_or_else(|| cuadro.imagen());
        match &mut self.cpu {
            Some(t) => t.set(imagen, TextureOptions::LINEAR),
            None => self.cpu = Some(ctx.load_texture("android", imagen, TextureOptions::LINEAR)),
        }
        self.id = self.cpu.as_ref().map(|t| t.id());
    }
}

struct Egl {
    display: Ptr,
    crear: unsafe extern "C" fn(Ptr, Ptr, u32, Ptr, *const i32) -> Ptr,
    destruir: unsafe extern "C" fn(Ptr, Ptr) -> u32,
    error: unsafe extern "C" fn() -> i32,
    asignar: unsafe extern "C" fn(u32, Ptr),
    modificadores: bool,
}

/// Una textura de OpenGL por buffer de Android (son 6), creada la primera vez que se muestra.
struct Importador {
    egl: Egl,
    texturas: HashMap<u64, Importada>,
}

struct Importada {
    buffer: Weak<Buffer>,
    imagen: Ptr,
    textura: glow::Texture,
    id: TextureId,
}

impl Importador {
    fn nuevo(frame: &eframe::Frame) -> Option<Importador> {
        frame.gl()?;
        let egl = unsafe { cargar_egl()? };
        Some(Importador { egl, texturas: HashMap::new() })
    }

    fn textura(&mut self, gl: &glow::Context, frame: &mut eframe::Frame, b: &Arc<Buffer>) -> Result<TextureId, String> {
        // Buffers que Android ya no usa (se reconectó o cambió la resolución).
        let muertos: Vec<u64> = self.texturas.iter().filter(|(_, t)| t.buffer.strong_count() == 0).map(|(s, _)| *s).collect();
        for serie in muertos {
            let t = self.texturas.remove(&serie).unwrap();
            // El id de egui no se puede liberar; la textura queda vacía y no se vuelve a usar.
            unsafe {
                (self.egl.destruir)(self.egl.display, t.imagen);
                gl.delete_texture(t.textura);
            }
        }
        if let Some(t) = self.texturas.get(&b.serie) {
            return Ok(t.id);
        }
        let imagen = self.importar(b)?;
        let textura = unsafe {
            let textura = gl.create_texture().inspect_err(|_| {
                (self.egl.destruir)(self.egl.display, imagen);
            })?;
            gl.bind_texture(glow::TEXTURE_2D, Some(textura));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            while gl.get_error() != glow::NO_ERROR {}
            (self.egl.asignar)(glow::TEXTURE_2D, imagen);
            let error = gl.get_error();
            gl.bind_texture(glow::TEXTURE_2D, None);
            if error != glow::NO_ERROR {
                gl.delete_texture(textura);
                (self.egl.destruir)(self.egl.display, imagen);
                return Err(format!("glEGLImageTargetTexture2DOES: error 0x{error:x}"));
            }
            textura
        };
        let id = frame.register_native_glow_texture(textura);
        self.texturas.insert(b.serie, Importada { buffer: Arc::downgrade(b), imagen, textura, id });
        Ok(id)
    }

    fn importar(&self, b: &Buffer) -> Result<Ptr, String> {
        let mut atributos = vec![
            EGL_WIDTH, b.ancho as i32,
            EGL_HEIGHT, b.alto as i32,
            EGL_LINUX_DRM_FOURCC, if b.bgra { XRGB8888 } else { XBGR8888 },
            EGL_DMA_BUF_PLANE0_FD, b.fd(),
            EGL_DMA_BUF_PLANE0_OFFSET, 0,
            EGL_DMA_BUF_PLANE0_PITCH, b.stride as i32,
        ];
        if self.egl.modificadores && b.modificador != DRM_FORMAT_MOD_INVALID {
            atributos.extend([
                EGL_DMA_BUF_PLANE0_MODIFIER_LO, b.modificador as u32 as i32,
                EGL_DMA_BUF_PLANE0_MODIFIER_HI, (b.modificador >> 32) as u32 as i32,
            ]);
        }
        atributos.push(EGL_NONE);
        let imagen = unsafe { (self.egl.crear)(self.egl.display, std::ptr::null_mut(), EGL_LINUX_DMA_BUF, std::ptr::null_mut(), atributos.as_ptr()) };
        if imagen.is_null() {
            return Err(format!("eglCreateImageKHR: error 0x{:x}", unsafe { (self.egl.error)() }));
        }
        Ok(imagen)
    }
}

/// Funciones de EGL de la biblioteca que ya cargó eframe. None si la ventana no usa EGL o falta
/// alguna extensión.
unsafe fn cargar_egl() -> Option<Egl> {
    let lib = libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_NOW);
    if lib.is_null() {
        return None;
    }
    let simbolo = |nombre: &CStr| libc::dlsym(lib, nombre.as_ptr());
    let proc_address: unsafe extern "C" fn(*const c_char) -> Ptr = std::mem::transmute(no_nulo(simbolo(c"eglGetProcAddress"))?);
    let actual: unsafe extern "C" fn() -> Ptr = std::mem::transmute(no_nulo(simbolo(c"eglGetCurrentDisplay"))?);
    let consultar: unsafe extern "C" fn(Ptr, i32) -> *const c_char = std::mem::transmute(no_nulo(simbolo(c"eglQueryString"))?);
    let error: unsafe extern "C" fn() -> i32 = std::mem::transmute(no_nulo(simbolo(c"eglGetError"))?);
    let display = no_nulo(actual())?;
    let extensiones = no_nulo(consultar(display, EGL_EXTENSIONS) as Ptr)?;
    let extensiones = CStr::from_ptr(extensiones as *const c_char).to_string_lossy();
    let tiene = |e: &str| extensiones.split(' ').any(|x| x == e);
    if !tiene("EGL_EXT_image_dma_buf_import") {
        return None;
    }
    let funcion = |nombre: &CStr| no_nulo(proc_address(nombre.as_ptr()));
    Some(Egl {
        display,
        crear: std::mem::transmute(funcion(c"eglCreateImageKHR")?),
        destruir: std::mem::transmute(funcion(c"eglDestroyImageKHR")?),
        error,
        asignar: std::mem::transmute(funcion(c"glEGLImageTargetTexture2DOES")?),
        modificadores: tiene("EGL_EXT_image_dma_buf_import_modifiers"),
    })
}

fn no_nulo(p: Ptr) -> Option<Ptr> {
    (!p.is_null()).then_some(p)
}
