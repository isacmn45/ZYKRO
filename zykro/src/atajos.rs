//! Atajos de teclado de ZYKRO, como los de BlueStacks: cada acción tiene una tecla (con Ctrl, Alt o
//! Shift) que se cambia en Configuración › Atajos. La tecla que activa o suspende la puntería es de
//! cada mapa de teclas y se cambia en el editor de controles.

use std::collections::BTreeMap;

use eframe::egui::{Event, Key, Modifiers};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Ocultar,
    PantallaCompleta,
    Configuracion,
    Captura,
    SubirVolumen,
    BajarVolumen,
    Silenciar,
    Inicio,
    Atras,
    Recientes,
    Notificaciones,
    InstalarApk,
    Fondos,
    Editor,
    Controles,
    Guia,
    CarpetaCapturas,
}

pub struct Atajo {
    pub accion: Accion,
    /// Nombre en config.json.
    pub id: &'static str,
    pub nombre: &'static str,
    pub grupo: &'static str,
    pub tecla: &'static str,
}

#[rustfmt::skip]
pub const ATAJOS: [Atajo; 17] = [
    Atajo { accion: Accion::Ocultar, id: "ocultar", nombre: "Ocultar ZYKRO (minimiza y silencia)", grupo: "Ventana", tecla: "Ctrl+Shift+X" },
    Atajo { accion: Accion::PantallaCompleta, id: "pantalla_completa", nombre: "Pantalla completa", grupo: "Ventana", tecla: "F11" },
    Atajo { accion: Accion::Configuracion, id: "configuracion", nombre: "Abrir la configuración", grupo: "Ventana", tecla: "Ctrl+Shift+I" },
    Atajo { accion: Accion::Captura, id: "captura", nombre: "Captura de pantalla", grupo: "Sistema", tecla: "Ctrl+Shift+S" },
    Atajo { accion: Accion::SubirVolumen, id: "subir_volumen", nombre: "Subir el volumen", grupo: "Sistema", tecla: "Ctrl+Shift+ArrowUp" },
    Atajo { accion: Accion::BajarVolumen, id: "bajar_volumen", nombre: "Bajar el volumen", grupo: "Sistema", tecla: "Ctrl+Shift+ArrowDown" },
    Atajo { accion: Accion::Silenciar, id: "silenciar", nombre: "Silenciar o activar el sonido", grupo: "Sistema", tecla: "Ctrl+Shift+M" },
    Atajo { accion: Accion::Inicio, id: "inicio", nombre: "Pantalla de inicio", grupo: "Android", tecla: "Ctrl+Shift+H" },
    Atajo { accion: Accion::Atras, id: "atras", nombre: "Atrás", grupo: "Android", tecla: "Ctrl+Shift+Backspace" },
    Atajo { accion: Accion::Recientes, id: "recientes", nombre: "Apps abiertas", grupo: "Android", tecla: "Ctrl+Shift+R" },
    Atajo { accion: Accion::Notificaciones, id: "notificaciones", nombre: "Panel de notificaciones", grupo: "Android", tecla: "Ctrl+Shift+N" },
    Atajo { accion: Accion::InstalarApk, id: "instalar_apk", nombre: "Instalar un APK", grupo: "Android", tecla: "Ctrl+Shift+B" },
    Atajo { accion: Accion::Fondos, id: "fondos", nombre: "Fondo de pantalla", grupo: "Android", tecla: "Ctrl+Shift+W" },
    Atajo { accion: Accion::Editor, id: "editor", nombre: "Editor de controles", grupo: "Controles del juego", tecla: "Ctrl+Shift+A" },
    Atajo { accion: Accion::Controles, id: "controles", nombre: "Activar o desactivar los controles", grupo: "Controles del juego", tecla: "Ctrl+Shift+K" },
    Atajo { accion: Accion::Guia, id: "guia", nombre: "Mostrar u ocultar la guía de teclas", grupo: "Controles del juego", tecla: "Ctrl+Shift+F" },
    Atajo { accion: Accion::CarpetaCapturas, id: "carpeta_capturas", nombre: "Abrir la carpeta de capturas", grupo: "Varios", tecla: "Ctrl+Shift+O" },
];

/// La tecla de `atajo` según la configuración (vacía = sin atajo).
pub fn tecla<'a>(guardadas: &'a BTreeMap<String, String>, atajo: &'a Atajo) -> &'a str {
    guardadas.get(atajo.id).map_or(atajo.tecla, String::as_str)
}

/// La tecla de una acción, para mostrarla junto a su botón ("" si no tiene).
pub fn de(guardadas: &BTreeMap<String, String>, accion: Accion) -> String {
    ATAJOS.iter().find(|a| a.accion == accion).map(|a| mostrar(tecla(guardadas, a))).unwrap_or_default()
}

/// "Ctrl+Shift+S" a partir de la tecla y los modificadores.
pub fn texto(modificadores: Modifiers, tecla: Key) -> String {
    let mut t = String::new();
    if modificadores.ctrl || modificadores.command {
        t += "Ctrl+";
    }
    if modificadores.alt {
        t += "Alt+";
    }
    if modificadores.shift {
        t += "Shift+";
    }
    t + tecla.name()
}

/// Cómo se ve una tecla guardada ("Ctrl+Shift+ArrowUp" → "Ctrl+Shift+Arriba").
pub fn mostrar(tecla: &str) -> String {
    if tecla.is_empty() {
        return "Sin atajo".into();
    }
    tecla
        .split('+')
        .map(|parte| match parte {
            "ArrowUp" => "Arriba",
            "ArrowDown" => "Abajo",
            "ArrowLeft" => "Izquierda",
            "ArrowRight" => "Derecha",
            "Backspace" => "Retroceso",
            "Escape" => "Esc",
            "Space" => "Espacio",
            "Enter" => "Intro",
            "Delete" => "Supr",
            otra => otra,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Saca de `eventos` las teclas que son atajos y devuelve sus acciones. Solo cuentan al bajar la
/// tecla (no al repetirse) y con exactamente esos modificadores.
pub fn tomar(eventos: &mut Vec<Event>, guardadas: &BTreeMap<String, String>) -> Vec<Accion> {
    let mut acciones = Vec::new();
    eventos.retain(|e| {
        let Event::Key { key, pressed: true, repeat: false, modifiers, .. } = e else { return true };
        let pulsada = texto(*modifiers, *key);
        match ATAJOS.iter().find(|a| tecla(guardadas, a) == pulsada) {
            Some(a) => {
                acciones.push(a.accion);
                false
            }
            None => true,
        }
    });
    acciones
}
