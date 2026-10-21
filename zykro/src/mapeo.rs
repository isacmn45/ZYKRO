//! Mapeo de teclas: mismo formato que SwagPlayer/QtScrcpy (`keyMapNodes`, `mouseMoveMap`,
//! `switchKey`) y el motor que convierte teclado y mouse en toques de Android.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, PointerButton, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::control::{Control, BAJAR, MOVER, SOLTAR};

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
    #[serde(rename = "speedRatioX", default = "uno")]
    pub vel_x: f32,
    #[serde(rename = "speedRatioY", default = "uno")]
    pub vel_y: f32,
    #[serde(rename = "shootOnClick", default = "si")]
    pub disparo_con_clic: bool,
    #[serde(rename = "shootPos", default, skip_serializing_if = "Option::is_none")]
    pub disparo: Option<Punto>,
    #[serde(rename = "adsEnabled", default)]
    pub ads: bool,
    #[serde(rename = "adsKey", default, skip_serializing_if = "Option::is_none")]
    pub tecla_ads: Option<String>,
    #[serde(rename = "adsSensMultiplier", default = "medio")]
    pub mult_ads: f32,
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
    /// Suavizado por milisegundo (como SwagPlayer): fracción del camino que avanza el dedo en cada ms.
    #[serde(rename = "smoothAlphaMin", default, skip_serializing_if = "Option::is_none")]
    pub suave_min: Option<f32>,
    #[serde(rename = "smoothAlphaMax", default, skip_serializing_if = "Option::is_none")]
    pub suave_max: Option<f32>,
    #[serde(rename = "smoothSpeedThresh", default, skip_serializing_if = "Option::is_none")]
    pub suave_umbral: Option<f32>,
    #[serde(rename = "adsSmoothAlphaMin", default, skip_serializing_if = "Option::is_none")]
    pub ads_suave_min: Option<f32>,
    #[serde(rename = "adsSmoothAlphaMax", default, skip_serializing_if = "Option::is_none")]
    pub ads_suave_max: Option<f32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn medio() -> f32 {
    0.5
}
fn espera() -> u64 {
    2500
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
            vel_x: 1.0,
            vel_y: 1.0,
            disparo_con_clic: true,
            disparo: Some(Punto::nuevo(inicio.x + 0.05, inicio.y + 0.2)),
            ads: true,
            tecla_ads: Some("RightButton".into()),
            mult_ads: 0.5,
            tecla_pausa: Some("Key_Control".into()),
            espera_ms: 2500,
            limite_izq: 350.0,
            limite_der: 350.0,
            limite_arr: 350.0,
            limite_aba: 350.0,
            suave_min: Some(0.04),
            suave_max: Some(0.55),
            suave_umbral: Some(12.0),
            ads_suave_min: None,
            ads_suave_max: None,
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
/// Cuánto mueve la cámara cada unidad de movimiento del mouse con sensibilidad 1.
const FACTOR_RATON: f32 = 0.55;

/// Dedo de la cámara. `objetivo` es adonde mandó el mouse; `actual`, por donde va el dedo.
struct Apunte {
    actual: Vec2,
    objetivo: Vec2,
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
    ads: bool,
    presionadas: HashMap<String, bool>,
    dpad: Option<(i32, i32)>,
    apunte: Option<Apunte>,
    /// Posición del dedo en el botón de disparo mientras el clic izquierdo está presionado.
    disparo: Option<Vec2>,
    /// La puntería avanza sola cada milisegundo (hilo de raton.rs) con suavizado.
    suavizado_directo: bool,
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
            ads: false,
            presionadas: HashMap::new(),
            dpad: None,
            apunte: None,
            disparo: None,
            suavizado_directo: false,
            activos: HashMap::new(),
            pendientes: Vec::new(),
            salida: None,
        }
    }

    pub fn fijar_salida(&mut self, ctl: Option<Control>, w: f32, h: f32) {
        self.salida = ctl.map(|c| (c, w, h));
    }

    /// Movimiento leído directo del mouse (hilo de raton.rs). El dedo lo avanza `paso_directo`.
    pub fn mover_directo(&mut self, delta: Vec2) {
        if let Some((c, w, h)) = self.salida.clone() {
            self.suavizado_directo = true;
            self.mover(delta, &c, w, h);
        }
    }

    /// Un paso de 1 ms del dedo de la cámara hacia donde mandó el mouse (hilo de raton.rs).
    /// Igual que SwagPlayer: se manda la posición cada milisegundo aunque no cambie.
    pub fn paso_directo(&mut self) {
        let Some((c, w, h)) = self.salida.clone() else { return };
        let Some(r) = self.mapa.as_ref().and_then(|m| m.raton.as_ref()) else { return };
        let (min, max) = if self.ads {
            (r.ads_suave_min.or(r.suave_min), r.ads_suave_max.or(r.suave_max))
        } else {
            (r.suave_min, r.suave_max)
        };
        let umbral = r.suave_umbral.unwrap_or(12.0).max(0.1);
        let Some(a) = self.apunte.as_mut() else { return };
        let falta = a.objetivo - a.actual;
        let alfa = match (min, max) {
            (Some(min), Some(max)) => (min + (max - min) * (falta.length() / umbral).clamp(0.0, 1.0)).clamp(0.005, 1.0),
            _ => 1.0,
        };
        a.actual = if falta.length() < 0.05 { a.objetivo } else { a.actual + falta * alfa };
        let (x, y) = (a.actual.x.round() as i32, a.actual.y.round() as i32);
        c.toque(MOVER, ID_APUNTAR, x, y, w as u16, h as u16);
        self.activos.insert(ID_APUNTAR, (x, y));
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
        self.ads = false;
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
            if r.ads && r.tecla_ads.as_deref() == Some(nombre) {
                self.ads = presionada;
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

    /// Movimiento del mouse en modo puntería. Como SwagPlayer, el dedo de la cámara nunca se
    /// levanta ni se recentra: puede salirse de la pantalla y Android lo sigue aceptando. Así el
    /// juego no pierde el arrastre (levantarlo causaba tirones y la mira se quedaba pegada al disparar).
    pub fn mover(&mut self, delta: Vec2, ctl: &Control, w: f32, h: f32) {
        let Some(r) = self.mapa.as_ref().and_then(|m| m.raton.clone()) else { return };
        if !self.cursor_bloqueado() {
            return;
        }
        let escala = FACTOR_RATON * w / 1280.0 * if self.ads { r.mult_ads } else { 1.0 };
        let d = Vec2::new(delta.x * r.sens * r.vel_x, delta.y * r.sens * r.ratio_y * r.vel_y) * escala;
        let inicio = Vec2::new(r.inicio.x * w, r.inicio.y * h);
        let apunte = self.apunte.get_or_insert_with(|| {
            ctl.toque(BAJAR, ID_APUNTAR, inicio.x as i32, inicio.y as i32, w as u16, h as u16);
            Apunte { actual: inicio, objetivo: inicio }
        });
        apunte.objetivo += d;
        // Solo por seguridad tras mucho rato: recentrar si el dedo se fue demasiado lejos.
        if apunte.objetivo.x.abs() > 30000.0 || apunte.objetivo.y.abs() > 30000.0 {
            ctl.toque(SOLTAR, ID_APUNTAR, apunte.actual.x as i32, apunte.actual.y as i32, w as u16, h as u16);
            ctl.toque(BAJAR, ID_APUNTAR, inicio.x as i32, inicio.y as i32, w as u16, h as u16);
            *apunte = Apunte { actual: inicio, objetivo: inicio };
        }
        if !self.suavizado_directo {
            // Sin el hilo del mouse: el dedo va directo adonde mandó el mouse.
            apunte.actual = apunte.objetivo;
            let (x, y) = (apunte.actual.x as i32, apunte.actual.y as i32);
            ctl.toque(MOVER, ID_APUNTAR, x, y, w as u16, h as u16);
        }
        self.activos.insert(ID_APUNTAR, (apunte.actual.x as i32, apunte.actual.y as i32));
    }

    /// Temporizadores: toques programados y soltar el dedo de la cámara si el mouse se quedó quieto.
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
            self.tocar(ctl, SOLTAR, ID_APUNTAR, a.actual.x as i32, a.actual.y as i32, w, h);
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
