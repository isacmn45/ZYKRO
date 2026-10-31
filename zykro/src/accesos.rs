//! Accesos directos de las apps de Android en el menú de aplicaciones de la laptop (Configuración ›
//! Preferencias), como los que BlueStacks pone en el escritorio: abren ZYKRO con esa app (`--abrir`).

use std::collections::HashSet;
use std::path::PathBuf;

use crate::android;

/// Carpeta de Android donde la pantalla de inicio deja la lista de apps y sus íconos.
const EXPORTADO: &str = "/data/media/0/Android/data/com.zykro.inicio/files";
const PREFIJO: &str = "zykro-app-";

/// Crea un acceso por cada app propia (no del sistema) y quita los de las apps que ya no están;
/// con `activar` en `false`, los quita todos.
pub fn sincronizar(activar: bool) {
    let datos = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    let menu = datos.join("applications");
    let iconos = datos.join("zykro/iconos-apps");
    let mut quedan = HashSet::new();

    if activar {
        let lista = android::shell(&format!("cat {EXPORTADO}/apps.json")).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let Some(apps) = lista.as_ref().and_then(|l| l.as_array()) else { return };
        let _ = std::fs::remove_dir_all(&iconos);
        let _ = std::fs::create_dir_all(&menu);
        let con_iconos = android::copiar_desde(&format!("{EXPORTADO}/iconos"), &iconos);
        let zykro = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("zykro"));
        for app in apps.iter().filter(|a| !a["sistema"].as_bool().unwrap_or(true)) {
            let (Some(paquete), Some(nombre)) = (app["paquete"].as_str(), app["nombre"].as_str()) else { continue };
            // Solo paquetes con nombres normales: van en el nombre del archivo y en la orden.
            if !paquete.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
                continue;
            }
            let icono = iconos.join(format!("{paquete}.png"));
            let icono = if con_iconos && icono.exists() { icono.to_string_lossy().into_owned() } else { "zykro".into() };
            let entrada = format!(
                "[Desktop Entry]\nType=Application\nName={nombre}\nComment=Abrir en ZYKRO\nExec=\"{}\" --abrir {paquete}\n\
                 Icon={icono}\nCategories=Game;\nStartupWMClass=zykro\n",
                zykro.display()
            );
            let archivo = format!("{PREFIJO}{paquete}.desktop");
            let _ = std::fs::write(menu.join(&archivo), entrada);
            quedan.insert(archivo);
        }
    }

    for archivo in std::fs::read_dir(&menu).into_iter().flatten().flatten() {
        let nombre = archivo.file_name().to_string_lossy().into_owned();
        if nombre.starts_with(PREFIJO) && nombre.ends_with(".desktop") && !quedan.contains(&nombre) {
            let _ = std::fs::remove_file(archivo.path());
        }
    }
    if !activar {
        let _ = std::fs::remove_dir_all(&iconos);
    }
}
