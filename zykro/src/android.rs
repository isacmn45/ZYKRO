//! Encender Android (el contenedor redroid que creó SwagPlayer) y conectarse a swag-rust-server.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::config::{self, Config};
use crate::control::Control;
use crate::video::{self, Compartido};

pub const CONTENEDOR: &str = "redroid-redroid-1";
const SOCKETS: &str = "/tmp/swag_sockets";

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
    /// Configuración de Android pendiente de aplicar (se aplica recreando el contenedor).
    pub reinicio: Option<config::Android>,
    /// El juego tiene abierto su cuadro de texto (chat): el mapeo se apaga para poder escribir.
    pub escribiendo: bool,
}

pub type EstadoCompartido = Arc<Mutex<Estado>>;

fn docker(args: &[&str]) -> bool {
    Command::new("docker")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
        .spawn(move || loop {
            let pendiente = estado.lock().unwrap().reinicio.take();
            if let Some(nueva) = pendiente {
                mensaje(&estado, Etapa::Encender, "Reiniciando Android con la nueva configuración…");
                ctx.request_repaint();
                if let Err(e) = recrear(&nueva) {
                    error(&estado, format!("No se pudo aplicar la configuración: {e}"));
                    ctx.request_repaint();
                    thread::sleep(Duration::from_secs(4));
                }
            }
            if let Err(e) = conectar_y_recibir(&ctx, &estado, &destino) {
                let reiniciando = {
                    let mut s = estado.lock().unwrap();
                    s.control = None;
                    if s.reinicio.is_none() {
                        s.mensaje = format!("{e}\nReintentando…");
                        s.error = true;
                    }
                    s.reinicio.is_some()
                };
                ctx.request_repaint();
                if !reiniciando {
                    thread::sleep(Duration::from_secs(2));
                }
            }
        })
        .expect("no se pudo crear el hilo de Android");
}

fn conectar_y_recibir(ctx: &egui::Context, estado: &EstadoCompartido, destino: &Compartido) -> std::io::Result<()> {
    if !corriendo() {
        mensaje(estado, Etapa::Encender, "Encendiendo Android…");
        ctx.request_repaint();
        if !docker(&["start", CONTENEDOR]) {
            return Err(std::io::Error::other("No se pudo encender Android. ¿Está corriendo Docker?"));
        }
    }

    mensaje(estado, Etapa::Arrancar, "Esperando a Android…");
    ctx.request_repaint();
    let ruta_video = format!("{SOCKETS}/video.sock");
    let ruta_control = format!("{SOCKETS}/control.sock");
    let inicio = Instant::now();
    let mut socket_video = loop {
        if estado.lock().unwrap().reinicio.is_some() {
            return Err(std::io::Error::other("reinicio pedido"));
        }
        if Path::new(&ruta_video).exists() && Path::new(&ruta_control).exists() {
            match UnixStream::connect(&ruta_video) {
                Ok(socket) => break socket,
                // Mientras Android arranca quedan los sockets de la sesión anterior, sin nadie
                // escuchando: se sigue esperando a que el servidor los vuelva a crear.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {}
                Err(e) => return Err(e),
            }
        }
        if inicio.elapsed() > Duration::from_secs(20) {
            // El servidor borra video.sock cuando otro cliente se conecta.
            mensaje(estado, Etapa::Arrancar, "Esperando a Android…\n¿SwagPlayer está abierto? Ciérralo: solo uno puede conectarse.");
            ctx.request_repaint();
        }
        thread::sleep(Duration::from_millis(300));
    };

    mensaje(estado, Etapa::Conectar, "Conectando…");
    ctx.request_repaint();
    let (android, ocultar_teclado) = {
        let s = estado.lock().unwrap();
        (s.config.android.clone(), s.config.ocultar_teclado)
    };
    let control = Control::conectar(&ruta_control, android.lod_activado, android.lod)?;

    // El servidor no manda nada hasta que la pantalla cambia: apagar y prender la pantalla la redibuja.
    // El servidor atiende a un solo cliente y, si el anterior se cerró de golpe, no se entera hasta
    // que le toca mandar un cuadro: nuestra conexión se queda en cola y no llega la cabecera.
    // Tras dos intentos se reinicia el servidor y se vuelve a conectar.
    socket_video.set_read_timeout(Some(Duration::from_secs(4)))?;
    let mut intentos = 0;
    let pantalla = loop {
        redibujar(&control);
        match video::leer_cabecera(&mut socket_video) {
            Ok(p) => break p,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                intentos += 1;
                if intentos >= 2 {
                    reiniciar_servidor();
                    return Err(std::io::Error::other("El servidor de video no respondía: se reinició"));
                }
            }
            Err(e) => return Err(e),
        }
    };
    socket_video.set_read_timeout(None)?;

    // A veces, tras la cabecera, el primer cuadro no llega; con la pantalla quieta (escritorio,
    // menús) el servidor no manda otro y se quedaba la pantalla de carga hasta tocar algo.
    // Si en 1.5 s no llegó ninguno, se vuelve a redibujar (hasta 3 veces); si ni así, el servidor
    // está trabado: se reinicia y la conexión cortada hace que se vuelva a conectar.
    let llego = Arc::new(AtomicBool::new(false));
    {
        let (llego, control) = (llego.clone(), control.clone());
        thread::spawn(move || {
            for intento in 0..4 {
                thread::sleep(Duration::from_millis(1500));
                if llego.load(Ordering::Relaxed) {
                    return;
                }
                if intento < 3 {
                    redibujar(&control);
                } else {
                    reiniciar_servidor();
                }
            }
        });
    }

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

    let ctx2 = ctx.clone();
    video::recibir_cuadros(socket_video, destino.clone(), move || {
        llego.store(true, Ordering::Relaxed);
        ctx2.request_repaint();
    })
}

/// Reinicia swag-rust-server (un servicio de Android) sin reiniciar Android; corta sus conexiones.
fn reiniciar_servidor() {
    docker(&["exec", CONTENEDOR, "setprop", "ctl.restart", "swag-rust-server"]);
}

/// Apaga y prende la pantalla de Android para que se redibuje (y el servidor mande un cuadro).
fn redibujar(control: &Control) {
    control.pulsar(223); // KEYCODE_SLEEP
    thread::sleep(Duration::from_millis(250));
    control.pulsar(224); // KEYCODE_WAKEUP
}

/// Usa teclado-android (de swagplayer-mods) para que Android no muestre su teclado en pantalla.
fn ocultar_teclado_en_pantalla() {
    if let Some(home) = std::env::var_os("HOME") {
        let script = PathBuf::from(home).join(".local/bin/teclado-android");
        if script.exists() {
            let _ = Command::new(script).arg("ocultar").stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        }
    }
}

/// Vigila los cambios de foco de Android para saber cuándo un juego abre su cuadro de texto
/// (en Unity es una ventana aparte que pide el teclado siempre visible).
pub fn vigilar_texto(ctx: egui::Context, estado: EstadoCompartido) {
    use std::io::{BufRead, BufReader};
    thread::Builder::new()
        .name("zykro-texto".into())
        .spawn(move || loop {
            let hijo = Command::new("docker")
                .args(["exec", CONTENEDOR, "logcat", "-b", "events", "-s", "input_focus", "-T", "1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut hijo) = hijo {
                let mut cuadro: Option<String> = None;
                for linea in BufReader::new(hijo.stdout.take().unwrap()).lines().skip(1).map_while(Result::ok) {
                    let Some(resto) = linea.split("Focus ").nth(1) else { continue };
                    let mut partes = resto.split_whitespace();
                    let (Some(accion), Some(ventana)) = (partes.next(), partes.next()) else { continue };
                    let nuevo = match accion {
                        "entering" if es_cuadro_texto(ventana) => Some(ventana.to_string()),
                        "entering" => None,
                        "leaving" if cuadro.as_deref() == Some(ventana) => None,
                        _ => continue,
                    };
                    if nuevo.is_some() != cuadro.is_some() {
                        estado.lock().unwrap().escribiendo = nuevo.is_some();
                        ctx.request_repaint();
                    }
                    cuadro = nuevo;
                }
                let _ = hijo.wait();
            }
            estado.lock().unwrap().escribiendo = false;
            thread::sleep(Duration::from_secs(2));
        })
        .expect("no se pudo crear el hilo de texto");
}

fn es_cuadro_texto(ventana: &str) -> bool {
    let Ok(salida) = Command::new("docker").args(["exec", CONTENEDOR, "dumpsys", "window", "windows"]).output() else {
        return false;
    };
    let texto = String::from_utf8_lossy(&salida.stdout);
    let Some(resto) = texto.split(&format!("Window{{{ventana} ")).nth(1) else { return false };
    let bloque: String = resto.chars().take(3000).collect();
    !bloque.contains("BASE_APPLICATION") && bloque.contains("always_visible")
}

/// Paquete de la app en primer plano, según swag-rust-server.
pub fn app_activa() -> Option<String> {
    let texto = std::fs::read_to_string(format!("{SOCKETS}/active_app")).ok()?;
    texto.lines().find_map(|l| l.strip_prefix("package=").or_else(|| l.strip_prefix("Package=")).map(|p| p.trim().to_string()))
}

/// Configuración de Android con la que está creado el contenedor.
pub fn leer_config_android() -> Option<config::Android> {
    let salida = Command::new("docker").args(["inspect", CONTENEDOR]).output().ok()?;
    let info: serde_json::Value = serde_json::from_slice(&salida.stdout).ok()?;
    let args: Vec<String> = ["Entrypoint", "Cmd"]
        .iter()
        .flat_map(|k| info[0]["Config"][k].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    if args.is_empty() {
        return None;
    }
    Some(config::Android::desde_argumentos(&args))
}

/// Pide aplicar una nueva configuración de Android: corta la conexión apagando el contenedor y
/// el hilo de Android lo recrea.
pub fn pedir_reinicio(estado: &EstadoCompartido, nueva: config::Android) {
    estado.lock().unwrap().reinicio = Some(nueva);
    thread::spawn(|| docker(&["stop", "-t", "5", CONTENEDOR]));
}

/// Recrea el contenedor con los mismos ajustes de Docker que le puso SwagPlayer, cambiando solo
/// los argumentos de arranque que maneja ZYKRO. Los datos de Android (volumen) no se tocan.
fn recrear(nueva: &config::Android) -> Result<(), String> {
    let salida = Command::new("docker").args(["inspect", CONTENEDOR]).output().map_err(|e| e.to_string())?;
    let info: serde_json::Value = serde_json::from_slice(&salida.stdout).map_err(|e| e.to_string())?;
    let run = comando_run(&info, nueva)?;

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
        let nuevo = format!("{clave}={valor}");
        match args.iter_mut().find(|a| a.split_once('=').map(|(k, _)| k) == Some(clave)) {
            Some(a) => *a = nuevo,
            None => args.push(nuevo),
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
