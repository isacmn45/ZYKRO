//! Configuración de ZYKRO. Las preferencias de la app se guardan en ~/.config/zykro/config.json;
//! la de Android vive en los argumentos de arranque del contenedor y se lee de ahí.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mostrar_fps: bool,
    pub mostrar_temperatura: bool,
    /// Modo de rendimiento (se aplica sin reiniciar Android).
    pub modo: crate::rendimiento::Modo,
    pub ocultar_teclado: bool,
    pub android: Android,
    /// Núcleos de la CPU para Android; `None` = los del modo de rendimiento.
    pub nucleos: Option<usize>,
    /// Tamaño de la interfaz de ZYKRO (1 = normal).
    pub escala: f32,
    /// Sobre la imagen, el cursor de ZYKRO (grande y de color, se ve sobre cualquier juego).
    pub cursor_zykro: bool,
    /// Apps que se ponen en pantalla completa al abrirlas.
    pub pantalla_completa: Vec<String>,
    /// Salida de sonido de PipeWire o PulseAudio; `None` = la del sistema.
    pub salida_sonido: Option<String>,
    /// El sonido de Android al tocar la pantalla.
    pub sonido_toques: bool,
    /// Esc sale de la pantalla completa y suelta el mouse (si no, llega a Android como Atrás).
    pub esc_sale: bool,
    /// Avisar en el escritorio de las notificaciones de Android cuando ZYKRO no está al frente.
    pub notificaciones_escritorio: bool,
    /// Un acceso directo en el menú de aplicaciones por cada app instalada.
    pub accesos_directos: bool,
    /// Dónde se guardan las capturas; `None` = ~/Imágenes/ZYKRO.
    pub carpeta_capturas: Option<PathBuf>,
    /// Código de la operadora que anuncia Android (`telefono::OPERADORAS`); `None` = ninguna.
    pub operadora: Option<String>,
    /// Teclas de los atajos, por acción (ver `atajos.rs`); las que faltan usan la de siempre.
    pub atajos: BTreeMap<String, String>,
    /// Android Debug Bridge, para conectarse con `adb`.
    pub adb: bool,
    /// Los mandos de la laptop llegan a Android (`gamepad.rs`).
    pub gamepad: bool,
    /// Y la vibración que piden los juegos llega a los mandos.
    pub vibracion: bool,
}

/// Argumento de arranque para que Android use una batería falsa fija (ver `Android::argumentos`).
pub const BATERIA_FIJA: &str = "androidboot.fake_battery";
const VULKAN: &str = "ro.hardware.vulkan";
const NODO_GPU: &str = "androidboot.redroid_gpu_node";
/// El teléfono elegido (su número en `telefono::PERFILES`): sus propiedades van en un archivo aparte
/// (`telefono::archivo`), y este argumento solo lo recuerda.
const PERFIL: &str = "androidboot.zykro_perfil";

/// Lo que se aplica reiniciando Android.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Android {
    pub ancho: u32,
    pub alto: u32,
    pub dpi: u32,
    pub fps: u32,
    /// Vulkan para las apps; sin él, solo OpenGL ES.
    pub vulkan: bool,
    /// Teléfono que anuncia Android (`telefono::PERFILES`); `None` = otro, no se toca.
    pub perfil: Option<usize>,
    /// GPU que usa Android (`/dev/dri/renderD…`); `None` = la primera que sirva.
    pub gpu: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mostrar_fps: true,
            mostrar_temperatura: true,
            modo: crate::rendimiento::Modo::default(),
            ocultar_teclado: true,
            android: Android::default(),
            nucleos: None,
            escala: 1.0,
            cursor_zykro: false,
            pantalla_completa: Vec::new(),
            salida_sonido: None,
            sonido_toques: false,
            esc_sale: false,
            notificaciones_escritorio: true,
            accesos_directos: false,
            carpeta_capturas: None,
            operadora: None,
            atajos: BTreeMap::new(),
            adb: false,
            gamepad: true,
            vibracion: true,
        }
    }
}

impl Default for Android {
    fn default() -> Self {
        Android { ancho: 1280, alto: 720, dpi: 240, fps: 60, vulkan: true, perfil: Some(0), gpu: None }
    }
}

fn ruta() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("zykro/config.json")
}

impl Config {
    /// Lee la configuración guardada. Lo de Android sale del contenedor (es con lo que está
    /// creado), salvo con `android_guardado`: ZYKRO se reinició para aplicar lo guardado.
    pub fn cargar(android_guardado: bool) -> Config {
        let mut config: Config =
            std::fs::read(ruta()).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
        if !android_guardado {
            if let Some(actual) = crate::android::leer_config_android() {
                config.android = actual;
            }
        }
        config
    }

    pub fn guardar(&self) {
        let ruta = ruta();
        if let Some(carpeta) = ruta.parent() {
            let _ = std::fs::create_dir_all(carpeta);
        }
        if let Ok(datos) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(ruta, datos);
        }
    }
}

impl Android {
    /// Valores de los argumentos de arranque del contenedor que maneja ZYKRO (`None` = quitarlo).
    pub fn argumentos(&self) -> Vec<(String, Option<String>)> {
        let mut args: Vec<(String, Option<String>)> = [
            ("androidboot.redroid_width", self.ancho.to_string()),
            ("androidboot.redroid_height", self.alto.to_string()),
            ("androidboot.redroid_dpi", self.dpi.to_string()),
            ("androidboot.redroid_fps", self.fps.to_string()),
            ("androidboot.qemu.vsync", self.fps.to_string()),
            // Batería falsa fija (42 %): Android lee la batería de la laptop y, al verla en 0 % sin
            // cargador, se apagaba solo («shutdown,battery»).
            (BATERIA_FIJA, "1".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), Some(v)))
        .collect();
        // Sin Vulkan: el script de la GPU de Android ya no puede ponerlo (las `ro.` se escriben una vez).
        args.push((VULKAN.into(), (!self.vulkan).then(|| "none".into())));
        args.push((NODO_GPU.into(), self.gpu.clone()));
        args.push((PERFIL.into(), self.perfil.map(|i| i.to_string())));
        args
    }

    pub fn desde_argumentos(args: &[String]) -> Android {
        let mut a = Android::default();
        for arg in args {
            let Some((clave, valor)) = arg.split_once('=') else { continue };
            match clave {
                "androidboot.redroid_width" => a.ancho = valor.parse().unwrap_or(a.ancho),
                "androidboot.redroid_height" => a.alto = valor.parse().unwrap_or(a.alto),
                "androidboot.redroid_dpi" => a.dpi = valor.parse().unwrap_or(a.dpi),
                "androidboot.redroid_fps" => a.fps = valor.parse().unwrap_or(a.fps),
                VULKAN => a.vulkan = valor != "none",
                NODO_GPU => a.gpu = Some(valor.to_string()),
                _ => {}
            }
        }
        a.perfil = args.iter().find_map(|a| a.strip_prefix(&format!("{PERFIL}="))?.parse().ok());
        a
    }
}
