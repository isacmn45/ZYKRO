//! Encender Android (un contenedor redroid con la imagen de ZYKRO, Android 13, ver imagen/) y
//! conectarse a su servidor (servidor/): la pantalla, los toques y el sonido.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::config::{self, Config};
use crate::control::Control;
use crate::video::{self, Compartido};

pub const CONTENEDOR: &str = "zykro-android";
/// Imagen con la que se crea (la arma imagen/construir.sh).
const IMAGEN: &str = "zykro-android:13";
/// Volumen con los datos de Android: apps, partidas y sesiones.
const VOLUMEN: &str = "zykro-android13-datos";
/// Sockets del servidor de ZYKRO: en Android es /data/local/tmp/zykro.
pub const SOCKETS: &str = "/tmp/zykro-android";
/// Dónde escucha ADB en la laptop (Configuración › Avanzado lo enciende).
const PUERTO_ADB: &str = "127.0.0.1:5555";

/// Paso del arranque en el que está Android (para la pantalla de carga).
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Etapa {
    #[default]
    Encender,
    Arrancar,
    Conectar,
}

#[derive(Default)]
pub struct Estado {
    /// Texto que se muestra mientras no hay video (vacío cuando todo va bien).
    pub mensaje: String,
    pub etapa: Etapa,
    /// Desde cuándo está en la etapa actual.
    pub etapa_desde: Option<Instant>,
    /// El mensaje es un error (se reintenta solo).
    pub error: bool,
    /// Aviso corto en la esquina (captura guardada, APK instalada...).
    pub aviso: Option<(String, Instant)>,
    pub control: Option<Control>,
    pub pantalla: Option<(u32, u32)>,
    pub config: Config,
    /// Configuración de Android que se aplica al arrancar recreando el contenedor (ZYKRO se
    /// volvió a abrir para aplicarla).
    pub reinicio: Option<config::Android>,
    /// El juego tiene abierto su cuadro de texto (chat): el mapeo se apaga para poder escribir.
    pub escribiendo: bool,
    /// Cambiaron las notificaciones (las exporta la pantalla de inicio de ZYKRO).
    pub notificaciones_cambiaron: bool,
    /// Entre los cambios hay una notificación que avisa: ZYKRO la muestra como emergente.
    pub notificacion_emergente: bool,
    /// Se mantuvo presionado un espacio vacío del inicio: abrir la ventana de fondos de pantalla.
    pub pedir_fondos: bool,
    /// La pantalla de inicio cambió su fondo de pantalla.
    pub fondo_cambio: bool,
    /// Se instaló, quitó o actualizó una app (para los accesos directos).
    pub apps_cambiaron: bool,
    /// Respaldo que hay que restaurar antes de encender Android (Configuración › Datos del usuario).
    pub restaurar: Option<PathBuf>,
}

pub type EstadoCompartido = Arc<Mutex<Estado>>;

/// Corre `orden` en la shell de Android (como root) y devuelve lo que escribió, si terminó bien.
pub fn shell(orden: &str) -> Option<String> {
    let salida = Command::new("docker").args(["exec", CONTENEDOR, "sh", "-c", orden]).stderr(Stdio::null()).output().ok()?;
    salida.status.success().then(|| String::from_utf8_lossy(&salida.stdout).into_owned())
}

/// Copia un archivo de la laptop a Android.
pub fn copiar_a(origen: &Path, destino: &str) -> bool {
    docker(&["cp", &origen.to_string_lossy(), &format!("{CONTENEDOR}:{destino}")])
}

/// Copia un archivo o carpeta de Android a la laptop.
pub fn copiar_desde(origen: &str, destino: &Path) -> bool {
    docker(&["cp", &format!("{CONTENEDOR}:{origen}"), &destino.to_string_lossy()])
}

fn docker(args: &[&str]) -> bool {
    Command::new("docker")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn existe() -> bool {
    docker(&["container", "inspect", CONTENEDOR])
}

/// Crea el contenedor de Android (la primera vez, o si se borró). Los datos van en su volumen.
fn crear(android: &config::Android) -> Result<(), String> {
    if !docker(&["image", "inspect", IMAGEN]) {
        return Err(format!("Falta la imagen de Android ({IMAGEN}): corre ./instalar.sh"));
    }
    let _ = std::fs::create_dir_all(SOCKETS);
    escribir_telefono(android.perfil)?;
    let telefono = crate::telefono::archivo();
    let mut run: Vec<String> = [
        "run", "-d", "--name", CONTENEDOR, "--privileged", "--security-opt", "label=disable", "--shm-size", "64m",
        "-v", &format!("{SOCKETS}:/data/local/tmp/zykro"), "-v", &format!("{VOLUMEN}:/data"),
        "-v", &format!("{}:{PROPIEDADES_PRODUCTO}:ro", telefono.display()),
        "-p", &format!("{PUERTO_ADB}:5555"), "--entrypoint", "/init", IMAGEN,
        // GPU de la laptop; memfd en vez de ashmem (los kernels nuevos ya no lo traen).
        "androidboot.hardware=redroid", "androidboot.redroid_gpu_mode=host", "androidboot.use_memfd=1",
        // SurfaceFlinger toma los cuadros sin esperar a que la GPU avise que terminó, con un búfer más:
        // menos retraso. Y más memoria para las apps de Java (los juegos grandes la piden).
        "debug.sf.latch_unsignaled=1", "ro.surface_flinger.max_frame_buffer_acquired_buffers=3",
        "dalvik.vm.heapsize=512m", "dalvik.vm.heapgrowthlimit=256m",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    run.extend(android.argumentos().into_iter().filter_map(|(k, v)| Some(format!("{k}={}", v?))));
    // En el idioma de la laptop (es_MX.UTF-8 -> es-MX); se cambia en Configuración › Android.
    let idioma = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|l| !l.is_empty()));
    if let Some(idioma) = idioma.map(|l| l.split(['.', '@']).next().unwrap_or("").replace('_', "-")).filter(|l| l.len() >= 2 && l != "C" && l != "POSIX") {
        run.push(format!("ro.product.locale={idioma}"));
    }
    let creado = Command::new("docker").args(&run).output().map_err(|e| e.to_string())?;
    if !creado.status.success() {
        return Err(String::from_utf8_lossy(&creado.stderr).trim().to_string());
    }
    Ok(())
}

/// Dónde lee Android las propiedades del producto (ver `telefono::archivo`).
const PROPIEDADES_PRODUCTO: &str = "/system/product/etc/build.prop";

/// Escribe el archivo del teléfono `perfil`, sobre las propiedades del producto de la imagen.
fn escribir_telefono(perfil: Option<usize>) -> Result<(), String> {
    // Toybox de la imagen con el enlazador del primer arranque (fuera de Android no hay /apex).
    let base = Command::new("docker")
        .args(["run", "--rm", "-e", "LD_LIBRARY_PATH=/system/lib64/bootstrap:/system/lib64"])
        .args(["--entrypoint", "/system/bin/bootstrap/linker64", IMAGEN, "/system/bin/toybox", "cat", PROPIEDADES_PRODUCTO])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !base.status.success() {
        return Err(format!("no se pudieron leer las propiedades de {IMAGEN}"));
    }
    crate::telefono::escribir(perfil, &String::from_utf8_lossy(&base.stdout)).map_err(|e| e.to_string())
}

fn corriendo() -> bool {
    Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", CONTENEDOR])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

fn mensaje(estado: &EstadoCompartido, etapa: Etapa, texto: impl Into<String>) {
    let mut s = estado.lock().unwrap();
    if s.etapa != etapa || s.etapa_desde.is_none() {
        s.etapa = etapa;
        s.etapa_desde = Some(Instant::now());
    }
    s.mensaje = texto.into();
    s.error = false;
}

fn error(estado: &EstadoCompartido, texto: impl Into<String>) {
    let mut s = estado.lock().unwrap();
    s.mensaje = texto.into();
    s.error = true;
}

pub fn avisar(estado: &EstadoCompartido, texto: impl Into<String>) {
    estado.lock().unwrap().aviso = Some((texto.into(), Instant::now()));
}

/// Hilo que mantiene la conexión con Android y reconecta si se pierde.
pub fn iniciar(ctx: egui::Context, estado: EstadoCompartido, destino: Compartido) {
    thread::Builder::new()
        .name("zykro-android".into())
        .spawn(move || {
            let respaldo = estado.lock().unwrap().restaurar.take();
            if let Some(respaldo) = respaldo {
                mensaje(&estado, Etapa::Encender, "Restaurando el respaldo… (puede tardar unos minutos)");
                ctx.request_repaint();
                match crate::datos::restaurar(&respaldo) {
                    Ok(()) => avisar(&estado, "Respaldo restaurado"),
                    Err(e) => {
                        error(&estado, format!("No se pudo restaurar el respaldo: {e}"));
                        ctx.request_repaint();
                        thread::sleep(Duration::from_secs(5));
                    }
                }
            }
            let mut pendiente = estado.lock().unwrap().reinicio.take();
            // Contenedores creados antes de la batería fija, o con una imagen de Android más vieja que
            // la que dejó instalar.sh: se recrean (mismos datos) si Android está apagado, para no
            // cortar una sesión.
            if pendiente.is_none() && !corriendo() {
                let args = argumentos_contenedor().unwrap_or_default();
                let sin_bateria = !args.iter().any(|a| a.starts_with(config::BATERIA_FIJA));
                if !args.is_empty() && (sin_bateria || imagen_desactualizada()) {
                    pendiente = Some(config::Android::desde_argumentos(&args));
                }
            }
            if let Some(nueva) = pendiente.filter(|_| existe()) {
                mensaje(&estado, Etapa::Encender, "Reiniciando Android con la nueva configuración…");
                ctx.request_repaint();
                match recrear(&nueva) {
                    // El contenedor nuevo nace sin el límite de núcleos del modo: se vuelve a aplicar.
                    Ok(()) => {
                        let (modo, nucleos) = {
                            let s = estado.lock().unwrap();
                            (s.config.modo, s.config.nucleos)
                        };
                        crate::rendimiento::aplicar(modo, nucleos);
                    }
                    Err(e) => {
                        error(&estado, format!("No se pudo aplicar la configuración: {e}"));
                        ctx.request_repaint();
                        thread::sleep(Duration::from_secs(4));
                    }
                }
            }
            loop {
                if let Err(e) = conectar_y_recibir(&ctx, &estado, &destino) {
                    {
                        let mut s = estado.lock().unwrap();
                        s.control = None;
                        s.mensaje = format!("{e}\nReintentando…");
                        s.error = true;
                    }
                    ctx.request_repaint();
                    thread::sleep(Duration::from_secs(2));
                }
            }
        })
        .expect("no se pudo crear el hilo de Android");
}

fn conectar_y_recibir(ctx: &egui::Context, estado: &EstadoCompartido, destino: &Compartido) -> std::io::Result<()> {
    // La carpeta de los sockets está en /tmp, que se vacía al apagar la laptop. Si Docker la creara,
    // sería de root.
    let _ = std::fs::create_dir_all(SOCKETS);
    if !existe() {
        mensaje(estado, Etapa::Encender, "Preparando Android por primera vez…");
        ctx.request_repaint();
        let android = estado.lock().unwrap().config.android.clone();
        crear(&android).map_err(|e| std::io::Error::other(format!("No se pudo crear Android: {e}")))?;
    } else if !corriendo() {
        mensaje(estado, Etapa::Encender, "Encendiendo Android…");
        ctx.request_repaint();
        // Si se borró el archivo del teléfono, Docker montaría una carpeta en su lugar.
        if !crate::telefono::archivo().exists() {
            let perfil = estado.lock().unwrap().config.android.perfil;
            let _ = escribir_telefono(perfil);
        }
        if !docker(&["start", CONTENEDOR]) {
            return Err(std::io::Error::other("No se pudo encender Android. ¿Está corriendo Docker?"));
        }
    }

    mensaje(estado, Etapa::Arrancar, "Esperando a Android…");
    ctx.request_repaint();
    let ruta_video = format!("{SOCKETS}/video.sock");
    let ruta_control = format!("{SOCKETS}/control.sock");
    let mut socket_video = loop {
        if Path::new(&ruta_video).exists() && Path::new(&ruta_control).exists() {
            match video::conectar(&ruta_video) {
                Ok(socket) => break socket,
                // Mientras Android arranca quedan los sockets de la sesión anterior, sin nadie
                // escuchando: se sigue esperando a que el servidor los vuelva a crear.
                Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        thread::sleep(Duration::from_millis(300));
    };

    mensaje(estado, Etapa::Conectar, "Conectando…");
    ctx.request_repaint();
    let ocultar_teclado = estado.lock().unwrap().config.ocultar_teclado;
    let control = Control::conectar(&ruta_control)?;

    // El servidor manda la cabecera y el cuadro actual en cuanto se conecta. Si no llegan, está
    // trabado: se reinicia y se vuelve a conectar.
    socket_video.set_read_timeout(Some(Duration::from_secs(5)))?;
    let pantalla = match video::leer_cabecera(&mut socket_video) {
        Ok(p) => p,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
            reiniciar_servidor();
            return Err(std::io::Error::other("El servidor de video no respondía: se reinició"));
        }
        Err(e) => return Err(e),
    };
    socket_video.set_read_timeout(None)?;

    {
        let mut s = estado.lock().unwrap();
        s.control = Some(control);
        s.pantalla = Some((pantalla.ancho, pantalla.alto));
        s.mensaje.clear();
        s.error = false;
    }
    if ocultar_teclado {
        ocultar_teclado_en_pantalla();
    }
    poner_zona_horaria();
    asegurar_inicio();
    quitar_sobrantes();
    let config = estado.lock().unwrap().config.clone();
    aplicar_preferencias(&config);
    if config.accesos_directos {
        thread::spawn(|| crate::accesos::sincronizar(true));
    }
    // Abierto desde un acceso directo de una app: se abre en cuanto Android está listo.
    if let Some(paquete) = ABRIR_AL_INICIAR.lock().unwrap().take() {
        thread::spawn(move || abrir_app(&paquete));
    }

    let ctx2 = ctx.clone();
    video::recibir_cuadros(socket_video, destino.clone(), move || ctx2.request_repaint())
}

/// Reinicia el servidor de ZYKRO (un servicio de Android) sin reiniciar Android; corta sus conexiones.
pub fn reiniciar_servidor() {
    docker(&["exec", CONTENEDOR, "setprop", "ctl.restart", "zykro-servidor"]);
}

/// Android arranca con la hora UTC: se le pone la zona horaria de la laptop para que su reloj (el de la
/// pantalla de inicio) marque la hora local.
fn poner_zona_horaria() {
    let enlace = std::fs::read_link("/etc/localtime").ok();
    let Some(zona) = enlace.as_ref().and_then(|r| r.to_str()).and_then(|r| r.split_once("zoneinfo/")).map(|(_, z)| z.to_string())
    else {
        return;
    };
    thread::spawn(move || {
        docker(&["exec", CONTENEDOR, "cmd", "alarm", "set-timezone", &zona]);
    });
}

/// Apps de Android que no sirven en un emulador. Se desinstalan solo para el usuario: siguen en la
/// imagen y `cmd package install-existing <paquete>` devuelve cualquiera. Lo que tiene un teléfono de
/// verdad (llamadas, Bluetooth, Wi-Fi, ubicación, cámara) se queda: los juegos lo revisan para saber
/// si corren en un emulador.
const SOBRANTES: &[&str] = &[
    // Protectores de pantalla, fondos animados y el huevo de pascua.
    "com.android.dreams.basic",
    "com.android.dreams.phototable",
    "com.android.egg",
    "com.android.wallpaper.livepicker",
    "com.android.wallpapercropper",
    "com.android.wallpaperbackup",
    // Impresión.
    "com.android.bips",
    "com.android.printservice.recommendation",
    "com.android.printspooler",
    // Copias de seguridad.
    "com.android.localtransport",
    "com.android.backupconfirm",
    "com.android.sharedstoragebackup",
    // Perfiles de empresa, instalar otro sistema, tareas del primer arranque y de pruebas.
    "com.android.managedprovisioning",
    "com.android.dynsystem",
    "com.android.onetimeinitializer",
    "com.android.cts.ctsshim",
    "com.android.cts.priv.ctsshim",
    "com.android.traceur",
    // Hardware que no hay: NFC, chip seguro, USB, relojes, MIDI por Bluetooth, efectos de cámara.
    "com.android.nfc",
    "com.android.se",
    "com.android.mtp",
    "com.android.companiondevicemanager",
    "com.android.bluetoothmidiservice",
    "com.android.cameraextensions",
    // Mensajes MMS y de la SIM (el teléfono en sí se queda).
    "com.android.mms.service",
    "com.android.smspush",
    "com.android.simappdialog",
    // Inicio de sesión de redes Wi-Fi públicas (la red sale de Docker).
    "com.android.captiveportallogin",
    "com.android.hotspot2.osulogin",
    // Calendario, correo, sincronizar contactos con Google, ecualizador, verificar enlaces,
    // marcadores, el visor de licencias, la barra de búsqueda de Google de AOSP y el navegador de
    // pruebas de WebView.
    "com.android.providers.calendar",
    "com.android.calendar",
    "com.android.email",
    "com.google.android.syncadapters.calendar",
    "com.google.android.syncadapters.contacts",
    "com.android.musicfx",
    "com.android.statementservice",
    "com.android.bookmarkprovider",
    "com.android.htmlviewer",
    "com.android.quicksearchbox",
    "org.chromium.webview_shell",
    // Lo que Google Play instala solo: SafetyCore (revisa las imágenes de los mensajes), los
    // verificadores de contactos y de desarrolladores (este revisa las apps instaladas por fuera de la
    // tienda, como la pantalla de inicio de ZYKRO) y realidad aumentada. Si Play las reinstala, se
    // vuelven a quitar al abrir ZYKRO.
    "com.google.android.safetycore",
    "com.google.android.contactkeys",
    "com.google.android.verifier",
    "com.google.ar.core",
];

/// Quita de Android las apps de `SOBRANTES` que sigan instaladas.
fn quitar_sobrantes() {
    thread::spawn(|| {
        let lista = SOBRANTES.join(" ");
        let salida = shell(&format!(
            "instaladas=$(pm list packages --user 0); for p in {lista}; do \
             echo \"$instaladas\" | grep -qx \"package:$p\" && pm uninstall --user 0 $p >/dev/null && echo $p; done; true"
        ));
        let quitadas: Vec<&str> = salida.as_deref().unwrap_or("").lines().collect();
        if !quitadas.is_empty() {
            eprintln!("[android] apps que sobraban quitadas: {}", quitadas.join(", "));
        }
    });
}

/// Paquete de la pantalla de inicio de ZYKRO (`launcher/` de este repositorio).
pub const INICIO: &str = "com.zykro.inicio";

/// Instala la pantalla de inicio de ZYKRO en Android si falta o si instalar.sh dejó otra versión
/// (~/.local/share/zykro/zykro-inicio.apk). La primera vez desactiva el launcher de fábrica para que
/// Android no pregunte cuál usar.
fn asegurar_inicio() {
    let datos = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    let Some(apk) = datos.map(|d| d.join("zykro/zykro-inicio.apk")).filter(|p| p.exists()) else { return };
    thread::spawn(move || {
        // Tamaño y fecha del APK: si no cambiaron y sigue instalado, no hay nada que hacer.
        let marca = apk.with_extension("instalado");
        let firma = std::fs::metadata(&apk)
            .map(|m| format!("{} {:?}", m.len(), m.modified().ok()))
            .unwrap_or_default();
        let ruta = Command::new("docker").args(["exec", CONTENEDOR, "pm", "path", INICIO]).output();
        let estaba = ruta.is_ok_and(|s| String::from_utf8_lossy(&s.stdout).contains("package:"));
        if estaba && std::fs::read_to_string(&marca).is_ok_and(|m| m == firma) {
            permisos_inicio();
            return;
        }
        let destino = "/data/local/tmp/zykro-inicio.apk";
        if !docker(&["cp", &apk.to_string_lossy(), &format!("{CONTENEDOR}:{destino}")]) {
            return;
        }
        let instalar = || {
            let salida = Command::new("docker").args(["exec", CONTENEDOR, "pm", "install", "-r", destino]).output().ok()?;
            Some(String::from_utf8_lossy(&salida.stdout).to_string() + &String::from_utf8_lossy(&salida.stderr))
        };
        let mut resultado = instalar().unwrap_or_default();
        // Compilada con otra llave (otra copia del repositorio): se quita la vieja y se instala de nuevo.
        if resultado.contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE") {
            docker(&["exec", CONTENEDOR, "pm", "uninstall", INICIO]);
            resultado = instalar().unwrap_or_default();
        }
        docker(&["exec", CONTENEDOR, "rm", "-f", destino]);
        if !resultado.contains("Success") {
            eprintln!("[android] no se pudo instalar la pantalla de inicio: {}", resultado.trim());
            return;
        }
        if !estaba {
            docker(&["exec", CONTENEDOR, "pm", "disable-user", "--user", "0", "com.android.launcher3"]);
            docker(&["exec", CONTENEDOR, "input", "keyevent", "3"]);
        }
        let _ = std::fs::write(&marca, firma);
        permisos_inicio();
    });
}

/// La pantalla de inicio escucha las notificaciones para el panel de ZYKRO, y las abre aunque esté
/// detrás de otra app (el permiso de mostrarse encima la exime de esa restricción de Android). Las
/// emergentes de Android se apagan: las muestra ZYKRO, con su tema.
fn permisos_inicio() {
    shell(&format!(
        "cmd notification allow_listener {INICIO}/{INICIO}.Escucha; appops set {INICIO} SYSTEM_ALERT_WINDOW allow; \
         settings put global heads_up_notifications_enabled 0"
    ));
}

/// Usa teclado-android (`bin/` de este repositorio) para que Android no muestre su teclado en pantalla.
/// El aviso «Configura el teclado físico» que eso provoca lo pospone la pantalla de inicio de ZYKRO.
fn ocultar_teclado_en_pantalla() {
    thread::spawn(|| {
        if let Some(home) = std::env::var_os("HOME") {
            let script = PathBuf::from(home).join(".local/bin/teclado-android");
            if script.exists() {
                let _ = Command::new(script).arg("ocultar").stdout(Stdio::null()).stderr(Stdio::null()).status();
            }
        }
    });
}

/// Escucha lo que la pantalla de inicio de ZYKRO pide por el registro de Android (etiqueta `ZYKRO`):
/// abrir la ventana de fondos al mantener presionado un espacio vacío, y avisar que cambiaron las
/// notificaciones o el fondo.
pub fn vigilar_peticiones(ctx: egui::Context, estado: EstadoCompartido) {
    use std::io::{BufRead, BufReader};
    thread::Builder::new()
        .name("zykro-peticiones".into())
        .spawn(move || loop {
            // Solo lo que llegue desde ahora (la hora de Android), para no repetir pedidos viejos.
            let orden = "exec logcat -v raw -s ZYKRO:I -T \"$(date '+%m-%d %H:%M:%S.000')\"";
            let hijo = Command::new("docker")
                .args(["exec", CONTENEDOR, "sh", "-c", orden])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut hijo) = hijo {
                for linea in BufReader::new(hijo.stdout.take().unwrap()).lines().map_while(Result::ok) {
                    match linea.trim() {
                        "fondos" => estado.lock().unwrap().pedir_fondos = true,
                        "fondo" => estado.lock().unwrap().fondo_cambio = true,
                        "apps" => estado.lock().unwrap().apps_cambiaron = true,
                        "notificaciones" => estado.lock().unwrap().notificaciones_cambiaron = true,
                        "emergente" => {
                            let mut estado = estado.lock().unwrap();
                            estado.notificaciones_cambiaron = true;
                            estado.notificacion_emergente = true;
                        }
                        _ => continue,
                    }
                    ctx.request_repaint();
                }
                let _ = hijo.wait();
            }
            // Android apagado o reiniciándose: se vuelve a intentar.
            thread::sleep(Duration::from_secs(3));
        })
        .expect("no se pudo crear el hilo de peticiones");
}

/// Sabe cuándo un juego abre su cuadro de texto (en Unity es una ventana aparte que pide el teclado
/// siempre visible): el servidor de ZYKRO lo deja en el archivo `texto` (1 o 0).
pub fn vigilar_texto(ctx: egui::Context, estado: EstadoCompartido) {
    let ruta = format!("{SOCKETS}/texto");
    thread::Builder::new()
        .name("zykro-texto".into())
        .spawn(move || {
            let mut antes = false;
            loop {
                let ahora = std::fs::read_to_string(&ruta).is_ok_and(|t| t.trim() == "1");
                if ahora != antes {
                    estado.lock().unwrap().escribiendo = ahora;
                    ctx.request_repaint();
                    antes = ahora;
                }
                thread::sleep(Duration::from_millis(250));
            }
        })
        .expect("no se pudo crear el hilo de texto");
}

/// Paquete de la app en primer plano, según el servidor de ZYKRO.
pub fn app_activa() -> Option<String> {
    let texto = std::fs::read_to_string(format!("{SOCKETS}/active_app")).ok()?;
    texto.lines().find_map(|l| l.strip_prefix("package=").or_else(|| l.strip_prefix("Package=")).map(|p| p.trim().to_string()))
}

/// Una app abierta en Android (una tarea, como en la vista de recientes).
#[derive(Clone)]
pub struct Tarea {
    /// Tarea raíz que la contiene: es lo que se quita para cerrarla.
    pub raiz: u32,
    pub paquete: String,
}

/// Apps abiertas, de la más reciente a la más vieja, sin el launcher. Android no tiene vista de
/// recientes sin Quickstep, así que ZYKRO las saca de `am stack list` y las ordena con
/// `dumpsys activity recents`.
pub fn tareas_abiertas() -> Vec<Tarea> {
    let consulta = "cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME | tail -1; \
                    echo ===; am stack list; echo ===; dumpsys activity recents | grep 'Recent #'";
    let Ok(salida) = Command::new("docker").args(["exec", CONTENEDOR, "sh", "-c", consulta]).output() else { return Vec::new() };
    let texto = String::from_utf8_lossy(&salida.stdout);
    let mut partes = texto.split("===\n");
    let inicio = partes.next().unwrap_or("").trim().split('/').next().unwrap_or("").to_string();
    let (pila, recientes) = (partes.next().unwrap_or(""), partes.next().unwrap_or(""));

    // Orden: "* Recent #0: Task{1412083 #16 type=home ...}" -> la tarea 16 es la más reciente.
    let orden: Vec<u32> = recientes
        .lines()
        .filter_map(|l| l.split("Task{").nth(1)?.split_whitespace().nth(1)?.strip_prefix('#')?.parse().ok())
        .collect();

    let mut tareas = Vec::new();
    let mut raiz = 0;
    for linea in pila.lines() {
        if let Some(resto) = linea.strip_prefix("RootTask id=") {
            raiz = resto.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
        } else if let Some(resto) = linea.trim_start().strip_prefix("taskId=") {
            // "taskId=17: com.android.settings/com.android.settings.Settings bounds=..."
            let Some((id, resto)) = resto.split_once(": ") else { continue };
            let componente = resto.split_whitespace().next().unwrap_or("");
            let Some((paquete, actividad)) = componente.split_once('/') else { continue };
            if paquete == inicio || actividad.contains("FallbackHome") {
                continue;
            }
            let id: u32 = id.parse().unwrap_or(0);
            tareas.push((orden.iter().position(|&o| o == id).unwrap_or(usize::MAX), Tarea { raiz, paquete: paquete.to_string() }));
        }
    }
    tareas.sort_by_key(|(posicion, _)| *posicion);
    tareas.into_iter().map(|(_, t)| t).collect()
}

/// Trae una app al frente como lo hace un launcher: con su intent de inicio, que reutiliza la tarea.
pub fn abrir_app(paquete: &str) {
    if !paquete.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
        return;
    }
    let orden = format!(
        "C=$(cmd package resolve-activity --brief -c android.intent.category.LAUNCHER {paquete} | tail -1); \
         am start -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -f 0x10200000 -n \"$C\""
    );
    docker(&["exec", CONTENEDOR, "sh", "-c", &orden]);
}

/// App que hay que abrir en cuanto Android esté listo (ZYKRO se abrió desde su acceso directo).
pub static ABRIR_AL_INICIAR: Mutex<Option<String>> = Mutex::new(None);

/// Con la imagen sin Bluetooth (sin interfaz HCI), el adaptador y la búsqueda BLE quedan apagados
/// para que el servicio de Android no falle al arrancar.
const BLUETOOTH_SIN_HAL: &str = "if [ \"$(getprop persist.bluetooth.disabled)\" = true ] || \
    [ \"$(getprop bluetooth.service.enabled)\" = false ]; then \
    settings put global ble_scan_always_enabled 0; svc bluetooth disable; fi";

/// Lo de Configuración que se aplica en Android sin reiniciarlo: la operadora, el sonido al tocar y
/// ADB (Android lo olvida al reiniciarse, así que se aplica cada vez que arranca).
pub fn aplicar_preferencias(config: &config::Config) {
    let mut adb = if config.adb { "setprop service.adb.tcp.port 5555; setprop ctl.start adbd".to_string() } else { "setprop ctl.stop adbd".into() };
    // La llave de `adb` de la laptop queda autorizada: sin esto, Android pide aceptarla (como un
    // teléfono con la depuración USB) y adb dice «unauthorized».
    let llave = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".android/adbkey")).and_then(|privada| {
        std::fs::read_to_string(privada.with_extension("pub"))
            .ok()
            .or_else(|| Command::new("adb").arg("pubkey").arg(&privada).output().ok().map(|s| String::from_utf8_lossy(&s.stdout).into_owned()))
    });
    if let Some(llave) = llave.map(|l| l.trim().to_string()).filter(|l| config.adb && !l.is_empty() && !l.contains('\'')) {
        adb = format!(
            "k='{llave}'; f=/data/misc/adb/adb_keys; mkdir -p /data/misc/adb; touch $f; grep -qF \"$k\" $f || echo \"$k\" >> $f; \
             chown system:shell $f; chmod 640 $f; {adb}"
        );
    }
    let orden = format!(
        "{BLUETOOTH_SIN_HAL}; {}; settings put system sound_effects_enabled {}; {adb}",
        crate::telefono::orden_operadora(config.operadora.as_deref()),
        config.sonido_toques as u8
    );
    thread::spawn(move || shell(&orden));
}

/// Cierra una app quitando su tarea raíz.
pub fn cerrar_tarea(raiz: u32) {
    docker(&["exec", CONTENEDOR, "am", "stack", "remove", &raiz.to_string()]);
}

/// Configuración de Android con la que está creado el contenedor.
pub fn leer_config_android() -> Option<config::Android> {
    let args = argumentos_contenedor()?;
    if args.is_empty() {
        return None;
    }
    Some(config::Android::desde_argumentos(&args))
}

/// El contenedor usa una imagen distinta de la que hoy tiene su nombre (se reconstruyó).
fn imagen_desactualizada() -> bool {
    let leer = |args: &[&str]| {
        Command::new("docker").args(args).output().ok().map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
    };
    let Some(usada) = leer(&["inspect", "-f", "{{.Image}}", CONTENEDOR]) else { return false };
    let Some(nombre) = leer(&["inspect", "-f", "{{.Config.Image}}", CONTENEDOR]) else { return false };
    let Some(actual) = leer(&["image", "inspect", "-f", "{{.Id}}", &nombre]) else { return false };
    !usada.is_empty() && !actual.is_empty() && usada != actual
}

/// Argumentos de arranque del contenedor (`docker inspect`).
fn argumentos_contenedor() -> Option<Vec<String>> {
    let salida = Command::new("docker").args(["inspect", CONTENEDOR]).output().ok()?;
    let info: serde_json::Value = serde_json::from_slice(&salida.stdout).ok()?;
    Some(
        ["Entrypoint", "Cmd"]
            .iter()
            .flat_map(|k| info[0]["Config"][k].as_array().cloned().unwrap_or_default())
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
    )
}

/// Recrea el contenedor con los mismos ajustes de Docker con que se creó, cambiando solo
/// los argumentos de arranque que maneja ZYKRO. Los datos de Android (volumen) no se tocan.
fn recrear(nueva: &config::Android) -> Result<(), String> {
    let salida = Command::new("docker").args(["inspect", CONTENEDOR]).output().map_err(|e| e.to_string())?;
    let info: serde_json::Value = serde_json::from_slice(&salida.stdout).map_err(|e| e.to_string())?;
    let run = comando_run(&info, nueva)?;
    escribir_telefono(nueva.perfil)?;

    // Se guarda el contenedor viejo con otro nombre por si el nuevo no arranca.
    let respaldo = format!("{CONTENEDOR}-respaldo");
    docker(&["stop", "-t", "5", CONTENEDOR]);
    docker(&["rm", "-f", &respaldo]);
    if !docker(&["rename", CONTENEDOR, &respaldo]) {
        return Err("no se pudo apartar el contenedor actual".into());
    }
    let creado = Command::new("docker").args(&run).output().map_err(|e| e.to_string())?;
    if !creado.status.success() {
        docker(&["rm", "-f", CONTENEDOR]);
        docker(&["rename", &respaldo, CONTENEDOR]);
        return Err(String::from_utf8_lossy(&creado.stderr).trim().to_string());
    }
    docker(&["rm", "-f", &respaldo]);
    Ok(())
}

/// Arma `docker run ...` a partir de `docker inspect` del contenedor actual.
fn comando_run(info: &serde_json::Value, nueva: &config::Android) -> Result<Vec<String>, String> {
    let c = &info[0];
    let (cfg, host) = (&c["Config"], &c["HostConfig"]);
    let cadenas = |v: &serde_json::Value| -> Vec<String> {
        v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
    };
    let imagen = cfg["Image"].as_str().ok_or("el contenedor no tiene imagen")?.to_string();
    let entrada = cadenas(&cfg["Entrypoint"]);
    let mut args: Vec<String> = entrada.iter().skip(1).cloned().chain(cadenas(&cfg["Cmd"])).collect();
    for (clave, valor) in nueva.argumentos() {
        let es_esta = |a: &String| a.split_once('=').map(|(k, _)| k) == Some(clave.as_str());
        match valor {
            Some(valor) => {
                let nuevo = format!("{clave}={valor}");
                match args.iter_mut().find(|a| es_esta(a)) {
                    Some(a) => *a = nuevo,
                    None => args.push(nuevo),
                }
            }
            None => args.retain(|a| !es_esta(a)),
        }
    }

    let mut run: Vec<String> = vec!["run".into(), "-d".into(), "--name".into(), CONTENEDOR.into()];
    if host["Privileged"].as_bool() == Some(true) {
        run.push("--privileged".into());
    }
    for opcion in cadenas(&host["SecurityOpt"]) {
        run.extend(["--security-opt".into(), opcion]);
    }
    if let Some(shm) = host["ShmSize"].as_u64().filter(|&s| s > 0) {
        run.extend(["--shm-size".into(), shm.to_string()]);
    }
    for montaje in cadenas(&host["Binds"]) {
        run.extend(["-v".into(), montaje]);
    }
    for d in host["Devices"].as_array().into_iter().flatten() {
        let (h, c, p) = (d["PathOnHost"].as_str(), d["PathInContainer"].as_str(), d["CgroupPermissions"].as_str());
        if let (Some(h), Some(c), Some(p)) = (h, c, p) {
            run.extend(["--device".into(), format!("{h}:{c}:{p}")]);
        }
    }
    for regla in cadenas(&host["DeviceCgroupRules"]) {
        run.extend(["--device-cgroup-rule".into(), regla]);
    }
    for (puerto, enlaces) in host["PortBindings"].as_object().into_iter().flatten() {
        for e in enlaces.as_array().into_iter().flatten() {
            let ip = e["HostIp"].as_str().unwrap_or("");
            let hp = e["HostPort"].as_str().unwrap_or("");
            let destino = puerto.trim_end_matches("/tcp");
            run.extend(["-p".into(), if ip.is_empty() { format!("{hp}:{destino}") } else { format!("{ip}:{hp}:{destino}") }]);
        }
    }
    if let Some(primero) = entrada.first() {
        run.extend(["--entrypoint".into(), primero.clone()]);
    }
    run.push(imagen);
    run.extend(args);
    Ok(run)
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn solo_apaga_bluetooth_cuando_la_imagen_lo_declara_inhabilitado() {
        let orden = format!(
            "getprop() {{ case \"$1\" in persist.bluetooth.disabled) printf '%s' \"$PRUEBA_DESHABILITADO\";; \
             bluetooth.service.enabled) printf '%s' \"$PRUEBA_SERVICIO\";; esac; }}; \
             settings() {{ printf 'settings %s\\n' \"$*\"; }}; svc() {{ printf 'svc %s\\n' \"$*\"; }}; {}",
            super::BLUETOOTH_SIN_HAL
        );
        for (deshabilitado, servicio, apagar) in [("true", "", true), ("", "false", true), ("false", "true", false), ("", "", false)] {
            let salida = std::process::Command::new("/bin/sh")
                .args(["-c", &orden])
                .env("PRUEBA_DESHABILITADO", deshabilitado)
                .env("PRUEBA_SERVICIO", servicio)
                .output().unwrap();
            assert!(salida.status.success());
            let esperado = if apagar { "settings put global ble_scan_always_enabled 0\nsvc bluetooth disable\n" } else { "" };
            assert_eq!(String::from_utf8(salida.stdout).unwrap(), esperado);
        }
    }

    /// Lista las apps abiertas del Android que está corriendo:
    /// `cargo test tareas_reales -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn tareas_reales() {
        for t in super::tareas_abiertas() {
            println!("tarea raíz {}: {}", t.raiz, t.paquete);
        }
    }

    /// Muestra el comando que recrearía el contenedor actual, sin ejecutarlo:
    /// `cargo test comando_actual -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn comando_actual() {
        let salida = std::process::Command::new("docker").args(["inspect", super::CONTENEDOR]).output().unwrap();
        let info: serde_json::Value = serde_json::from_slice(&salida.stdout).unwrap();
        let actual = super::leer_config_android().unwrap();
        let run = super::comando_run(&info, &actual).unwrap();
        println!("docker {}", run.join(" "));
    }
}

/// Apaga Android al cerrar la ventana, sin esperar a que termine.
pub fn apagar() {
    let _ = Command::new("docker")
        .args(["stop", "-t", "5", CONTENEDOR])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

pub fn instalar_apk(estado: EstadoCompartido) {
    thread::spawn(move || {
        let elegido = Command::new("kdialog")
            .args(["--title", "Instalar APK", "--getopenfilename", ".", "*.apk|Aplicaciones Android"])
            .output();
        let Ok(salida) = elegido else { return };
        let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
        if ruta.is_empty() {
            return;
        }
        let nombre = Path::new(&ruta).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        avisar(&estado, format!("Instalando {nombre}…"));
        let destino = format!("{CONTENEDOR}:/data/local/tmp/zykro.apk");
        let ok = docker(&["cp", &ruta, &destino])
            && docker(&["exec", CONTENEDOR, "pm", "install", "-r", "/data/local/tmp/zykro.apk"]);
        docker(&["exec", CONTENEDOR, "rm", "-f", "/data/local/tmp/zykro.apk"]);
        avisar(&estado, if ok { format!("{nombre} instalada") } else { format!("No se pudo instalar {nombre}") });
    });
}

/// Código de tecla de Android (`AKEYCODE_*`) para una tecla de la laptop.
pub fn tecla_android(tecla: egui::Key) -> Option<u32> {
    use egui::Key::*;
    let letras = [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
    if let Some(i) = letras.iter().position(|&l| l == tecla) {
        return Some(29 + i as u32);
    }
    let numeros = [Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9];
    if let Some(i) = numeros.iter().position(|&n| n == tecla) {
        return Some(7 + i as u32);
    }
    Some(match tecla {
        Space => 62,
        Enter => 66,
        Backspace => 67,
        Delete => 112,
        Tab => 61,
        Escape => 4, // Atrás, como en los otros emuladores
        ArrowUp => 19,
        ArrowDown => 20,
        ArrowLeft => 21,
        ArrowRight => 22,
        Home => 122,
        End => 123,
        PageUp => 92,
        PageDown => 93,
        Minus => 69,
        Equals => 70,
        OpenBracket => 71,
        CloseBracket => 72,
        Backslash => 73,
        Semicolon => 74,
        Quote => 75,
        Slash => 76,
        Comma => 55,
        Period => 56,
        Backtick => 68,
        _ => return None,
    })
}
