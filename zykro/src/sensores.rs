//! Temperatura de la CPU y la GPU para la barra de título. Se lee cada 2 s en un hilo.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::rendimiento::{self, Fabricante};

#[derive(Clone, Copy, Default)]
pub struct Temperaturas {
    pub cpu: Option<f32>,
    pub gpu: Option<f32>,
    /// GPU integrada sin sensor propio: comparte chip (y temperatura) con la CPU.
    pub gpu_en_cpu: bool,
}

pub type Compartidas = Arc<Mutex<Temperaturas>>;

pub fn iniciar() -> Compartidas {
    let temperaturas = Compartidas::default();
    let destino = temperaturas.clone();
    thread::Builder::new()
        .name("zykro-sensores".into())
        .spawn(move || {
            let cpu = sensor_cpu();
            let gpu = rendimiento::gpu();
            let sensor_gpu = gpu.as_ref().and_then(|g| hwmon_temp(&g.tarjeta.join("device/hwmon")));
            loop {
                let mut t = Temperaturas { cpu: cpu.as_deref().and_then(leer_milicelsius), ..Default::default() };
                match &gpu {
                    Some(g) if g.fabricante == Fabricante::Nvidia => t.gpu = nvidia_smi(),
                    Some(g) => {
                        t.gpu = sensor_gpu.as_deref().and_then(leer_milicelsius);
                        t.gpu_en_cpu = t.gpu.is_none() && g.integrada;
                    }
                    None => {}
                }
                *destino.lock().unwrap() = t;
                thread::sleep(Duration::from_secs(2));
            }
        })
        .expect("no se pudo crear el hilo de sensores");
    temperaturas
}

/// Sensor del paquete del procesador: coretemp (Intel), k10temp o zenpower (AMD).
fn sensor_cpu() -> Option<PathBuf> {
    let hwmon = std::fs::read_dir("/sys/class/hwmon").ok()?;
    for entrada in hwmon.flatten() {
        let nombre = std::fs::read_to_string(entrada.path().join("name")).unwrap_or_default();
        if ["coretemp", "k10temp", "zenpower"].contains(&nombre.trim()) {
            return Some(entrada.path().join("temp1_input"));
        }
    }
    // Sin hwmon: la zona térmica del paquete x86.
    std::fs::read_dir("/sys/class/thermal").ok()?.flatten().map(|e| e.path()).find_map(|p| {
        let tipo = std::fs::read_to_string(p.join("type")).unwrap_or_default();
        (tipo.trim() == "x86_pkg_temp").then(|| p.join("temp"))
    })
}

/// Primer temp1_input bajo un directorio hwmon de la GPU (AMD, Arc...).
fn hwmon_temp(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path().join("temp1_input")).find(|p| p.exists())
}

fn leer_milicelsius(ruta: &Path) -> Option<f32> {
    std::fs::read_to_string(ruta).ok()?.trim().parse::<f32>().ok().map(|m| m / 1000.0)
}

fn nvidia_smi() -> Option<f32> {
    let salida = Command::new("nvidia-smi").args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"]).output().ok()?;
    String::from_utf8_lossy(&salida.stdout).lines().next()?.trim().parse().ok()
}
