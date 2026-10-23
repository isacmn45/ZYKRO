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

/// `activo`: la ventana lo prende mientras la puntería está activa con el cursor bloqueado.
/// `listo`: el hilo lo prende si pudo abrir algún mouse (si no, la ventana usa sus propios eventos).
pub fn iniciar(motor: Arc<Mutex<Motor>>, activo: Arc<AtomicBool>, listo: Arc<AtomicBool>) {
    thread::Builder::new()
        .name("zykro-raton".into())
        .spawn(move || loop {
            let mut ratones = abrir_ratones();
            listo.store(!ratones.is_empty(), Ordering::Relaxed);
            if ratones.is_empty() {
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            // Se vuelven a buscar los mouse cada rato por si conectas o desconectas uno.
            let revisar = Instant::now() + Duration::from_secs(5);
            let mut pendiente = vec![Vec2::ZERO; ratones.len()];
            let mut botones: Vec<Vec<(PointerButton, bool)>> = vec![Vec::new(); ratones.len()];
            let mut buf = [0u8; TAM_EVENTO * 64];
            while Instant::now() < revisar {
                let apuntando = activo.load(Ordering::Relaxed);
                let listos: Vec<bool> = {
                    let mut fds: Vec<PollFd> = ratones.iter().map(|f| PollFd::new(f.as_fd(), PollFlags::POLLIN)).collect();
                    if poll(&mut fds, PollTimeout::from(200u8)).is_err() {
                        break;
                    }
                    fds.iter().map(|f| f.revents().is_some_and(|r| r.contains(PollFlags::POLLIN))).collect()
                };
                for (i, listo_i) in listos.into_iter().enumerate() {
                    if !listo_i {
                        continue;
                    }
                    let Ok(n) = ratones[i].read(&mut buf) else { continue };
                    for ev in buf[..n].chunks_exact(TAM_EVENTO) {
                        let tipo = u16::from_ne_bytes([ev[16], ev[17]]);
                        let codigo = u16::from_ne_bytes([ev[18], ev[19]]);
                        let valor = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
                        match (tipo, codigo) {
                            (EV_REL, REL_X) => pendiente[i].x += valor as f32,
                            (EV_REL, REL_Y) => pendiente[i].y += valor as f32,
                            // 1 = presionar, 0 = soltar; 2 es una repetición y se ignora.
                            (EV_KEY, c) if valor != 2 => {
                                if let Some(b) = boton(c) {
                                    botones[i].push((b, valor == 1));
                                }
                            }
                            (EV_SYN, _) => {
                                let delta = std::mem::take(&mut pendiente[i]);
                                let clics = std::mem::take(&mut botones[i]);
                                if apuntando && (delta != Vec2::ZERO || !clics.is_empty()) {
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
            }
            ratones.clear();
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

/// Dispositivos que se mueven en X e Y (mouse y touchpads), sin los virtuales de SwagPlayer.
fn abrir_ratones() -> Vec<File> {
    let Ok(lista) = std::fs::read_to_string("/proc/bus/input/devices") else { return Vec::new() };
    lista
        .split("\n\n")
        .filter_map(|bloque| {
            let nombre = bloque.lines().find_map(|l| l.strip_prefix("N: Name="))?;
            if nombre.to_lowercase().contains("swag") {
                return None;
            }
            let rel = bloque.lines().find_map(|l| l.strip_prefix("B: REL="))?;
            let bits = u64::from_str_radix(rel.split_whitespace().last()?, 16).ok()?;
            if bits & 0b11 != 0b11 {
                return None;
            }
            let evento = bloque
                .lines()
                .find_map(|l| l.strip_prefix("H: Handlers="))?
                .split_whitespace()
                .find(|h| h.starts_with("event"))?;
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(format!("/dev/input/{evento}"))
                .ok()
        })
        .collect()
}
