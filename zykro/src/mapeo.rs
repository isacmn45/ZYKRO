//! Mapeo de teclas: mismo formato que SwagPlayer/QtScrcpy (`keyMapNodes`, `mouseMoveMap`,
//! `switchKey`) y el motor que convierte teclado y mouse en toques de Android.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, PointerButton, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::control::{Control, BAJAR, MOVER, SOLTAR};
use crate::curva;

// ---------------------------------------------------------------------------------------------
// Formato del archivo
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Debug)]
pub struct Punto {
    pub x: f32,
    pub y: f32,
}

impl Punto {
    pub fn nuevo(x: f32, y: f32) -> Punto {
        Punto { x: x.clamp(0.0, 1.0), y: y.clamp(0.0, 1.0) }
    }
}

fn si() -> bool {
    true
}
fn uno() -> f32 {
    1.0
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Overlay {
    #[serde(default = "si")]
    pub enabled: bool,
    #[serde(default = "opacidad")]
    pub opacity: f32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn opacidad() -> f32 {
    0.7
}

impl Default for Overlay {
    fn default() -> Self {
        Overlay { enabled: true, opacity: 0.7, extra: Map::new() }
    }
}

/// Puntería con el mouse (`KMT_MOUSE_MOVE`).
#[derive(Clone, Serialize, Deserialize)]
pub struct Raton {
    #[serde(rename = "startPos")]
    pub inicio: Punto,
    #[serde(rename = "sensitivityX", default = "uno")]
    pub sens: f32,
    #[serde(rename = "sensitivityRatioY", default = "uno")]
    pub ratio_y: f32,
    /// Bit 0x4000 (`MODO_ABSOLUTO`): posición desde el ancla con la curva en X, como SwagPlayer por defecto.
    #[serde(rename = "tweaks", alias = "Tweaks", default = "tweaks")]
    pub tweaks: u32,
    #[serde(rename = "shootOnClick", default = "si")]
    pub disparo_con_clic: bool,
    #[serde(rename = "shootPos", default, skip_serializing_if = "Option::is_none")]
    pub disparo: Option<Punto>,
    #[serde(rename = "suspendKey", default, skip_serializing_if = "Option::is_none")]
    pub tecla_pausa: Option<String>,
    #[serde(rename = "touchIdleTimeoutMs", default = "espera")]
    pub espera_ms: u64,
    #[serde(rename = "panLimitLeft", default = "limite")]
    pub limite_izq: f32,
    #[serde(rename = "panLimitRight", default = "limite")]
    pub limite_der: f32,
    #[serde(rename = "panLimitTop", default = "limite")]
    pub limite_arr: f32,
    #[serde(rename = "panLimitBottom", default = "limite")]
    pub limite_aba: f32,
    /// `speedRatio*`, `smoothAlpha*`, `velocityCurveK`, `ads*`... van aquí: SwagPlayer no los usa para
    /// mover la cámara, así que ZYKRO tampoco.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn tweaks() -> u32 {
    0x4042
}
fn espera() -> u64 {
    2000
}
fn limite() -> f32 {
    350.0
}

impl Raton {
    pub fn nuevo(inicio: Punto) -> Raton {
        let mut extra = Map::new();
        extra.insert("type".into(), "KMT_MOUSE_MOVE".into());
        Raton {
            inicio,
            sens: 1.0,
            ratio_y: 1.0,
            tweaks: tweaks(),
            disparo_con_clic: true,
            disparo: Some(Punto::nuevo(inicio.x + 0.05, inicio.y + 0.2)),
            tecla_pausa: Some("Key_Control".into()),
            espera_ms: espera(),
            limite_izq: 350.0,
            limite_der: 350.0,
            limite_arr: 350.0,
            limite_aba: 350.0,
            extra,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Clic {
    pub key: String,
    pub pos: Punto,
    #[serde(rename = "switchMap", default)]
    pub cambia_modo: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PasoMulti {
    #[serde(default)]
    pub delay: u64,
    pub pos: Punto,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Multi {
    pub key: String,
    #[serde(rename = "clickNodes", default)]
    pub pasos: Vec<PasoMulti>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Dpad {
    #[serde(rename = "centerPos")]
    pub centro: Punto,
    #[serde(rename = "upKey")]
    pub arriba: String,
    #[serde(rename = "downKey")]
    pub abajo: String,
    #[serde(rename = "leftKey")]
    pub izquierda: String,
    #[serde(rename = "rightKey")]
    pub derecha: String,
    #[serde(rename = "upOffset", default = "desplazamiento")]
    pub d_arriba: f32,
    #[serde(rename = "downOffset", default = "desplazamiento")]
    pub d_abajo: f32,
    #[serde(rename = "leftOffset", default = "desplazamiento")]
    pub d_izquierda: f32,
    #[serde(rename = "rightOffset", default = "desplazamiento")]
    pub d_derecha: f32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn desplazamiento() -> f32 {
    0.05
}

impl Dpad {
    pub fn radio(&self) -> f32 {
        self.d_arriba.max(self.d_abajo).max(self.d_izquierda).max(self.d_derecha)
    }

    pub fn fijar_radio(&mut self, r: f32) {
        let r = r.clamp(0.015, 0.25);
        (self.d_arriba, self.d_abajo, self.d_izquierda, self.d_derecha) = (r, r, r, r);
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Arrastre {
    pub key: String,
    #[serde(rename = "startPos")]
    pub inicio: Punto,
    #[serde(rename = "endPos")]
    pub fin: Punto,
    #[serde(rename = "dragSpeed", default = "uno")]
    pub velocidad: f32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone)]
pub enum Nodo {
    /// `KMT_CLICK` (o `KMT_CLICK_TWICE` si `doble`).
    Clic(Clic, bool),
    Multi(Multi),
    Dpad(Dpad),
    Arrastre(Arrastre),
    /// Tipos que ZYKRO todavía no maneja: se conservan tal cual al guardar.
    Otro(Value),
}

impl Nodo {
    fn desde_json(v: Value) -> Nodo {
        let tipo = v["type"].as_str().unwrap_or("").to_string();
        let r = match tipo.as_str() {
            "KMT_CLICK" => serde_json::from_value(v.clone()).map(|c| Nodo::Clic(c, false)),
            "KMT_CLICK_TWICE" => serde_json::from_value(v.clone()).map(|c| Nodo::Clic(c, true)),
            "KMT_CLICK_MULTI" => serde_json::from_value(v.clone()).map(Nodo::Multi),
            "KMT_STEER_WHEEL" => serde_json::from_value(v.clone()).map(Nodo::Dpad),
            "KMT_DRAG" => serde_json::from_value(v.clone()).map(Nodo::Arrastre),
            _ => return Nodo::Otro(v),
        };
        r.unwrap_or(Nodo::Otro(v))
    }

    fn a_json(&self) -> Value {
        let (tipo, mut v) = match self {
            Nodo::Clic(c, doble) => (if *doble { "KMT_CLICK_TWICE" } else { "KMT_CLICK" }, serde_json::to_value(c)),
            Nodo::Multi(m) => ("KMT_CLICK_MULTI", serde_json::to_value(m)),
            Nodo::Dpad(d) => ("KMT_STEER_WHEEL", serde_json::to_value(d)),
            Nodo::Arrastre(a) => ("KMT_DRAG", serde_json::to_value(a)),
            Nodo::Otro(v) => return v.clone(),
        };
        if let Ok(Value::Object(o)) = &mut v {
            o.insert("type".into(), tipo.into());
        }
        v.unwrap_or(Value::Null)
    }
}

#[derive(Clone)]
pub struct Mapa {
    pub cambio: String,
    pub raton: Option<Raton>,
    pub nodos: Vec<Nodo>,
    pub overlay: Overlay,
    extra: Map<String, Value>,
}

impl Default for Mapa {
    fn default() -> Self {
        Mapa { cambio: "Key_QuoteLeft".into(), raton: None, nodos: Vec::new(), overlay: Overlay::default(), extra: Map::new() }
    }
}

fn carpeta(app: &str) -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join(app).join("keymap")
}

impl Mapa {
    fn desde_json(v: Value) -> Option<Mapa> {
        let Value::Object(mut o) = v else { return None };
        let cambio = o.remove("switchKey").and_then(|v| v.as_str().map(String::from)).unwrap_or("Key_QuoteLeft".into());
        let raton = o.remove("mouseMoveMap").and_then(|v| serde_json::from_value(v).ok());
        let nodos = match o.remove("keyMapNodes") {
            Some(Value::Array(a)) => a.into_iter().map(Nodo::desde_json).collect(),
            _ => Vec::new(),
        };
        let overlay = o.remove("overlaySettings").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        Some(Mapa { cambio, raton, nodos, overlay, extra: o })
    }

    fn a_json(&self) -> Value {
        let mut o = self.extra.clone();
        o.entry("width").or_insert(1920.into());
        o.entry("height").or_insert(1080.into());
        o.insert("switchKey".into(), self.cambio.clone().into());
        if let Some(r) = &self.raton {
            o.insert("mouseMoveMap".into(), serde_json::to_value(r).unwrap_or(Value::Null));
        }
        o.insert("keyMapNodes".into(), Value::Array(self.nodos.iter().map(Nodo::a_json).collect()));
        o.insert("overlaySettings".into(), serde_json::to_value(&self.overlay).unwrap_or(Value::Null));
        Value::Object(o)
    }

    /// Carga el mapeo de una app: primero el de ZYKRO y si no existe, el de SwagPlayer.
    pub fn cargar(paquete: &str) -> Option<Mapa> {
        ["zykro", "swagplayer"].iter().find_map(|app| {
            let datos = std::fs::read(carpeta(app).join(format!("{paquete}.json"))).ok()?;
            Mapa::desde_json(serde_json::from_slice(&datos).ok()?)
        })
    }

    pub fn guardar(&self, paquete: &str) -> std::io::Result<()> {
        let dir = carpeta("zykro");
        std::fs::create_dir_all(&dir)?;
        let datos = serde_json::to_vec_pretty(&self.a_json()).map_err(std::io::Error::other)?;
        std::fs::write(dir.join(format!("{paquete}.json")), datos)
    }
}

// ---------------------------------------------------------------------------------------------
// Nombres de teclas (formato Qt que usa el archivo)
// ---------------------------------------------------------------------------------------------

pub fn nombre_tecla(k: egui::Key) -> Option<String> {
    use egui::Key::*;
    let nombre = k.name();
    if nombre.len() == 1 && nombre.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(nombre.to_ascii_uppercase());
    }
    Some(
        match k {
            Space => "Key_Space",
            Tab => "Key_Tab",
            Backtick => "Key_QuoteLeft",
            Escape => "Key_Escape",
            Enter => "Key_Return",
            Backspace => "Key_Backspace",
            Delete => "Key_Delete",
            Insert => "Key_Insert",
            Home => "Key_Home",
            End => "Key_End",
            PageUp => "Key_PageUp",
            PageDown => "Key_PageDown",
            ArrowUp => "Key_Up",
            ArrowDown => "Key_Down",
            ArrowLeft => "Key_Left",
            ArrowRight => "Key_Right",
            Minus => "Key_Minus",
            Equals => "Key_Equal",
            Comma => "Key_Comma",
            Period => "Key_Period",
            Slash => "Key_Slash",
            Backslash => "Key_Backslash",
            Semicolon => "Key_Semicolon",
            Quote => "Key_Apostrophe",
            OpenBracket => "Key_BracketLeft",
            CloseBracket => "Key_BracketRight",
            F1 => "Key_F1",
            F2 => "Key_F2",
            F3 => "Key_F3",
            F4 => "Key_F4",
            F5 => "Key_F5",
            F6 => "Key_F6",
            F7 => "Key_F7",
            F8 => "Key_F8",
            F9 => "Key_F9",
            F10 => "Key_F10",
            F12 => "Key_F12",
            _ => return None,
        }
        .to_string(),
    )
}

pub fn nombre_boton(b: PointerButton) -> &'static str {
    match b {
        PointerButton::Primary => "LeftButton",
        PointerButton::Secondary => "RightButton",
        PointerButton::Middle => "MiddleButton",
        PointerButton::Extra1 => "XButton1",
        PointerButton::Extra2 => "XButton2",
    }
}

/// Texto corto para mostrar la tecla en pantalla.
pub fn etiqueta(nombre: &str) -> String {
    match nombre {
        "Key_Space" => "Espacio".into(),
        "Key_Shift" => "Shift".into(),
        "Key_Control" => "Ctrl".into(),
        "Key_Alt" => "Alt".into(),
        "Key_QuoteLeft" => "`".into(),
        "Key_Return" => "Enter".into(),
        "Key_Escape" => "Esc".into(),
        "Key_Backspace" => "Borrar".into(),
        "LeftButton" => "Clic izq".into(),
        "RightButton" => "Clic der".into(),
        "MiddleButton" => "Rueda".into(),
        "XButton1" => "Lateral 1".into(),
        "XButton2" => "Lateral 2".into(),
        "Key_Up" => "↑".into(),
        "Key_Down" => "↓".into(),
        "Key_Left" => "←".into(),
        "Key_Right" => "→".into(),
        "" => "?".into(),
        otro => otro.strip_prefix("Key_").unwrap_or(otro).to_string(),
    }
}

// ---------------------------------------------------------------------------------------------
// Motor: teclado y mouse -> toques
// ---------------------------------------------------------------------------------------------

const ID_DPAD: u64 = 1;
const ID_APUNTAR: u64 = 2;
const ID_DISPARO: u64 = 3;
const ID_ARRASTRE: u64 = 4;
const ID_NODO: u64 = 100;
/// Bit de `tweaks`: la posición se calcula desde el ancla con el total acumulado y la curva en X.
const MODO_ABSOLUTO: u32 = 0x4000;
/// Alto de la imagen de BlueStacks maximizado en una pantalla de 1920×1080. BlueStacks mide el
/// movimiento del mouse en píxeles de su ventana y lo pasa a porcentaje de la pantalla de Android:
/// con este tamaño fijo la cámara va como allá, sin depender de la resolución de Android ni del
/// tamaño de la ventana de ZYKRO (en BlueStacks va más lenta cuanto más grande es la ventana).
const ALTO_BLUESTACKS: f64 = 1000.0;
/// Dedo de la cámara, como BlueStacks: se baja en `startPos`, cada movimiento recalcula la posición
/// desde ahí con todo lo que se movió el mouse y queda puesto mientras dure la puntería.
struct Apunte {
    /// Donde se bajó el dedo, en píxeles de Android.
    ancla: (f64, f64),
    /// Cuentas del mouse desde que se bajó el dedo.
    acumulado: (f64, f64),
    /// Último punto mandado.
    punto: (i32, i32),
}

/// Toque programado (doble toque, varios toques, deslizar).
struct Pendiente {
    cuando: Instant,
    accion: u8,
    dedo: u64,
    x: i32,
    y: i32,
}

pub struct Motor {
    pub mapa: Option<Mapa>,
    pub paquete: String,
    /// Modo puntería: el mouse mueve la cámara con el cursor bloqueado.
    pub apuntando: bool,
    pausa: bool,
    presionadas: HashMap<String, bool>,
    dpad: Option<(i32, i32)>,
    apunte: Option<Apunte>,
    /// Posición del dedo en el botón de disparo mientras el clic izquierdo está presionado.
    disparo: Option<Vec2>,
    activos: HashMap<u64, (i32, i32)>,
    pendientes: Vec<Pendiente>,
    /// Canal de control y tamaño de pantalla, para el hilo que lee el mouse directo.
    salida: Option<(Control, f32, f32)>,
}

impl Motor {
    pub fn new() -> Motor {
        Motor {
            mapa: None,
            paquete: String::new(),
            apuntando: false,
            pausa: false,
            presionadas: HashMap::new(),
            dpad: None,
            apunte: None,
            disparo: None,
            activos: HashMap::new(),
            pendientes: Vec::new(),
            salida: None,
        }
    }

    pub fn fijar_salida(&mut self, ctl: Option<Control>, w: f32, h: f32) {
        self.salida = ctl.map(|c| (c, w, h));
    }

    /// Movimiento leído directo del mouse (hilo de raton.rs): un mensaje por cada reporte del mouse.
    pub fn mover_directo(&mut self, delta: Vec2) {
        if let Some((c, w, h)) = self.salida.clone() {
            self.mover(delta, &c, w, h);
        }
    }

    /// Clic leído directo del mouse (hilo de raton.rs).
    pub fn boton_directo(&mut self, boton: PointerButton, presionado: bool) {
        if let Some((c, w, h)) = self.salida.clone() {
            self.boton(boton, presionado, &c, w, h);
        }
    }

    pub fn cambiar_app(&mut self, paquete: &str, ctl: Option<&Control>, w: f32, h: f32) {
        if paquete == self.paquete {
            return;
        }
        if let Some(c) = ctl {
            self.soltar_todo(c, w, h);
        }
        self.paquete = paquete.to_string();
        self.mapa = Mapa::cargar(paquete);
        self.apuntando = false;
    }

    /// El cursor tiene que estar bloqueado (puntería activa y sin pausa).
    pub fn cursor_bloqueado(&self) -> bool {
        self.apuntando && !self.pausa && self.mapa.as_ref().is_some_and(|m| m.raton.is_some())
    }

    fn tocar(&mut self, ctl: &Control, accion: u8, dedo: u64, x: i32, y: i32, w: f32, h: f32) {
        ctl.toque(accion, dedo, x, y, w as u16, h as u16);
        if accion == SOLTAR {
            self.activos.remove(&dedo);
        } else {
            self.activos.insert(dedo, (x, y));
        }
    }

    /// Suelta todos los dedos (al apagar el mapeo, abrir el editor o perder el foco).
    pub fn soltar_todo(&mut self, ctl: &Control, w: f32, h: f32) {
        for (dedo, (x, y)) in std::mem::take(&mut self.activos) {
            ctl.toque(SOLTAR, dedo, x, y, w as u16, h as u16);
        }
        self.presionadas.clear();
        self.dpad = None;
        self.apunte = None;
        self.disparo = None;
        self.pendientes.clear();
        self.pausa = false;
    }

    /// Procesa una tecla (o botón del mouse). Devuelve `true` si está mapeada.
    pub fn tecla(&mut self, nombre: &str, presionada: bool, repeticion: bool, ctl: &Control, w: f32, h: f32) -> bool {
        let Some(mapa) = self.mapa.clone() else { return false };
        if repeticion {
            return self.presionadas.contains_key(nombre) || nombre == mapa.cambio;
        }
        if presionada {
            self.presionadas.insert(nombre.to_string(), true);
        } else {
            self.presionadas.remove(nombre);
        }
        let px = |p: Punto| ((p.x * w) as i32, (p.y * h) as i32);
        let mut usada = false;

        if nombre == mapa.cambio && mapa.raton.is_some() {
            if presionada {
                self.alternar_punteria(ctl, w, h);
            }
            return true;
        }
        if let Some(r) = &mapa.raton {
            if r.tecla_pausa.as_deref() == Some(nombre) {
                self.pausa = presionada;
                if presionada {
                    self.soltar_apunte(ctl, w, h);
                }
                usada = true;
            }
        }

        for (i, nodo) in mapa.nodos.iter().enumerate() {
            let dedo = ID_NODO + i as u64;
            match nodo {
                Nodo::Clic(c, doble) if c.key == nombre => {
                    usada = true;
                    let (x, y) = px(c.pos);
                    if presionada {
                        self.tocar(ctl, BAJAR, dedo, x, y, w, h);
                        if *doble {
                            self.tocar(ctl, SOLTAR, dedo, x, y, w, h);
                            let t = Instant::now() + Duration::from_millis(70);
                            self.programar(t, BAJAR, dedo, x, y);
                            self.programar(t + Duration::from_millis(40), SOLTAR, dedo, x, y);
                        }
                        if c.cambia_modo {
                            self.alternar_punteria(ctl, w, h);
                        }
                    } else if !*doble {
                        self.tocar(ctl, SOLTAR, dedo, x, y, w, h);
                    }
                }
                Nodo::Multi(m) if m.key == nombre => {
                    usada = true;
                    if presionada {
                        let mut t = Instant::now();
                        for paso in &m.pasos {
                            t += Duration::from_millis(paso.delay);
                            let (x, y) = px(paso.pos);
                            self.programar(t, BAJAR, dedo, x, y);
                            self.programar(t + Duration::from_millis(30), SOLTAR, dedo, x, y);
                        }
                    }
                }
                Nodo::Dpad(d) if [&d.arriba, &d.abajo, &d.izquierda, &d.derecha].contains(&&nombre.to_string()) => {
                    usada = true;
                    self.actualizar_dpad(d, ctl, w, h);
                }
                Nodo::Arrastre(a) if a.key == nombre => {
                    usada = true;
                    if presionada {
                        let (x0, y0) = px(a.inicio);
                        let (x1, y1) = px(a.fin);
                        self.tocar(ctl, BAJAR, ID_ARRASTRE, x0, y0, w, h);
                        let pasos = 10;
                        let total = (180.0 / a.velocidad.max(0.1)) as u64;
                        for k in 1..=pasos {
                            let f = k as f32 / pasos as f32;
                            let x = x0 + ((x1 - x0) as f32 * f) as i32;
                            let y = y0 + ((y1 - y0) as f32 * f) as i32;
                            let t = Instant::now() + Duration::from_millis(total * k / pasos);
                            self.programar(t, if k == pasos { SOLTAR } else { MOVER }, ID_ARRASTRE, x, y);
                        }
                    }
                }
                _ => {}
            }
        }
        usada
    }

    /// Botones del mouse. Con la puntería activa, el clic izquierdo dispara.
    pub fn boton(&mut self, boton: PointerButton, presionado: bool, ctl: &Control, w: f32, h: f32) -> bool {
        let nombre = nombre_boton(boton);
        if boton == PointerButton::Primary && self.cursor_bloqueado() {
            let disparo = self.mapa.as_ref().and_then(|m| m.raton.as_ref()).filter(|r| r.disparo_con_clic).and_then(|r| r.disparo);
            if let Some(p) = disparo {
                if presionado && self.disparo.is_some() {
                    return true;
                }
                if presionado {
                    // Como SwagPlayer: el dedo de disparo se queda quieto y la cámara sigue moviéndose
                    // (subir el mouse mientras disparas levanta la mira).
                    let pos = Vec2::new(p.x * w, p.y * h);
                    self.tocar(ctl, BAJAR, ID_DISPARO, pos.x as i32, pos.y as i32, w, h);
                    self.disparo = Some(pos);
                } else if let Some(pos) = self.disparo.take() {
                    self.tocar(ctl, SOLTAR, ID_DISPARO, pos.x as i32, pos.y as i32, w, h);
                }
                return true;
            }
        }
        self.tecla(nombre, presionado, false, ctl, w, h)
    }

    /// Movimiento del mouse en modo puntería, con la fórmula de BlueStacks (`delta` en cuentas crudas;
    /// detalles en docs/investigacion.md, «Cámara de BlueStacks»):
    ///
    /// ```text
    /// x = ancla_x + S(sensX · acumulado_x) / ancho_bs · W      S = curva::horizontal (sin MODO_ABSOLUTO, lineal)
    /// y = ancla_y + sensX · ratioY · acumulado_y / ALTO_BLUESTACKS · H
    /// ```
    ///
    /// SwagPlayer copió esta fórmula pero toma el resultado como píxeles de Android; BlueStacks, como
    /// píxeles de su ventana (`ancho_bs` × `ALTO_BLUESTACKS`, con la forma de la pantalla de Android).
    ///
    /// No recorta a la pantalla ni recentra mientras se mueve: el dedo puede salirse y Android lo
    /// sigue aceptando. Como en BlueStacks, el dedo no se levanta aunque el mouse se quede quieto
    /// (SwagPlayer lo levantaba tras `touchIdleTimeoutMs`, que aquí se ignora): solo al salir de la
    /// puntería o al soltar el mapeo.
    pub fn mover(&mut self, delta: Vec2, ctl: &Control, w: f32, h: f32) {
        let Some(r) = self.mapa.as_ref().and_then(|m| m.raton.clone()) else { return };
        if !self.cursor_bloqueado() || delta == Vec2::ZERO {
            return;
        }
        let apunte = self.apunte.get_or_insert_with(|| {
            let ancla = (r.inicio.x as f64 * w as f64, r.inicio.y as f64 * h as f64);
            let punto = (ancla.0 as i32, ancla.1 as i32);
            ctl.toque(BAJAR, ID_APUNTAR, punto.0, punto.1, w as u16, h as u16);
            Apunte { ancla, acumulado: (0.0, 0.0), punto }
        });
        apunte.acumulado.0 += delta.x as f64;
        apunte.acumulado.1 += delta.y as f64;
        let (sens_x, sens_y) = (r.sens as f64, r.sens as f64 * r.ratio_y as f64);
        let dx = if r.tweaks & MODO_ABSOLUTO != 0 {
            curva::horizontal(sens_x * apunte.acumulado.0)
        } else {
            sens_x * apunte.acumulado.0
        };
        let dy = sens_y * apunte.acumulado.1;
        // De píxeles de la ventana de BlueStacks a píxeles de Android.
        let ancho_bs = ALTO_BLUESTACKS * w as f64 / h as f64;
        let (dx, dy) = (dx / ancho_bs * w as f64, dy / ALTO_BLUESTACKS * h as f64);
        // Se trunca hacia cero, como SwagPlayer.
        let punto = ((apunte.ancla.0 + dx) as i32, (apunte.ancla.1 + dy) as i32);
        apunte.punto = punto;
        ctl.toque(MOVER, ID_APUNTAR, punto.0, punto.1, w as u16, h as u16);
        self.activos.insert(ID_APUNTAR, punto);
    }

    /// Temporizador de los toques programados.
    pub fn tick(&mut self, ctl: &Control, w: f32, h: f32) {
        let ahora = Instant::now();
        let (listos, faltan): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pendientes).into_iter().partition(|p| p.cuando <= ahora);
        self.pendientes = faltan;
        for p in listos {
            self.tocar(ctl, p.accion, p.dedo, p.x, p.y, w, h);
        }
    }

    /// Hay algo pendiente que necesita que la interfaz siga refrescando.
    pub fn ocupado(&self) -> bool {
        !self.pendientes.is_empty()
    }

    fn programar(&mut self, cuando: Instant, accion: u8, dedo: u64, x: i32, y: i32) {
        self.pendientes.push(Pendiente { cuando, accion, dedo, x, y });
    }

    fn alternar_punteria(&mut self, ctl: &Control, w: f32, h: f32) {
        self.apuntando = !self.apuntando;
        if !self.apuntando {
            self.soltar_apunte(ctl, w, h);
        }
    }

    fn soltar_apunte(&mut self, ctl: &Control, w: f32, h: f32) {
        if let Some(a) = self.apunte.take() {
            self.tocar(ctl, SOLTAR, ID_APUNTAR, a.punto.0, a.punto.1, w, h);
        }
        if let Some(pos) = self.disparo.take() {
            self.tocar(ctl, SOLTAR, ID_DISPARO, pos.x as i32, pos.y as i32, w, h);
        }
    }

    fn actualizar_dpad(&mut self, d: &Dpad, ctl: &Control, w: f32, h: f32) {
        let p = |k: &String| self.presionadas.contains_key(k.as_str()) as i32 as f32;
        let dx = p(&d.derecha) * d.d_derecha - p(&d.izquierda) * d.d_izquierda;
        let dy = p(&d.abajo) * d.d_abajo - p(&d.arriba) * d.d_arriba;
        let (cx, cy) = (d.centro.x * w, d.centro.y * h);
        if dx == 0.0 && dy == 0.0 {
            if let Some((x, y)) = self.dpad.take() {
                self.tocar(ctl, SOLTAR, ID_DPAD, x, y, w, h);
            }
            return;
        }
        // Como en QtScrcpy: desplazamiento horizontal relativo al ancho y vertical al alto.
        let destino = ((cx + dx * w) as i32, (cy + dy * h) as i32);
        if self.dpad.is_none() {
            self.tocar(ctl, BAJAR, ID_DPAD, cx as i32, cy as i32, w, h);
        }
        self.tocar(ctl, MOVER, ID_DPAD, destino.0, destino.1, w, h);
        self.dpad = Some(destino);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La puntería manda lo mismo que BlueStacks: bajar en `startPos` y luego la posición calculada
    /// desde el ancla con el total acumulado (curva en X, lineal en Y), un toque por movimiento.
    #[test]
    fn punteria_como_bluestacks() {
        use std::io::Read;
        use std::os::unix::net::UnixListener;

        let ruta = std::env::temp_dir().join(format!("zykro-prueba-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        let servidor = UnixListener::bind(&ruta).unwrap();
        let ctl = Control::conectar(ruta.to_str().unwrap(), false, 0.0).unwrap();
        let (mut socket, _) = servidor.accept().unwrap();
        let _ = std::fs::remove_file(&ruta);

        let mut r = Raton::nuevo(Punto::nuevo(0.79366535, 0.48994160));
        (r.sens, r.ratio_y) = (1.35, 1.15);
        let mut motor = Motor::new();
        motor.mapa = Some(Mapa { raton: Some(r), ..Mapa::default() });
        motor.apuntando = true;
        let (w, h) = (1280.0, 720.0);
        for d in [Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0), Vec2::new(-400.0, 3.0)] {
            motor.mover(d, &ctl, w, h);
        }
        drop(ctl);

        let mut datos = Vec::new();
        socket.read_to_end(&mut datos).unwrap();
        let inicio = 2 + datos[1] as usize + 6; // 0x12 (layout) y 0x14
        let toques: Vec<(u8, i32, i32)> = datos[inicio..]
            .chunks_exact(32)
            .map(|m| {
                assert_eq!(m[0], 0x02);
                let i32_en = |i: usize| i32::from_be_bytes(m[i..i + 4].try_into().unwrap());
                (m[1], i32_en(10), i32_en(14))
            })
            .collect();
        // Calculado aparte en Python con el spline: x = 1015.9 + S(1.35·acum_x) · 1280/1777.8,
        // y = 352.8 + 1.5525·acum_y · 720/1000, acumulando (10, 0), (10, 10) y (−390, 13).
        assert_eq!(toques, vec![(BAJAR, 1015, 352), (MOVER, 1024, 352), (MOVER, 1024, 363), (MOVER, 855, 367)]);
    }

    /// Lee el mapeo de Free Fire de SwagPlayer y comprueba que al volver a guardarlo no se pierde
    /// nada: `cargo test ida_y_vuelta -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn ida_y_vuelta() {
        let ruta = carpeta("swagplayer").join("com.dts.freefireth.json");
        let original: Value = serde_json::from_slice(&std::fs::read(ruta).unwrap()).unwrap();
        let mapa = Mapa::desde_json(original.clone()).unwrap();
        let tipos: Vec<&str> = mapa
            .nodos
            .iter()
            .map(|n| match n {
                Nodo::Clic(_, false) => "clic",
                Nodo::Clic(_, true) => "doble",
                Nodo::Multi(_) => "multi",
                Nodo::Dpad(_) => "dpad",
                Nodo::Arrastre(_) => "arrastre",
                Nodo::Otro(_) => "OTRO",
            })
            .collect();
        println!("nodos: {tipos:?}");
        println!("raton: {}", mapa.raton.is_some());
        let vuelta = mapa.a_json();
        fn claves(v: &Value, ruta: String, salida: &mut Vec<String>) {
            match v {
                Value::Object(o) => o.iter().for_each(|(k, v)| claves(v, format!("{ruta}.{k}"), salida)),
                Value::Array(a) => a.iter().enumerate().for_each(|(i, v)| claves(v, format!("{ruta}[{i}]"), salida)),
                _ => salida.push(ruta),
            }
        }
        let (mut a, mut b) = (Vec::new(), Vec::new());
        claves(&original, String::new(), &mut a);
        claves(&vuelta, String::new(), &mut b);
        let faltan: Vec<_> = a.iter().filter(|k| !b.contains(k)).collect();
        println!("campos originales: {}, después de guardar: {}, perdidos: {faltan:?}", a.len(), b.len());
        assert!(faltan.is_empty());
    }
}
