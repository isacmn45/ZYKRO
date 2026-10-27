//! Modos de rendimiento. Cada uno cambia cosas reales del sistema:
//! - Perfil de energía (power-profiles-daemon): cuánto puede gastar el procesador, el ventilador
//!   y los límites de potencia de la placa.
//! - CPU para Android: núcleos y prioridad del contenedor (`docker update`, en caliente).
//! - GPU, solo en Ultra: frecuencia mínima al máximo (Intel) o nivel «high» (AMD), para que no baje
//!   de velocidad en pleno juego. Necesita el permiso que instala `pedir_permiso_gpu` (una vez).
//!
//! Al cerrar ZYKRO se deja todo como estaba (`restaurar`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::android::CONTENEDOR;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub enum Modo {
    Ahorro,
    #[default]
    Equilibrado,
    Rendimiento,
    Ultra,
}

impl Modo {
    pub const TODOS: [Modo; 4] = [Modo::Ahorro, Modo::Equilibrado, Modo::Rendimiento, Modo::Ultra];

    pub fn nombre(self) -> &'static str {
        match self {
            Modo::Ahorro => "Ahorro de energía",
            Modo::Equilibrado => "Equilibrado",
            Modo::Rendimiento => "Rendimiento",
            Modo::Ultra => "Ultra rendimiento",
        }
    }

    pub fn descripcion(self) -> &'static str {
        match self {
            Modo::Ahorro => "Menos calor y más batería. Android usa la mitad de los núcleos.",
            Modo::Equilibrado => "Lo normal, pero la GPU no baja de la mitad de su velocidad (evita tirones).",
            Modo::Rendimiento => "CPU y ventilador más fuertes, Android con prioridad; GPU a 3/4 como mínimo.",
            Modo::Ultra => "Todo al máximo: además la GPU no baja nunca de velocidad.",
        }
    }

    fn perfil(self) -> &'static str {
        match self {
            Modo::Ahorro => "power-saver",
            Modo::Equilibrado => "balanced",
            Modo::Rendimiento | Modo::Ultra => "performance",
        }
    }

    /// Peso de CPU del contenedor frente a otros programas (1024 es lo normal).
    fn prioridad(self) -> u32 {
        match self {
            Modo::Ahorro => 512,
            Modo::Equilibrado => 1024,
            Modo::Rendimiento => 2048,
            Modo::Ultra => 4096,
        }
    }

    fn nucleos(self) -> usize {
        let total = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        match self {
            Modo::Ahorro => (total / 2).max(2),
            _ => total,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// GPU
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Fabricante {
    Intel,
    Amd,
    Nvidia,
    Otro,
}

/// La GPU con la que dibuja Android (la de /dev/dri/renderD128).
#[derive(Clone)]
pub struct Gpu {
    pub nombre: String,
    pub fabricante: Fabricante,
    pub integrada: bool,
    /// /sys/class/drm/cardN
    pub tarjeta: PathBuf,
}

/// Archivo que fija la velocidad mínima de la GPU.
struct ControlGpu {
    archivo: PathBuf,
    tipo: TipoGpu,
}

enum TipoGpu {
    /// Velocidades en MHz: la mínima del sistema y la máxima.
    Intel { minima: u32, maxima: u32 },
    /// `power_dpm_force_performance_level`: "high" o "auto".
    Amd,
}

impl ControlGpu {
    /// Qué escribir con cada modo. En Intel, con la velocidad que el sistema elige sola, Free Fire
    /// tardaba de 6 a 29 ms por cuadro (la GPU saltaba entre 100 y 650 MHz) y se veía a tirones aunque
    /// marcara 60 FPS; con un mínimo de 600 MHz o más, cada cuadro llega a los 16–17 ms. Sin trabajo la
    /// GPU igual se apaga (RC6), así que el mínimo casi no gasta en reposo.
    fn valor(&self, modo: Modo) -> String {
        match self.tipo {
            TipoGpu::Intel { minima, maxima } => {
                let piso = match modo {
                    Modo::Ahorro => minima,
                    Modo::Equilibrado => maxima / 2,
                    Modo::Rendimiento => maxima * 3 / 4,
                    Modo::Ultra => maxima,
                };
                piso.max(minima).to_string()
            }
            TipoGpu::Amd => if modo == Modo::Ultra { "high" } else { "auto" }.into(),
        }
    }

    /// Lo que el sistema tenía antes de ZYKRO.
    fn normal(&self) -> String {
        match self.tipo {
            TipoGpu::Intel { minima, .. } => minima.to_string(),
            TipoGpu::Amd => "auto".into(),
        }
    }

    /// El modo cambia la velocidad mínima (si no, se deja la del sistema).
    fn ajusta(&self, modo: Modo) -> bool {
        self.valor(modo) != self.normal()
    }
}

pub fn gpu() -> Option<Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(detectar_gpu).clone()
}

fn detectar_gpu() -> Option<Gpu> {
    let dispositivo = std::fs::canonicalize("/sys/class/drm/renderD128/device").ok()?;
    let tarjeta = std::fs::read_dir("/sys/class/drm")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("card") && !n.contains('-')))
        .find(|p| std::fs::canonicalize(p.join("device")).ok().as_ref() == Some(&dispositivo))?;
    let leer = |nombre: &str| std::fs::read_to_string(dispositivo.join(nombre)).unwrap_or_default().trim().to_string();
    let fabricante = match leer("vendor").as_str() {
        "0x8086" => Fabricante::Intel,
        "0x1002" => Fabricante::Amd,
        "0x10de" => Fabricante::Nvidia,
        _ => Fabricante::Otro,
    };
    let direccion = dispositivo.file_name()?.to_str()?.to_string();
    let integrada = match fabricante {
        // La integrada de Intel siempre está en 00:02.0; las Arc van en otra ranura.
        Fabricante::Intel => direccion.ends_with("00:02.0"),
        // Las AMD integradas (APU) reservan poca VRAM.
        Fabricante::Amd => leer("mem_info_vram_total").parse::<u64>().map_or(false, |v| v < 1 << 30),
        _ => false,
    };
    let nombre = Command::new("lspci")
        .args(["-mm", "-s", &direccion])
        .output()
        .ok()
        .and_then(|s| String::from_utf8(s.stdout).ok())
        .and_then(|s| s.split('"').nth(5).map(String::from))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| match fabricante {
            Fabricante::Intel => "Intel".into(),
            Fabricante::Amd => "AMD Radeon".into(),
            Fabricante::Nvidia => "NVIDIA".into(),
            Fabricante::Otro => "Desconocida".into(),
        });
    Some(Gpu { nombre, fabricante, integrada, tarjeta })
}

fn control_gpu() -> Option<ControlGpu> {
    let g = gpu()?;
    let leer = |p: &Path| std::fs::read_to_string(p).ok().and_then(|s| s.trim().parse::<u32>().ok());
    match g.fabricante {
        Fabricante::Intel => {
            // i915 (Iris, UHD, Arc antiguas) o xe (Arc nuevas).
            let i915 = g.tarjeta.join("gt_min_freq_mhz");
            if i915.exists() {
                let (minima, maxima) = (leer(&g.tarjeta.join("gt_RPn_freq_mhz"))?, leer(&g.tarjeta.join("gt_RP0_freq_mhz"))?);
                return Some(ControlGpu { archivo: i915, tipo: TipoGpu::Intel { minima, maxima } });
            }
            let xe = g.tarjeta.join("device/tile0/gt0/freq0");
            let (minima, maxima) = (leer(&xe.join("rpn_freq"))?, leer(&xe.join("rp0_freq"))?);
            Some(ControlGpu { archivo: xe.join("min_freq"), tipo: TipoGpu::Intel { minima, maxima } })
        }
        Fabricante::Amd => {
            let archivo = g.tarjeta.join("device/power_dpm_force_performance_level");
            archivo.exists().then_some(ControlGpu { archivo, tipo: TipoGpu::Amd })
        }
        // NVIDIA: lo maneja su driver propietario; no se toca.
        _ => None,
    }
}

/// El modo ajusta la velocidad de la GPU (Intel: todos menos Ahorro; AMD: solo Ultra).
pub fn gpu_ajustable(modo: Modo) -> bool {
    control_gpu().is_some_and(|c| c.ajusta(modo))
}

/// ZYKRO ya puede ajustar la GPU sin contraseña.
pub fn gpu_permitida() -> bool {
    control_gpu().is_some_and(|c| escribible(&c.archivo))
}

fn escribible(ruta: &Path) -> bool {
    let Ok(c) = std::ffi::CString::new(ruta.as_os_str().as_encoded_bytes()) else { return false };
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

/// Regla udev que da a `usuario` los archivos de velocidad de la GPU (i915, xe y AMD).
fn regla_gpu(usuario: &str) -> String {
    format!(
        "# ZYKRO: deja a {usuario} ajustar la velocidad mínima de la GPU (para que no dé tirones).\n\
         SUBSYSTEM==\"drm\", ENV{{DEVTYPE}}==\"drm_minor\", KERNEL==\"card*\", ACTION==\"add|change\", \
         RUN+=\"/bin/sh -c 'for f in gt_min_freq_mhz device/tile0/gt0/freq0/min_freq device/power_dpm_force_performance_level; \
         do [ -e /sys%p/$$f ] && chown {usuario} /sys%p/$$f; done'\"\n"
    )
}

/// Instala una regla udev que deja a este usuario ajustar la velocidad de la GPU. Pide la
/// contraseña una sola vez (pkexec); después la regla se aplica sola en cada arranque.
pub fn pedir_permiso_gpu() -> bool {
    let usuario = std::env::var("USER").unwrap_or_default();
    if usuario.is_empty() || !usuario.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return false;
    }
    let regla = regla_gpu(&usuario);
    let temporal = std::env::temp_dir().join(format!("zykro-gpu-{}.rules", std::process::id()));
    if std::fs::write(&temporal, regla).is_err() {
        return false;
    }
    let orden = format!(
        "install -m 644 '{}' /etc/udev/rules.d/70-zykro-gpu.rules && udevadm control --reload && \
         udevadm trigger --subsystem-match=drm --action=change && udevadm settle",
        temporal.display()
    );
    let ok = Command::new("pkexec").args(["sh", "-c", &orden]).status().is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&temporal);
    ok && gpu_permitida()
}

// ---------------------------------------------------------------------------------------------
// Aplicar y restaurar
// ---------------------------------------------------------------------------------------------

/// Perfil de energía que había antes de que ZYKRO lo cambiara (para devolverlo al cerrar).
static PERFIL_ANTERIOR: OnceLock<Option<String>> = OnceLock::new();

fn perfil_actual() -> Option<String> {
    let salida = Command::new("powerprofilesctl").arg("get").output().ok()?;
    salida.status.success().then(|| String::from_utf8_lossy(&salida.stdout).trim().to_string())
}

fn poner_perfil(perfil: &str) -> bool {
    Command::new("powerprofilesctl").args(["set", perfil]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn cpu_android(nucleos: usize, prioridad: u32) -> bool {
    Command::new("docker")
        .args(["update", "--cpus", &nucleos.to_string(), "--cpu-shares", &prioridad.to_string(), CONTENEDOR])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn poner_gpu(modo: Option<Modo>) -> bool {
    let Some(c) = control_gpu() else { return false };
    std::fs::write(&c.archivo, modo.map_or_else(|| c.normal(), |m| c.valor(m))).is_ok()
}

/// Pone la velocidad mínima de la GPU de ese modo (al recibir el permiso, sin volver a aplicar todo).
pub fn aplicar_gpu(modo: Modo) -> bool {
    gpu_permitida() && poner_gpu(Some(modo))
}

/// Aplica el modo. Devuelve un aviso con lo que no se pudo hacer (vacío si salió todo).
pub fn aplicar(modo: Modo) -> Vec<&'static str> {
    PERFIL_ANTERIOR.get_or_init(perfil_actual);
    let mut fallos = Vec::new();
    if !poner_perfil(modo.perfil()) {
        fallos.push("no se pudo cambiar el perfil de energía (¿está power-profiles-daemon?)");
    }
    if !cpu_android(modo.nucleos(), modo.prioridad()) {
        fallos.push("no se pudo ajustar la CPU de Android");
    }
    if control_gpu().is_some() {
        if gpu_permitida() {
            poner_gpu(Some(modo));
        } else if modo == Modo::Ultra {
            fallos.push("falta el permiso para ajustar la GPU (en Configuración, pestaña Rendimiento)");
        }
    }
    fallos
}

/// Deja el sistema como estaba antes de abrir ZYKRO.
pub fn restaurar() {
    if let Some(Some(perfil)) = PERFIL_ANTERIOR.get() {
        poner_perfil(perfil);
    }
    cpu_android(Modo::Equilibrado.nucleos(), Modo::Equilibrado.prioridad());
    if gpu_permitida() {
        poner_gpu(None);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Muestra la GPU detectada y prueba un modo contra el sistema real (lo deja como estaba):
    /// `cargo test diagnostico -- --ignored --nocapture`
    /// Escribe la regla en el directorio temporal para revisarla con `udevadm verify`:
    /// `cargo test regla -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn regla() {
        let ruta = std::env::temp_dir().join("70-zykro-gpu.rules");
        std::fs::write(&ruta, regla_gpu("usuario")).unwrap();
        println!("{}", ruta.display());
    }

    #[test]
    #[ignore]
    fn diagnostico() {
        let g = gpu().expect("sin GPU");
        println!("GPU: {} integrada={} tarjeta={}", g.nombre, g.integrada, g.tarjeta.display());
        let c = control_gpu();
        println!("permitida={} archivo={:?}", gpu_permitida(), c.as_ref().map(|c| &c.archivo));
        for modo in Modo::TODOS {
            println!("  {}: {:?}", modo.nombre(), c.as_ref().map(|c| c.valor(modo)));
        }
        let antes = perfil_actual();
        println!("perfil antes: {antes:?}");
        println!("aplicar Ahorro: {:?}", aplicar(Modo::Ahorro));
        let cpu = Command::new("docker").args(["inspect", "-f", "{{.HostConfig.NanoCpus}} {{.HostConfig.CpuShares}}", CONTENEDOR]).output().unwrap();
        println!("perfil ahora: {:?}, docker: {}", perfil_actual(), String::from_utf8_lossy(&cpu.stdout).trim());
        println!("aplicar Ultra: {:?}", aplicar(Modo::Ultra));
        restaurar();
        let cpu = Command::new("docker").args(["inspect", "-f", "{{.HostConfig.NanoCpus}} {{.HostConfig.CpuShares}}", CONTENEDOR]).output().unwrap();
        println!("restaurado: perfil {:?}, docker: {}", perfil_actual(), String::from_utf8_lossy(&cpu.stdout).trim());
    }
}
