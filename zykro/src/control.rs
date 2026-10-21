//! Canal de control: mensajes que el cliente manda a swag-rust-server por `control.sock`.
//! Tecla (0x00), toque (0x02) y scroll (0x03) son los de scrcpy; 0x12 y 0x14 son propios.
//! Formato completo en docs/investigacion.md.

use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Sender};
use std::thread;

pub const BAJAR: u8 = 0;
pub const SOLTAR: u8 = 1;
pub const MOVER: u8 = 2;

pub const META_SHIFT: u32 = 0x1;
pub const META_ALT: u32 = 0x2;
pub const META_CTRL: u32 = 0x1000;

#[derive(Clone)]
pub struct Control {
    tx: Sender<Vec<u8>>,
}

impl Control {
    pub fn conectar(ruta: &str, lod_activado: bool, lod: f32) -> io::Result<Control> {
        let mut socket = UnixStream::connect(ruta)?;
        let layout = b"keyboard_layout_english_us";
        let mut inicio = vec![0x12, layout.len() as u8];
        inicio.extend_from_slice(layout);
        // 0x14: LOD bias activado y su valor.
        inicio.push(0x14);
        inicio.push(lod_activado as u8);
        inicio.extend_from_slice(&lod.clamp(0.0, 4.0).to_be_bytes());
        socket.write_all(&inicio)?;

        // Un hilo escribe para que la interfaz nunca se bloquee esperando al socket.
        let (tx, rx) = channel::<Vec<u8>>();
        thread::Builder::new().name("zykro-control".into()).spawn(move || {
            for mensaje in rx {
                if socket.write_all(&mensaje).is_err() {
                    break;
                }
            }
        })?;
        Ok(Control { tx })
    }

    fn enviar(&self, mensaje: Vec<u8>) {
        let _ = self.tx.send(mensaje);
    }

    pub fn tecla(&self, accion: u8, codigo: u32, meta: u32) {
        let mut m = Vec::with_capacity(14);
        m.push(0x00);
        m.push(accion);
        m.extend_from_slice(&codigo.to_be_bytes());
        m.extend_from_slice(&0u32.to_be_bytes()); // repeticiones
        m.extend_from_slice(&meta.to_be_bytes());
        self.enviar(m);
    }

    /// Presiona y suelta una tecla de Android (Atrás, Inicio, volumen...).
    pub fn pulsar(&self, codigo: u32) {
        self.tecla(BAJAR, codigo, 0);
        self.tecla(SOLTAR, codigo, 0);
    }

    pub fn toque(&self, accion: u8, dedo: u64, x: i32, y: i32, ancho: u16, alto: u16) {
        let presion: u16 = if accion == SOLTAR { 0 } else { 0xFFFF };
        let mut m = Vec::with_capacity(32);
        m.push(0x02);
        m.push(accion);
        m.extend_from_slice(&dedo.to_be_bytes());
        m.extend_from_slice(&x.to_be_bytes());
        m.extend_from_slice(&y.to_be_bytes());
        m.extend_from_slice(&ancho.to_be_bytes());
        m.extend_from_slice(&alto.to_be_bytes());
        m.extend_from_slice(&presion.to_be_bytes());
        m.extend_from_slice(&0u32.to_be_bytes()); // botón de acción
        m.extend_from_slice(&0u32.to_be_bytes()); // botones
        self.enviar(m);
    }

    /// `h` y `v` van de -1.0 a 1.0.
    pub fn scroll(&self, x: i32, y: i32, ancho: u16, alto: u16, h: f32, v: f32) {
        let a_i16 = |f: f32| (f.clamp(-1.0, 1.0) * 32767.0) as i16;
        let mut m = Vec::with_capacity(21);
        m.push(0x03);
        m.extend_from_slice(&x.to_be_bytes());
        m.extend_from_slice(&y.to_be_bytes());
        m.extend_from_slice(&ancho.to_be_bytes());
        m.extend_from_slice(&alto.to_be_bytes());
        m.extend_from_slice(&a_i16(h).to_be_bytes());
        m.extend_from_slice(&a_i16(v).to_be_bytes());
        m.extend_from_slice(&0u32.to_be_bytes());
        self.enviar(m);
    }
}
