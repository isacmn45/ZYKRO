//! Configuración de ZYKRO. Las preferencias de la app se guardan en ~/.config/zykro/config.json;
//! la de Android vive en los argumentos de arranque del contenedor y se lee de ahí.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub apagar_al_cerrar: bool,
    pub mostrar_fps: bool,
    pub mostrar_temperatura: bool,
    /// Modo de rendimiento (se aplica sin reiniciar Android).
    pub modo: crate::rendimiento::Modo,
    pub ocultar_teclado: bool,
    pub android: Android,
}

/// Lo que se aplica reiniciando Android.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Android {
    pub ancho: u32,
    pub alto: u32,
    pub dpi: u32,
    pub fps: u32,
    pub lod_activado: bool,
    pub lod: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            apagar_al_cerrar: true,
            mostrar_fps: true,
            mostrar_temperatura: true,
            modo: crate::rendimiento::Modo::default(),
            ocultar_teclado: true,
            android: Android::default(),
        }
    }
}

impl Default for Android {
    fn default() -> Self {
        Android { ancho: 1280, alto: 720, dpi: 240, fps: 60, lod_activado: true, lod: 0.5 }
    }
}

fn ruta() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("zykro/config.json")
}

impl Config {
    pub fn cargar() -> Config {
        let mut config: Config =
            std::fs::read(ruta()).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
        if let Some(actual) = crate::android::leer_config_android() {
            config.android = actual;
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
    /// Valores de los argumentos de arranque del contenedor que maneja ZYKRO.
    pub fn argumentos(&self) -> Vec<(&'static str, String)> {
        vec![
            ("androidboot.redroid_width", self.ancho.to_string()),
            ("androidboot.redroid_height", self.alto.to_string()),
            ("androidboot.redroid_dpi", self.dpi.to_string()),
            ("androidboot.redroid_fps", self.fps.to_string()),
            ("androidboot.qemu.vsync", self.fps.to_string()),
            ("debug.mesa.swag.lod.bias.enabled", (self.lod_activado as u8).to_string()),
            ("debug.mesa.swag.lod.bias", format!("{:.2}", self.lod)),
        ]
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
                "debug.mesa.swag.lod.bias.enabled" => a.lod_activado = valor == "1",
                "debug.mesa.swag.lod.bias" => a.lod = valor.parse().unwrap_or(a.lod),
                _ => {}
            }
        }
        a
    }
}
