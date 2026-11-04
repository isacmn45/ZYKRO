//! Lectura directa del mouse (evdev) para la puntería, como SwagPlayer: cada reporte del mouse se
//! manda a Android al instante (con un mouse de 1000 Hz, un toque por milisegundo), sin esperar a
//! que la ventana se vuelva a dibujar.

use std::fs::File;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{PointerButton, Vec2};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

use crate::mapeo::Motor;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const REL_X: u16 = 0;
const REL_Y: u16 = 1;
const TAM_EVENTO: usize = 24; // struct input_event en 64 bits

/// La puntería va directo del mouse a Android (copia de `activo` para el bucle de la ventana, que
/// entonces descarta el movimiento crudo del mouse).
pub static DIRECTO: AtomicBool = AtomicBool::new(false);

/// Un mouse abierto y lo que lleva de su reporte actual.
struct Raton {
    nodo: String,
    archivo: File,
    pendiente: Vec2,
    botones: Vec<(PointerButton, bool)>,
}

/// `activo`: la ventana lo prende mientras la puntería está activa con el cursor bloqueado.
/// `listo`: el hilo lo prende si pudo abrir algún mouse (si no, la ventana usa sus propios eventos).
pub fn iniciar(motor: Arc<Mutex<Motor>>, activo: Arc<AtomicBool>, listo: Arc<AtomicBool>) {
    thread::Builder::new()
        .name("zykro-raton".into())
        .spawn(move || {
            let mut ratones: Vec<Raton> = Vec::new();
            let mut revisar = Instant::now();
            let mut buf = [0u8; TAM_EVENTO * 64];
            loop {
                // Cada rato se buscan los mouse que conectes. Los abiertos no se cierran: al cerrarlos
                // y reabrirlos se perdían los movimientos y clics de ese momento (la mira daba un salto).
                if Instant::now() >= revisar {
                    abrir_nuevos(&mut ratones);
                    listo.store(!ratones.is_empty(), Ordering::Relaxed);
                    revisar = Instant::now() + Duration::from_secs(5);
                }
                if ratones.is_empty() {
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }
                let listos: Vec<bool> = {
                    let mut fds: Vec<PollFd> = ratones.iter().map(|r| PollFd::new(r.archivo.as_fd(), PollFlags::POLLIN)).collect();
                    if poll(&mut fds, PollTimeout::from(200u8)).is_err() {
                        continue;
                    }
                    // También los que se desconectaron: al leerlos dan error y se quitan.
                    let aviso = PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL;
                    fds.iter().map(|f| f.revents().is_some_and(|r| r.intersects(aviso))).collect()
                };
                let mut desconectados = Vec::new();
                for (i, listo_i) in listos.into_iter().enumerate() {
                    if !listo_i {
                        continue;
                    }
                    let raton = &mut ratones[i];
                    let n = match raton.archivo.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(_) => {
                            desconectados.push(i);
                            continue;
                        }
                    };
                    for ev in buf[..n].chunks_exact(TAM_EVENTO) {
                        let tipo = u16::from_ne_bytes([ev[16], ev[17]]);
                        let codigo = u16::from_ne_bytes([ev[18], ev[19]]);
                        let valor = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
                        match (tipo, codigo) {
                            (EV_REL, REL_X) => raton.pendiente.x += valor as f32,
                            (EV_REL, REL_Y) => raton.pendiente.y += valor as f32,
                            // 1 = presionar, 0 = soltar; 2 es una repetición y se ignora.
                            (EV_KEY, c) if valor != 2 => {
                                if let Some(b) = boton(c) {
                                    raton.botones.push((b, valor == 1));
                                }
                            }
                            (EV_SYN, _) => {
                                let delta = std::mem::take(&mut raton.pendiente);
                                let clics = std::mem::take(&mut raton.botones);
                                if activo.load(Ordering::Relaxed) && (delta != Vec2::ZERO || !clics.is_empty()) {
                                    let mut m = motor.lock().unwrap();
                                    if delta != Vec2::ZERO {
                                        m.mover_directo(delta);
                                    }
                                    for (b, presionado) in clics {
                                        m.boton_directo(b, presionado);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Los que se desconectaron (de atrás para adelante, para no correr los índices).
                for i in desconectados.into_iter().rev() {
                    ratones.remove(i);
                }
            }
        })
        .expect("no se pudo crear el hilo del mouse");
}

fn boton(codigo: u16) -> Option<PointerButton> {
    Some(match codigo {
        0x110 => PointerButton::Primary,
        0x111 => PointerButton::Secondary,
        0x112 => PointerButton::Middle,
        0x113 | 0x116 => PointerButton::Extra1, // BTN_SIDE / BTN_BACK
        0x114 | 0x115 => PointerButton::Extra2, // BTN_EXTRA / BTN_FORWARD
        _ => return None,
    })
}

/// Abre los dispositivos que se mueven en X e Y (mouse y touchpads) que todavía no estén abiertos,
/// sin los virtuales de SwagPlayer.
fn abrir_nuevos(ratones: &mut Vec<Raton>) {
    let Ok(lista) = std::fs::read_to_string("/proc/bus/input/devices") else { return };
    for bloque in lista.split("\n\n") {
        let nodo = (|| {
            let nombre = bloque.lines().find_map(|l| l.strip_prefix("N: Name="))?;
            if nombre.to_lowercase().contains("swag") {
                return None;
            }
            let rel = bloque.lines().find_map(|l| l.strip_prefix("B: REL="))?;
            let bits = u64::from_str_radix(rel.split_whitespace().last()?, 16).ok()?;
            if bits & 0b11 != 0b11 {
                return None;
            }
            bloque.lines().find_map(|l| l.strip_prefix("H: Handlers="))?.split_whitespace().find(|h| h.starts_with("event")).map(String::from)
        })();
        let Some(nodo) = nodo.filter(|n| !ratones.iter().any(|r| &r.nodo == n)) else { continue };
        let archivo = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open(format!("/dev/input/{nodo}"));
        if let Ok(archivo) = archivo {
            ratones.push(Raton { nodo, archivo, pendiente: Vec2::ZERO, botones: Vec::new() });
        }
    }
}

#[cfg(test)]
mod pruebas {
    /// Los mouse que abriría el hilo (sin leerlos): `cargo test ratones_conectados -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn ratones_conectados() {
        let mut ratones = Vec::new();
        super::abrir_nuevos(&mut ratones);
        let antes = ratones.len();
        // Buscar otra vez no duplica los que ya están abiertos.
        super::abrir_nuevos(&mut ratones);
        assert_eq!(ratones.len(), antes);
        for r in &ratones {
            println!("{}", r.nodo);
        }
    }
}
