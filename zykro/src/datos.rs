//! Pestaña Datos del usuario, como la de BlueStacks: limpiar el disco de lo que dejaron las apps
//! desinstaladas y respaldar o restaurar todo Android (apps, partidas, sesiones y archivos) en un
//! solo archivo .tar.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::android::{self, CONTENEDOR};

/// Lo que no vale la pena respaldar: Android lo vuelve a crear solo.
const EXCLUIR: [&str; 3] = ["./dalvik-cache", "./local/tmp", "./cache"];

/// Borra las carpetas de datos y obb de apps que ya no están instaladas (BlueStacks llama a esto
/// limpieza del disco). Devuelve un aviso con lo que se liberó.
pub fn limpiar() -> String {
    // Si la lista de apps no sale completa (Android ocupado), no se borra nada: si no, se irían los
    // datos de todas. La pantalla de inicio de ZYKRO siempre está instalada. En /data/local/tmp se
    // quedan los sockets del servidor de ZYKRO (zykro) y su teclado.
    let orden = "antes=$(du -sk /data/media/0/Android | cut -f1); \
                 instaladas=$(pm list packages | cut -d: -f2); \
                 echo \"$instaladas\" | grep -qx com.zykro.inicio || exit 1; \
                 for d in /data/media/0/Android/data/* /data/media/0/Android/obb/*; do \
                   [ -d \"$d\" ] || continue; p=${d##*/}; \
                   echo \"$instaladas\" | grep -qx \"$p\" || rm -rf \"$d\"; \
                 done; \
                 for f in /data/local/tmp/*; do case \"${f##*/}\" in zykro|teclado-fisico) ;; *) rm -rf \"$f\" ;; esac; done; \
                 echo $((antes - $(du -sk /data/media/0/Android | cut -f1)))";
    match android::shell(orden).and_then(|s| s.trim().lines().last().and_then(|l| l.parse::<i64>().ok())) {
        Some(kb) if kb > 1024 => format!("Listo: se liberaron {}", tamano(kb as u64 * 1024)),
        Some(_) => "Listo: no había nada que limpiar".into(),
        None => "No se pudo limpiar el disco".into(),
    }
}

/// Lo que ocupa Android sin lo que no se respalda, en bytes (para la barra de progreso).
pub fn tamano_respaldo() -> u64 {
    let excluir = EXCLUIR.map(|e| format!("/data/{}", &e[2..])).join(" ");
    let orden = format!("echo $(( $(du -sk /data | cut -f1) - $(du -sk {excluir} 2>/dev/null | awk '{{s+=$1}} END {{print s+0}}') ))");
    android::shell(&orden).and_then(|s| s.trim().parse::<u64>().ok()).map_or(0, |kb| kb * 1024)
}

/// Copia todo Android a `destino` mientras sigue encendido; `escrito` va contando los bytes.
pub fn respaldar(destino: &Path, escrito: &AtomicU64) -> Result<(), String> {
    let excluir: String = EXCLUIR.iter().map(|e| format!(" --exclude={e}")).collect();
    let mut hijo = Command::new("docker")
        .args(["exec", CONTENEDOR, "sh", "-c", &format!("sync; cd /data && tar c{excluir} .")])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut salida = std::fs::File::create(destino).map_err(|e| format!("no se pudo crear el archivo: {e}"))?;
    let mut entrada = hijo.stdout.take().ok_or("sin salida de tar")?;
    let mut bloque = vec![0u8; 1 << 20];
    loop {
        let n = entrada.read(&mut bloque).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        salida.write_all(&bloque[..n]).map_err(|e| format!("no se pudo escribir el respaldo: {e}"))?;
        escrito.fetch_add(n as u64, Ordering::Relaxed);
    }
    salida.sync_all().map_err(|e| e.to_string())?;
    // tar avisa (y termina con error) por los archivos que cambian mientras los lee: el respaldo sirve igual.
    let _ = hijo.wait();
    if escrito.load(Ordering::Relaxed) < 1024 * 1024 {
        let _ = std::fs::remove_file(destino);
        return Err("Android no devolvió datos".into());
    }
    Ok(())
}

/// Cambia todos los datos de Android por los del respaldo `origen`. Android tiene que estar apagado:
/// lo apaga, y se enciende después como siempre.
pub fn restaurar(origen: &Path) -> Result<(), String> {
    let mut cabecera = [0u8; 512];
    std::fs::File::open(origen).and_then(|mut f| f.read_exact(&mut cabecera)).map_err(|e| format!("no se pudo leer el respaldo: {e}"))?;
    if &cabecera[257..262] != b"ustar" {
        return Err("el archivo no es un respaldo de ZYKRO".into());
    }
    let info = Command::new("docker").args(["inspect", CONTENEDOR]).output().map_err(|e| e.to_string())?;
    let info: serde_json::Value = serde_json::from_slice(&info.stdout).map_err(|e| e.to_string())?;
    let imagen = info[0]["Config"]["Image"].as_str().ok_or("no se encontró la imagen de Android")?.to_string();
    let volumen = info[0]["Mounts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["Destination"] == "/data")
        .and_then(|m| m["Name"].as_str())
        .ok_or("no se encontró el volumen de los datos")?
        .to_string();
    Command::new("docker").args(["stop", "-t", "10", CONTENEDOR]).status().map_err(|e| e.to_string())?;
    // Vaciar los datos (el volumen en sí no se puede borrar: rm se queja y sigue) y abrir el respaldo.
    toybox(&volumen, &imagen, &["rm", "-rf", "/data"], None)?;
    let respaldo = std::fs::File::open(origen).map_err(|e| e.to_string())?;
    if !toybox(&volumen, &imagen, &["tar", "x", "-C", "/data"], Some(respaldo))? {
        return Err("tar no pudo abrir el respaldo".into());
    }
    Ok(())
}

/// Corre toybox de la imagen de Android con Android apagado. No hay `sh` fuera de Android: cada
/// orden va sola. Devuelve si terminó bien.
fn toybox(volumen: &str, imagen: &str, orden: &[&str], entrada: Option<std::fs::File>) -> Result<bool, String> {
    let mut docker = Command::new("docker");
    docker
        .args(["run", "--rm", "-i", "-v", &format!("{volumen}:/data")])
        .args(["--entrypoint", "/system/bin/toybox"])
        .arg(imagen)
        .args(orden)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(entrada) = entrada {
        docker.stdin(entrada);
    }
    docker.status().map(|s| s.success()).map_err(|e| e.to_string())
}

pub fn tamano(bytes: u64) -> String {
    let gb = bytes as f64 / (1u64 << 30) as f64;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / (1u64 << 20) as f64)
    }
}
