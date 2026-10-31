//! Reabrir ZYKRO: el candado de instancia y la orden que lanza la ventana nueva.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::Command;

/// Toma el candado de `ruta`. Con `esperar`, espera a que lo suelte otro proceso; si no, devuelve
/// `None` cuando ya está tomado.
pub fn bloquear(ruta: &Path, esperar: bool) -> io::Result<Option<File>> {
    let archivo = OpenOptions::new().create(true).truncate(false).write(true).open(ruta)?;
    let modo = if esperar { libc::LOCK_EX } else { libc::LOCK_EX | libc::LOCK_NB };
    if unsafe { libc::flock(archivo.as_raw_fd(), modo) } != 0 {
        let error = io::Error::last_os_error();
        if !esperar && error.kind() == io::ErrorKind::WouldBlock {
            return Ok(None);
        }
        return Err(error);
    }
    Ok(Some(archivo))
}

/// La orden que reabre ZYKRO con el ejecutable instalado (aunque se haya reemplazado mientras
/// estaba abierto) y el argumento de lo que hay que hacer al arrancar.
pub fn comando(exe: &Path, respaldo: Option<&Path>, reiniciar: bool) -> Command {
    let ruta = exe.to_string_lossy();
    let mut orden = Command::new(ruta.trim_end_matches(" (deleted)"));
    if reiniciar {
        orden.arg(crate::REINICIAR);
    } else if let Some(respaldo) = respaldo {
        orden.arg(crate::RESTAURAR).arg(respaldo);
    } else {
        orden.arg(crate::APLICAR_ANDROID);
    }
    orden
}

/// Corre `orden` (apagar Android) y la convierte en error si falla.
pub fn esperar_apagado(orden: &mut Command) -> io::Result<()> {
    let resultado = orden.output()?;
    if resultado.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("No se pudo apagar Android para reiniciar ZYKRO: {} {}",
            resultado.status, String::from_utf8_lossy(&resultado.stderr).trim())))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn selecciona_argumentos_y_ejecutable_instalado() {
        let orden = comando(Path::new("/tmp/zykro (deleted)"), None, true);
        assert_eq!(orden.get_program(), "/tmp/zykro");
        assert_eq!(orden.get_args().collect::<Vec<_>>(), vec![crate::REINICIAR]);
        let orden = comando(Path::new("/tmp/zykro"), None, false);
        assert_eq!(orden.get_args().collect::<Vec<_>>(), vec![crate::APLICAR_ANDROID]);
        let orden = comando(Path::new("/tmp/zykro"), Some(Path::new("/tmp/respaldo con espacios.tar")), false);
        assert_eq!(orden.get_args().collect::<Vec<_>>(), vec![crate::RESTAURAR, "/tmp/respaldo con espacios.tar"]);
    }

    #[test]
    fn espera_a_la_instancia_anterior_y_libera_el_candado() {
        let ruta = std::env::temp_dir().join(format!("zykro-reinicio-{}.lock", std::process::id()));
        let candado = bloquear(&ruta, false).unwrap().unwrap();
        assert!(bloquear(&ruta, false).unwrap().is_none());
        let (listo, recibir) = std::sync::mpsc::channel();
        let siguiente = ruta.clone();
        let hilo = std::thread::spawn(move || {
            let candado = bloquear(&siguiente, true).unwrap().unwrap();
            listo.send(()).unwrap();
            drop(candado);
        });
        assert!(recibir.recv_timeout(std::time::Duration::from_millis(30)).is_err());
        drop(candado);
        recibir.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        hilo.join().unwrap();
        assert!(bloquear(&ruta, false).unwrap().is_some());
        std::fs::remove_file(ruta).unwrap();
    }

    #[test]
    fn falla_si_no_puede_crear_el_candado() {
        assert!(bloquear(Path::new("/dev/null/zykro.lock"), false).is_err());
    }

    #[test]
    fn no_continua_si_falla_el_apagado() {
        assert!(esperar_apagado(Command::new("/bin/sh").args(["-c", "exit 7"])).is_err());
        assert!(esperar_apagado(Command::new("/bin/sh").args(["-c", "exit 0"])).is_ok());
        assert!(esperar_apagado(&mut Command::new("/dev/null/ejecutable-ausente")).is_err());
    }

    #[test]
    fn no_regresa_antes_de_que_termine_el_apagado() {
        let marca = std::env::temp_dir().join(format!("zykro-apagado-{}.txt", std::process::id()));
        let mut orden = Command::new("/bin/sh");
        orden.args(["-c", "sleep 0.05; printf apagado > \"$1\"", "zykro-prueba"]).arg(&marca);
        esperar_apagado(&mut orden).unwrap();
        assert_eq!(std::fs::read_to_string(&marca).unwrap(), "apagado");
        std::fs::remove_file(marca).unwrap();
    }
}
