//! Mapeo de teclas: mismo formato que SwagPlayer/QtScrcpy (`keyMapNodes`, `mouseMoveMap`,
//! `switchKey`) y el motor que convierte teclado y mouse en toques de Android. Además de los de
//! QtScrcpy, tiene los controles de BlueStacks 5 (tipos `ZYKRO_*`): toques repetidos, mirada libre,
//! script, deslizar en cuatro direcciones, zoom, D-pad y habilidades MOBA, rotar, desplazamiento de
//! borde, desplazarse y rueda del mouse. Cada juego puede tener varios esquemas de controles.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, PointerButton, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::control::{Control, BAJAR, MOVER, SOLTAR};
use crate::{curva, script};

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
fn cien() -> f32 {
    100.0
}
fn blanco() -> String {
    "#FFFFFF".into()
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
    /// Punto de mira en el centro de la imagen mientras apuntas.
    #[serde(rename = "crosshair", default)]
    pub mira: Mira,
    /// Aceleración del mouse: cuanto más rápido lo mueves, más gira la cámara.
    #[serde(rename = "mouseAcceleration", default)]
    pub aceleracion: bool,
    /// Mirar alrededor sin mover la mira («Look around mode» de BlueStacks): su tecla y el botón del ojo.
    #[serde(rename = "freeLookKey", default, skip_serializing_if = "Option::is_none")]
    pub mirada_tecla: Option<String>,
    #[serde(rename = "freeLookPos", default, skip_serializing_if = "Option::is_none")]
    pub mirada_pos: Option<Punto>,
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
            mira: Mira::default(),
            aceleracion: false,
            mirada_tecla: None,
            mirada_pos: None,
            extra,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub enum EstiloMira {
    #[default]
    Cruz,
    Punto,
    Circulo,
    CruzPunto,
}

impl EstiloMira {
    pub const TODOS: [(EstiloMira, &'static str); 4] =
        [(EstiloMira::Cruz, "Cruz"), (EstiloMira::Punto, "Punto"), (EstiloMira::Circulo, "Círculo"), (EstiloMira::CruzPunto, "Cruz con punto")];
}

/// El punto de mira: estilo, tamaño (1 a 5), opacidad (0 a 100) y color en hexadecimal.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Mira {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub style: EstiloMira,
    #[serde(default = "uno")]
    pub size: f32,
    #[serde(default = "cien")]
    pub opacity: f32,
    #[serde(default = "blanco")]
    pub color: String,
}

impl Default for Mira {
    fn default() -> Self {
        Mira { enabled: false, style: EstiloMira::Cruz, size: 1.0, opacity: 100.0, color: blanco() }
    }
}

impl Mira {
    pub fn color32(&self) -> egui::Color32 {
        let hex = self.color.trim_start_matches('#');
        let canal = |i: usize| hex.get(i..i + 2).and_then(|c| u8::from_str_radix(c, 16).ok()).unwrap_or(255);
        egui::Color32::from_rgb(canal(0), canal(2), canal(4)).gamma_multiply((self.opacity / 100.0).clamp(0.0, 1.0))
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
    /// Modificadores de velocidad: mientras se presiona su tecla, el D-pad usa ese radio (por
    /// ejemplo, Shift para correr). Radio 0 = sin modificador.
    #[serde(rename = "modifier1Key", default, skip_serializing_if = "String::is_empty")]
    pub tecla1: String,
    #[serde(rename = "modifier1Radius", default)]
    pub radio1: f32,
    #[serde(rename = "modifier2Key", default, skip_serializing_if = "String::is_empty")]
    pub tecla2: String,
    #[serde(rename = "modifier2Radius", default)]
    pub radio2: f32,
    /// Velocidad del deslizamiento del dedo (pantallas por segundo); 0 = de golpe.
    #[serde(rename = "swipeSpeed", default)]
    pub velocidad: f32,
    /// Espera desde que presionas hasta que el D-pad se mueve, y velocidad de ese primer movimiento.
    #[serde(rename = "activationTime", default)]
    pub activacion_ms: u64,
    #[serde(rename = "activationSpeed", default)]
    pub velocidad_inicial: f32,
    /// Hasta este radio el dedo salta de golpe; desde ahí desliza.
    #[serde(rename = "deadZone", default)]
    pub zona_muerta: f32,
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

fn cinco() -> u32 {
    5
}
fn diez() -> f32 {
    10.0
}
fn radio() -> f32 {
    0.08
}
fn velocidad() -> f32 {
    1.0
}
fn trescientos() -> u64 {
    300
}

/// Toques repetidos: `veces` toques al presionar, o sin parar mientras la tecla siga presionada.
#[derive(Clone, Serialize, Deserialize)]
pub struct Repetido {
    pub key: String,
    pub pos: Punto,
    #[serde(rename = "count", default = "cinco")]
    pub veces: u32,
    #[serde(rename = "untilKeyUp", default)]
    pub hasta_soltar: bool,
    #[serde(rename = "tapsPerSecond", default = "diez")]
    pub por_segundo: f32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Mirada libre: mientras se presiona la tecla, el dedo queda en el botón del ojo y el mouse (o las
/// flechas) lo arrastran para mirar alrededor.
#[derive(Clone, Serialize, Deserialize)]
pub struct MiradaLibre {
    pub key: String,
    pub pos: Punto,
    /// Con el teclado (las cuatro teclas) en vez del mouse.
    #[serde(rename = "keyboard", default)]
    pub teclado: bool,
    #[serde(rename = "upKey", default)]
    pub arriba: String,
    #[serde(rename = "downKey", default)]
    pub abajo: String,
    #[serde(rename = "leftKey", default)]
    pub izquierda: String,
    #[serde(rename = "rightKey", default)]
    pub derecha: String,
    /// Sensibilidad con el mouse, o velocidad (pantallas por segundo) con el teclado.
    #[serde(rename = "swipeSpeed", default = "velocidad")]
    pub velocidad: f32,
    /// Al llegar a la orilla, el dedo se levanta y vuelve al ojo tras esta espera.
    #[serde(rename = "edgeResetDelay", default = "trescientos")]
    pub espera_orilla_ms: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Script: comandos de BlueStacks (ver script.rs) que corren con una tecla.
#[derive(Clone, Serialize, Deserialize)]
pub struct ScriptNodo {
    pub key: String,
    pub pos: Punto,
    #[serde(rename = "script", default)]
    pub texto: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Deslizar en cuatro direcciones desde `pos`, con su tecla cada una.
#[derive(Clone, Serialize, Deserialize)]
pub struct Deslizar {
    pub pos: Punto,
    #[serde(rename = "upKey", default)]
    pub arriba: String,
    #[serde(rename = "downKey", default)]
    pub abajo: String,
    #[serde(rename = "leftKey", default)]
    pub izquierda: String,
    #[serde(rename = "rightKey", default)]
    pub derecha: String,
    /// Largo del deslizamiento (en altos de pantalla).
    #[serde(rename = "radius", default = "radio")]
    pub radio: f32,
    /// Pantallas por segundo.
    #[serde(rename = "speed", default = "velocidad")]
    pub velocidad: f32,
    /// El dedo se queda al final mientras la tecla siga presionada.
    #[serde(rename = "swipeHold", default)]
    pub mantener: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Zoom con dos dedos (pellizcar) en `pos`: con sus teclas o con la rueda del mouse.
#[derive(Clone, Serialize, Deserialize)]
pub struct Zoom {
    pub pos: Punto,
    #[serde(rename = "zoomInKey", default)]
    pub acercar: String,
    #[serde(rename = "zoomOutKey", default)]
    pub alejar: String,
    /// Si tiene, el zoom solo funciona con esta tecla también presionada.
    #[serde(rename = "modifierKey", default, skip_serializing_if = "String::is_empty")]
    pub modificador: String,
    /// Cuánto se separan o juntan los dedos con cada paso de la rueda (en altos de pantalla).
    #[serde(rename = "amplitude", default = "radio")]
    pub amplitud: f32,
    /// Con la tecla presionada, el zoom se acelera hasta este múltiplo.
    #[serde(rename = "acceleration", default = "velocidad")]
    pub aceleracion: f32,
    /// Pantallas por segundo.
    #[serde(rename = "speed", default = "velocidad")]
    pub velocidad: f32,
    /// Modo 1: un dedo se mueve más rápido que el otro (como hace una mano).
    #[serde(rename = "mode", default)]
    pub modo: u8,
    /// Espera antes de levantar los dedos.
    #[serde(rename = "resetDelay", default = "trescientos")]
    pub espera_ms: u64,
    /// Solo sus teclas; si no, también Ctrl + = y Ctrl + -.
    #[serde(rename = "override", default)]
    pub exclusivo: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// D-pad MOBA: clic derecho mueve al personaje hacia el cursor (como en un MOBA de PC).
#[derive(Clone, Serialize, Deserialize)]
pub struct MobaDpad {
    pub pos: Punto,
    #[serde(rename = "radius", default = "radio")]
    pub radio: f32,
    /// Dónde está el personaje en la pantalla (para saber hacia dónde moverlo).
    #[serde(rename = "heroPos")]
    pub heroe: Punto,
    /// Pantallas por segundo que recorre el personaje: decide cuánto dura cada movimiento.
    #[serde(rename = "heroSpeed", default = "velocidad")]
    pub velocidad_heroe: f32,
    /// Tecla para detener el movimiento (vacía = sin ella).
    #[serde(rename = "cancelKey", default, skip_serializing_if = "String::is_empty")]
    pub cancelar: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Habilidad MOBA: lanza la habilidad de `pos` hacia el cursor.
#[derive(Clone, Serialize, Deserialize)]
pub struct MobaHabilidad {
    pub key: String,
    pub pos: Punto,
    #[serde(rename = "radius", default = "radio")]
    pub radio: f32,
    /// 0: rápido (apunta con el mouse y lanza al soltar la tecla); 1: automático (un toque); 2: manual
    /// (apunta con el mouse y lanza con clic izquierdo).
    #[serde(rename = "castMode", default)]
    pub modo: u8,
    /// Al lanzar, el D-pad MOBA deja de mover al personaje.
    #[serde(rename = "stopMobaDpad", default)]
    pub detener_dpad: bool,
    /// Tecla y botón para cancelar la habilidad mientras apuntas (vacía = sin ella).
    #[serde(rename = "cancelKey", default, skip_serializing_if = "String::is_empty")]
    pub cancelar: String,
    #[serde(rename = "cancelPos", default, skip_serializing_if = "Option::is_none")]
    pub cancelar_pos: Option<Punto>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Rotar: un dedo gira alrededor de `pos` mientras se presiona su tecla (un volante, una rueda).
#[derive(Clone, Serialize, Deserialize)]
pub struct Rotar {
    pub pos: Punto,
    #[serde(rename = "clockwiseKey", default)]
    pub horario: String,
    #[serde(rename = "anticlockwiseKey", default)]
    pub antihorario: String,
    #[serde(rename = "radius", default = "radio")]
    pub radio: f32,
    /// Ángulo donde empieza (grados; 0 = a la derecha, 90 = abajo).
    #[serde(rename = "startAngle", default)]
    pub angulo: f32,
    /// Grados por segundo.
    #[serde(rename = "speed", default = "noventa")]
    pub velocidad: f32,
    #[serde(rename = "activationTime", default)]
    pub activacion_ms: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn noventa() -> f32 {
    90.0
}

/// Desplazamiento de borde: con el cursor en una orilla de la imagen, un dedo arrastra el mapa
/// hacia allá (juegos de estrategia).
#[derive(Clone, Serialize, Deserialize)]
pub struct Borde {
    pub pos: Punto,
    /// Pantallas por segundo en cada eje.
    #[serde(rename = "xVelocity", default = "medio")]
    pub vel_x: f32,
    #[serde(rename = "yVelocity", default = "medio")]
    pub vel_y: f32,
    /// Ancho de la orilla activa (partes de la imagen).
    #[serde(rename = "xActiveMargin", default = "margen")]
    pub margen_x: f32,
    #[serde(rename = "yActiveMargin", default = "margen")]
    pub margen_y: f32,
    /// Tras `acelerar_ms` en la orilla, la velocidad se multiplica por esto.
    #[serde(rename = "speedUpFactor", default = "dos")]
    pub factor: f32,
    #[serde(rename = "speedUpWaitTime", default = "mil")]
    pub acelerar_ms: u64,
    /// Espera entre un arrastre y el siguiente.
    #[serde(rename = "resetDelay", default = "cincuenta")]
    pub espera_ms: u64,
    #[serde(default = "si")]
    pub enabled: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn medio() -> f32 {
    0.5
}
fn margen() -> f32 {
    0.03
}
fn dos() -> f32 {
    2.0
}
fn mil() -> u64 {
    1000
}
fn cincuenta() -> u64 {
    50
}

/// Desplazarse: la rueda del mouse arrastra en `pos` (listas, inventarios).
#[derive(Clone, Serialize, Deserialize)]
pub struct Desplazar {
    pub pos: Punto,
    /// Pantallas por segundo.
    #[serde(rename = "speed", default = "dos")]
    pub velocidad: f32,
    /// Largo de cada arrastre (altos de pantalla).
    #[serde(rename = "amplitude", default = "radio")]
    pub amplitud: f32,
    /// Solo este control usa la rueda (si no, la rueda también desplaza donde está el cursor).
    #[serde(rename = "override", default = "si")]
    pub exclusivo: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Rueda del mouse: un toque arriba y otro abajo (cambiar de arma, por ejemplo).
#[derive(Clone, Serialize, Deserialize)]
pub struct RuedaNodo {
    #[serde(rename = "upPos")]
    pub arriba: Punto,
    #[serde(rename = "downPos")]
    pub abajo: Punto,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone)]
pub enum Nodo {
    /// `KMT_CLICK` (o `KMT_CLICK_TWICE` si `doble`).
    Clic(Clic, bool),
    Multi(Multi),
    Dpad(Dpad),
    /// `KMT_DRAG` de QtScrcpy: de un punto a otro con una tecla.
    Arrastre(Arrastre),
    Repetido(Repetido),
    MiradaLibre(MiradaLibre),
    Script(ScriptNodo),
    Deslizar(Deslizar),
    Zoom(Zoom),
    MobaDpad(MobaDpad),
    MobaHabilidad(MobaHabilidad),
    Rotar(Rotar),
    Borde(Borde),
    Desplazar(Desplazar),
    Rueda(RuedaNodo),
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
            "ZYKRO_REPEAT" => serde_json::from_value(v.clone()).map(Nodo::Repetido),
            "ZYKRO_FREE_LOOK" => serde_json::from_value(v.clone()).map(Nodo::MiradaLibre),
            "ZYKRO_SCRIPT" => serde_json::from_value(v.clone()).map(Nodo::Script),
            "ZYKRO_SWIPE" => serde_json::from_value(v.clone()).map(Nodo::Deslizar),
            "ZYKRO_ZOOM" => serde_json::from_value(v.clone()).map(Nodo::Zoom),
            "ZYKRO_MOBA_DPAD" => serde_json::from_value(v.clone()).map(Nodo::MobaDpad),
            "ZYKRO_MOBA_SKILL" => serde_json::from_value(v.clone()).map(Nodo::MobaHabilidad),
            "ZYKRO_ROTATE" => serde_json::from_value(v.clone()).map(Nodo::Rotar),
            "ZYKRO_EDGE_SCROLL" => serde_json::from_value(v.clone()).map(Nodo::Borde),
            "ZYKRO_SCROLL" => serde_json::from_value(v.clone()).map(Nodo::Desplazar),
            "ZYKRO_MOUSE_WHEEL" => serde_json::from_value(v.clone()).map(Nodo::Rueda),
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
            Nodo::Repetido(n) => ("ZYKRO_REPEAT", serde_json::to_value(n)),
            Nodo::MiradaLibre(n) => ("ZYKRO_FREE_LOOK", serde_json::to_value(n)),
            Nodo::Script(n) => ("ZYKRO_SCRIPT", serde_json::to_value(n)),
            Nodo::Deslizar(n) => ("ZYKRO_SWIPE", serde_json::to_value(n)),
            Nodo::Zoom(n) => ("ZYKRO_ZOOM", serde_json::to_value(n)),
            Nodo::MobaDpad(n) => ("ZYKRO_MOBA_DPAD", serde_json::to_value(n)),
            Nodo::MobaHabilidad(n) => ("ZYKRO_MOBA_SKILL", serde_json::to_value(n)),
            Nodo::Rotar(n) => ("ZYKRO_ROTATE", serde_json::to_value(n)),
            Nodo::Borde(n) => ("ZYKRO_EDGE_SCROLL", serde_json::to_value(n)),
            Nodo::Desplazar(n) => ("ZYKRO_SCROLL", serde_json::to_value(n)),
            Nodo::Rueda(n) => ("ZYKRO_MOUSE_WHEEL", serde_json::to_value(n)),
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

    /// Guarda el mapeo como el de la app y como su esquema (`<app>.schemes/<esquema>.json`).
    pub fn guardar(&self, paquete: &str) -> std::io::Result<()> {
        let dir = carpeta("zykro");
        std::fs::create_dir_all(&dir)?;
        let datos = serde_json::to_vec_pretty(&self.a_json()).map_err(std::io::Error::other)?;
        std::fs::write(dir.join(format!("{paquete}.json")), &datos)?;
        let esquemas = carpeta_esquemas(paquete);
        std::fs::create_dir_all(&esquemas)?;
        std::fs::write(esquemas.join(format!("{}.json", archivo_esquema(&self.esquema()))), datos)
    }

    /// El esquema de controles al que pertenece (se guarda dentro del archivo).
    pub fn esquema(&self) -> String {
        self.extra.get("zykroScheme").and_then(|v| v.as_str()).unwrap_or(PREDETERMINADO).to_string()
    }

    pub fn poner_esquema(&mut self, nombre: &str) {
        self.extra.insert("zykroScheme".into(), nombre.into());
    }

    pub fn exportar(&self, ruta: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(ruta, serde_json::to_vec_pretty(&self.a_json()).map_err(std::io::Error::other)?)
    }

    pub fn importar(ruta: &std::path::Path) -> Option<Mapa> {
        Mapa::desde_json(serde_json::from_slice(&std::fs::read(ruta).ok()?).ok()?)
    }
}

/// Nombre del esquema de siempre de cada juego.
pub const PREDETERMINADO: &str = "Predeterminado";

fn carpeta_esquemas(paquete: &str) -> PathBuf {
    carpeta("zykro").join(format!("{paquete}.schemes"))
}

/// El nombre de un esquema como nombre de archivo (sin barras).
fn archivo_esquema(nombre: &str) -> String {
    nombre.chars().map(|c| if c == '/' || c == '\\' || c == '\0' { '-' } else { c }).collect()
}

/// Los esquemas de controles guardados de una app (el predeterminado primero).
pub fn esquemas(paquete: &str) -> Vec<String> {
    let mut lista: Vec<String> = std::fs::read_dir(carpeta_esquemas(paquete))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").map(String::from))
        .collect();
    lista.sort_by_key(|n| (n != PREDETERMINADO, n.to_lowercase()));
    lista
}

pub fn cargar_esquema(paquete: &str, nombre: &str) -> Option<Mapa> {
    let ruta = carpeta_esquemas(paquete).join(format!("{}.json", archivo_esquema(nombre)));
    let mut mapa = Mapa::importar(&ruta)?;
    mapa.poner_esquema(nombre);
    Some(mapa)
}

pub fn borrar_esquema(paquete: &str, nombre: &str) {
    let _ = std::fs::remove_file(carpeta_esquemas(paquete).join(format!("{}.json", archivo_esquema(nombre))));
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
        // Las fuentes de egui no traen las flechas (↑↓←→): se usan las de Phosphor.
        "Key_Up" => egui_phosphor::regular::ARROW_UP.into(),
        "Key_Down" => egui_phosphor::regular::ARROW_DOWN.into(),
        "Key_Left" => egui_phosphor::regular::ARROW_LEFT.into(),
        "Key_Right" => egui_phosphor::regular::ARROW_RIGHT.into(),
        "" => "?".into(),
        otro => otro.strip_prefix("Key_").unwrap_or(otro).to_string(),
    }
}

// ---------------------------------------------------------------------------------------------
// Motor: teclado y mouse -> toques
// ---------------------------------------------------------------------------------------------

const ID_APUNTAR: u64 = 2;
const ID_DISPARO: u64 = 3;
const ID_MIRADA: u64 = 5;
const ID_NODO: u64 = 100;
/// Bit de `tweaks`: la posición se calcula desde el ancla con el total acumulado y la curva en X.
const MODO_ABSOLUTO: u32 = 0x4000;
/// Alto de la imagen de BlueStacks maximizado en una pantalla de 1920×1080. BlueStacks mide el
/// movimiento del mouse en píxeles de su ventana y lo pasa a porcentaje de la pantalla de Android:
/// con este tamaño fijo la cámara va como allá, sin depender de la resolución de Android ni del
/// tamaño de la ventana de ZYKRO (en BlueStacks va más lenta cuanto más grande es la ventana).
const ALTO_BLUESTACKS: f64 = 1000.0;
/// Con el cursor a esta distancia del personaje (en altos de pantalla), la habilidad MOBA apunta al
/// máximo de su radio.
const ALCANCE_HABILIDAD: f32 = 0.35;

/// Los dedos de cada control: el control `i` usa `ID_NODO + i·32 + k`.
fn dedo(i: usize, k: u64) -> u64 {
    ID_NODO + i as u64 * 32 + k
}

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

/// Un D-pad con dedo: dónde está y hacia dónde va.
struct MovDpad {
    actual: (f32, f32),
    objetivo: (f32, f32),
    /// Hasta cuándo espera (tiempo de activación).
    desde: Instant,
    /// Todavía no llegó la primera vez (usa la velocidad de activación).
    primero: bool,
}

/// La mirada libre en curso: su control (`None` = la de apuntar) y el dedo.
struct Mirada {
    nodo: Option<usize>,
    ancla: (f32, f32),
    punto: (f32, f32),
    /// Llegó a la orilla: el dedo vuelve al ojo a esta hora.
    vuelve: Option<Instant>,
}

struct MovDeslizar {
    actual: (f32, f32),
    objetivo: (f32, f32),
}

struct MovZoom {
    nodo: usize,
    /// Media distancia entre los dos dedos (píxeles).
    separacion: f32,
    /// +1 acercar, −1 alejar, 0 quieto (con las teclas).
    direccion: f32,
    /// Hacia dónde va con la rueda.
    objetivo: Option<f32>,
    desde: Instant,
    soltar: Option<Instant>,
}

struct MovMoba {
    nodo: usize,
    /// Cuándo se levanta el dedo (`None` mientras el clic derecho siga presionado).
    hasta: Option<Instant>,
}

struct MovRotar {
    angulo: f32,
    direccion: f32,
    desde: Instant,
}

#[derive(Default)]
struct MovBorde {
    actual: Option<(f32, f32)>,
    en_orilla: Option<Instant>,
    espera: Option<Instant>,
}

/// Una parte de un script corriendo.
struct Ejecucion {
    nodo: usize,
    comandos: Vec<script::Comando>,
    pc: usize,
    espera: Instant,
    /// Inicio y vueltas que faltan de cada `loop` abierto (`None` = hasta soltar la tecla).
    bucles: Vec<(usize, Option<u32>)>,
}

pub struct Motor {
    pub mapa: Option<Mapa>,
    pub paquete: String,
    /// Modo puntería: el mouse mueve la cámara con el cursor bloqueado.
    pub apuntando: bool,
    pausa: bool,
    presionadas: HashMap<String, bool>,
    apunte: Option<Apunte>,
    /// Posición del dedo en el botón de disparo mientras el clic izquierdo está presionado.
    disparo: Option<Vec2>,
    activos: HashMap<u64, (i32, i32)>,
    pendientes: Vec<Pendiente>,
    /// Canal de control y tamaño de pantalla, para el hilo que lee el mouse directo.
    salida: Option<(Control, f32, f32)>,
    dpads: HashMap<usize, MovDpad>,
    /// Próximo toque de los «toques repetidos» que siguen mientras la tecla esté presionada.
    repetidos: HashMap<usize, Instant>,
    mirada: Option<Mirada>,
    scripts: Vec<Ejecucion>,
    /// Dedos que tiene puestos cada script con `touch` (y si tiene el del mouse).
    dedos_script: HashMap<usize, usize>,
    deslizar: HashMap<usize, MovDeslizar>,
    zoom: Option<MovZoom>,
    moba: Option<MovMoba>,
    /// Habilidad MOBA apuntando (rápida o manual).
    habilidad: Option<usize>,
    rotar: HashMap<usize, MovRotar>,
    borde: MovBorde,
    /// Dónde está el cursor libre, en píxeles de Android.
    puntero: Option<(f32, f32)>,
    ultimo_tick: Instant,
}

impl Motor {
    pub fn new() -> Motor {
        Motor {
            mapa: None,
            paquete: String::new(),
            apuntando: false,
            pausa: false,
            presionadas: HashMap::new(),
            apunte: None,
            disparo: None,
            activos: HashMap::new(),
            pendientes: Vec::new(),
            salida: None,
            dpads: HashMap::new(),
            repetidos: HashMap::new(),
            mirada: None,
            scripts: Vec::new(),
            dedos_script: HashMap::new(),
            deslizar: HashMap::new(),
            zoom: None,
            moba: None,
            habilidad: None,
            rotar: HashMap::new(),
            borde: MovBorde::default(),
            puntero: None,
            ultimo_tick: Instant::now(),
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

    /// El cursor tiene que estar bloqueado: puntería activa y sin pausa, o mirada libre con el mouse.
    pub fn cursor_bloqueado(&self) -> bool {
        let mirando = self.mirada.as_ref().is_some_and(|m| !self.mirada_con_teclado(m));
        mirando || (self.apuntando && !self.pausa && self.mapa.as_ref().is_some_and(|m| m.raton.is_some()))
    }

    fn mirada_con_teclado(&self, m: &Mirada) -> bool {
        let nodo = m.nodo.and_then(|i| self.mapa.as_ref()?.nodos.get(i));
        matches!(nodo, Some(Nodo::MiradaLibre(n)) if n.teclado)
    }

    /// Una habilidad MOBA manual está apuntando: el próximo clic izquierdo es para lanzarla.
    pub fn captura_clic(&self) -> bool {
        let modo = self.habilidad.and_then(|i| match self.mapa.as_ref()?.nodos.get(i) {
            Some(Nodo::MobaHabilidad(h)) => Some(h.modo),
            _ => None,
        });
        modo == Some(2)
    }

    /// El punto de mira a dibujar, si se está apuntando y lo tiene activado.
    pub fn mira(&self) -> Option<Mira> {
        let raton = self.mapa.as_ref()?.raton.as_ref()?;
        (self.apuntando && !self.pausa && raton.mira.enabled).then(|| raton.mira.clone())
    }

    fn tocar(&mut self, ctl: &Control, accion: u8, dedo: u64, x: i32, y: i32, w: f32, h: f32) {
        ctl.toque(accion, dedo, x, y, w as u16, h as u16);
        if accion == SOLTAR {
            self.activos.remove(&dedo);
        } else {
            self.activos.insert(dedo, (x, y));
        }
    }

    fn tocar_f(&mut self, ctl: &Control, accion: u8, dedo: u64, p: (f32, f32), w: f32, h: f32) {
        self.tocar(ctl, accion, dedo, p.0 as i32, p.1 as i32, w, h);
    }

    /// Levanta un dedo si está puesto.
    fn levantar(&mut self, ctl: &Control, dedo: u64, w: f32, h: f32) {
        if let Some((x, y)) = self.activos.get(&dedo).copied() {
            self.tocar(ctl, SOLTAR, dedo, x, y, w, h);
        }
    }

    /// Un toque corto (baja ya y sube en 30 ms).
    fn toque_corto(&mut self, ctl: &Control, dedo: u64, p: (f32, f32), w: f32, h: f32) {
        self.tocar_f(ctl, BAJAR, dedo, p, w, h);
        self.programar(Instant::now() + Duration::from_millis(30), SOLTAR, dedo, p.0 as i32, p.1 as i32);
    }

    /// Suelta todos los dedos (al apagar el mapeo, abrir el editor o perder el foco).
    pub fn soltar_todo(&mut self, ctl: &Control, w: f32, h: f32) {
        for (dedo, (x, y)) in std::mem::take(&mut self.activos) {
            ctl.toque(SOLTAR, dedo, x, y, w as u16, h as u16);
        }
        self.presionadas.clear();
        self.apunte = None;
        self.disparo = None;
        self.pendientes.clear();
        self.pausa = false;
        self.dpads.clear();
        self.repetidos.clear();
        self.mirada = None;
        self.scripts.clear();
        self.dedos_script.clear();
        self.deslizar.clear();
        self.zoom = None;
        self.moba = None;
        self.habilidad = None;
        self.rotar.clear();
        self.borde = MovBorde::default();
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
        let pf = |p: Punto| (p.x * w, p.y * h);
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
            // Mirar alrededor dentro de la puntería: la mira se queda y el mouse mueve el ojo.
            if let (Some(tecla), Some(pos)) = (&r.mirada_tecla, r.mirada_pos) {
                if tecla == nombre {
                    if presionada {
                        self.empezar_mirada(None, pf(pos), ctl, w, h);
                    } else if self.mirada.as_ref().is_some_and(|m| m.nodo.is_none()) {
                        self.terminar_mirada(ctl, w, h);
                    }
                    usada = true;
                }
            }
        }
        let ctrl = self.presionadas.contains_key("Key_Control");

        for (i, nodo) in mapa.nodos.iter().enumerate() {
            match nodo {
                Nodo::Clic(c, doble) if c.key == nombre => {
                    usada = true;
                    let (x, y) = px(c.pos);
                    if presionada {
                        self.tocar(ctl, BAJAR, dedo(i, 0), x, y, w, h);
                        if *doble {
                            self.tocar(ctl, SOLTAR, dedo(i, 0), x, y, w, h);
                            let t = Instant::now() + Duration::from_millis(70);
                            self.programar(t, BAJAR, dedo(i, 0), x, y);
                            self.programar(t + Duration::from_millis(40), SOLTAR, dedo(i, 0), x, y);
                        }
                        if c.cambia_modo {
                            self.alternar_punteria(ctl, w, h);
                        }
                    } else if !*doble {
                        self.tocar(ctl, SOLTAR, dedo(i, 0), x, y, w, h);
                    }
                }
                Nodo::Multi(m) if m.key == nombre => {
                    usada = true;
                    if presionada {
                        let mut t = Instant::now();
                        for paso in &m.pasos {
                            t += Duration::from_millis(paso.delay);
                            let (x, y) = px(paso.pos);
                            self.programar(t, BAJAR, dedo(i, 0), x, y);
                            self.programar(t + Duration::from_millis(30), SOLTAR, dedo(i, 0), x, y);
                        }
                    }
                }
                Nodo::Dpad(d) if [&d.arriba, &d.abajo, &d.izquierda, &d.derecha, &d.tecla1, &d.tecla2].iter().any(|k| !k.is_empty() && k.as_str() == nombre) => {
                    usada = true;
                    self.actualizar_dpad(i, d, ctl, w, h);
                }
                Nodo::Arrastre(a) if a.key == nombre => {
                    usada = true;
                    if presionada {
                        let (x0, y0) = px(a.inicio);
                        let (x1, y1) = px(a.fin);
                        self.tocar(ctl, BAJAR, dedo(i, 0), x0, y0, w, h);
                        let pasos = 10;
                        let total = (180.0 / a.velocidad.max(0.1)) as u64;
                        for k in 1..=pasos {
                            let f = k as f32 / pasos as f32;
                            let x = x0 + ((x1 - x0) as f32 * f) as i32;
                            let y = y0 + ((y1 - y0) as f32 * f) as i32;
                            let t = Instant::now() + Duration::from_millis(total * k / pasos);
                            self.programar(t, if k == pasos { SOLTAR } else { MOVER }, dedo(i, 0), x, y);
                        }
                    }
                }
                Nodo::Repetido(n) if n.key == nombre => {
                    usada = true;
                    let intervalo = Duration::from_secs_f32(1.0 / n.por_segundo.clamp(1.0, 30.0));
                    if presionada {
                        self.toque_corto(ctl, dedo(i, 0), pf(n.pos), w, h);
                        if n.hasta_soltar {
                            self.repetidos.insert(i, Instant::now() + intervalo);
                        } else {
                            let (x, y) = px(n.pos);
                            for k in 1..n.veces.max(1) {
                                let t = Instant::now() + intervalo * k;
                                self.programar(t, BAJAR, dedo(i, 0), x, y);
                                self.programar(t + Duration::from_millis(30), SOLTAR, dedo(i, 0), x, y);
                            }
                        }
                    } else {
                        self.repetidos.remove(&i);
                    }
                }
                Nodo::MiradaLibre(m) if m.key == nombre => {
                    usada = true;
                    if presionada {
                        self.empezar_mirada(Some(i), pf(m.pos), ctl, w, h);
                    } else if self.mirada.as_ref().is_some_and(|x| x.nodo == Some(i)) {
                        self.terminar_mirada(ctl, w, h);
                    }
                }
                Nodo::MiradaLibre(m) if m.teclado && [&m.arriba, &m.abajo, &m.izquierda, &m.derecha].iter().any(|k| k.as_str() == nombre) => {
                    usada |= self.mirada.as_ref().is_some_and(|x| x.nodo == Some(i));
                }
                Nodo::Script(sn) if sn.key == nombre => {
                    usada = true;
                    let (s, _) = script::leer(&sn.texto);
                    if presionada {
                        self.correr(i, s.presionar);
                    } else {
                        self.correr(i, s.soltar);
                    }
                }
                Nodo::Deslizar(d) if [&d.arriba, &d.abajo, &d.izquierda, &d.derecha].iter().any(|k| !k.is_empty() && k.as_str() == nombre) => {
                    usada = true;
                    if presionada {
                        let dir = if nombre == d.arriba {
                            (0.0, -1.0)
                        } else if nombre == d.abajo {
                            (0.0, 1.0)
                        } else if nombre == d.izquierda {
                            (-1.0, 0.0)
                        } else {
                            (1.0, 0.0)
                        };
                        let inicio = pf(d.pos);
                        self.levantar(ctl, dedo(i, 0), w, h);
                        self.tocar_f(ctl, BAJAR, dedo(i, 0), inicio, w, h);
                        let objetivo = (inicio.0 + dir.0 * d.radio * h, inicio.1 + dir.1 * d.radio * h);
                        self.deslizar.insert(i, MovDeslizar { actual: inicio, objetivo });
                    } else if self.deslizar.remove(&i).is_some() || d.mantener {
                        self.levantar(ctl, dedo(i, 0), w, h);
                    }
                }
                Nodo::Zoom(z) => {
                    let mod_ok = z.modificador.is_empty() || self.presionadas.contains_key(z.modificador.as_str());
                    let acercar = nombre == z.acercar || (!z.exclusivo && ctrl && nombre == "Key_Equal");
                    let alejar = nombre == z.alejar || (!z.exclusivo && ctrl && nombre == "Key_Minus");
                    if (acercar || alejar) && mod_ok {
                        usada = true;
                        if presionada {
                            self.empezar_zoom(i, z, ctl, w, h);
                            if let Some(zm) = &mut self.zoom {
                                zm.direccion = if acercar { 1.0 } else { -1.0 };
                                zm.objetivo = None;
                                zm.desde = Instant::now();
                                zm.soltar = None;
                            }
                        } else if let Some(zm) = &mut self.zoom {
                            zm.direccion = 0.0;
                            zm.soltar = Some(Instant::now() + Duration::from_millis(z.espera_ms));
                        }
                    }
                }
                Nodo::MobaDpad(m) if nombre == "RightButton" => {
                    usada = true;
                    if presionada {
                        self.moba = Some(MovMoba { nodo: i, hasta: None });
                        self.mover_moba(i, m, ctl, w, h);
                    } else if let Some(mv) = &mut self.moba {
                        // Sigue andando lo que tardaría el personaje en llegar al punto del clic.
                        let (hx, hy) = pf(m.heroe);
                        let (cx, cy) = self.puntero.unwrap_or((hx, hy));
                        let segundos = ((cx - hx).hypot(cy - hy) / (m.velocidad_heroe.max(0.05) * h)).clamp(0.05, 5.0);
                        mv.hasta = Some(Instant::now() + Duration::from_secs_f32(segundos));
                    }
                }
                Nodo::MobaDpad(m) if !m.cancelar.is_empty() && nombre == m.cancelar => {
                    usada = true;
                    if presionada {
                        self.parar_moba(ctl, w, h);
                    }
                }
                Nodo::MobaHabilidad(hb) if hb.key == nombre => {
                    usada = true;
                    match (hb.modo, presionada) {
                        (1, true) => {
                            self.toque_corto(ctl, dedo(i, 0), pf(hb.pos), w, h);
                            if hb.detener_dpad {
                                self.parar_moba(ctl, w, h);
                            }
                        }
                        (0 | 2, true) => {
                            self.levantar(ctl, dedo(i, 0), w, h);
                            self.tocar_f(ctl, BAJAR, dedo(i, 0), pf(hb.pos), w, h);
                            self.habilidad = Some(i);
                            self.apuntar_habilidad(i, hb, ctl, w, h);
                        }
                        // Rápida: se lanza al soltar la tecla.
                        (0, false) if self.habilidad == Some(i) => self.lanzar(i, hb.detener_dpad, ctl, w, h),
                        _ => {}
                    }
                }
                Nodo::MobaHabilidad(hb) if !hb.cancelar.is_empty() && nombre == hb.cancelar && self.habilidad == Some(i) => {
                    usada = true;
                    if presionada {
                        // Se arrastra la habilidad al botón de cancelar del juego antes de soltarla.
                        if let Some(c) = hb.cancelar_pos {
                            self.tocar_f(ctl, MOVER, dedo(i, 0), pf(c), w, h);
                        }
                        self.levantar(ctl, dedo(i, 0), w, h);
                        self.habilidad = None;
                    }
                }
                Nodo::Rotar(r) if nombre == r.horario || nombre == r.antihorario => {
                    usada = true;
                    if presionada {
                        let direccion = if nombre == r.horario { 1.0 } else { -1.0 };
                        let angulo = self.rotar.get(&i).map_or(r.angulo, |m| m.angulo);
                        let p = punto_circulo(pf(r.pos), r.radio * h, angulo);
                        if !self.activos.contains_key(&dedo(i, 0)) {
                            self.tocar_f(ctl, BAJAR, dedo(i, 0), p, w, h);
                        }
                        self.rotar.insert(i, MovRotar { angulo, direccion, desde: Instant::now() + Duration::from_millis(r.activacion_ms) });
                    } else if self.rotar.remove(&i).is_some() {
                        self.levantar(ctl, dedo(i, 0), w, h);
                    }
                }
                _ => {}
            }
        }
        usada
    }

    /// Botones del mouse. Con la puntería activa, el clic izquierdo dispara; con una habilidad MOBA
    /// manual apuntando, la lanza.
    pub fn boton(&mut self, boton: PointerButton, presionado: bool, ctl: &Control, w: f32, h: f32) -> bool {
        let nombre = nombre_boton(boton);
        if boton == PointerButton::Primary && self.cursor_bloqueado() && self.mirada.is_none() {
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
        if boton == PointerButton::Primary && self.captura_clic() {
            if presionado {
                if let Some(i) = self.habilidad {
                    let detener = matches!(self.mapa.as_ref().and_then(|m| m.nodos.get(i)), Some(Nodo::MobaHabilidad(hb)) if hb.detener_dpad);
                    self.lanzar(i, detener, ctl, w, h);
                }
            }
            return true;
        }
        self.tecla(nombre, presionado, false, ctl, w, h)
    }

    /// Rueda del mouse (`pasos` > 0 hacia arriba). Devuelve si un control la usó (si no, desplaza
    /// donde está el cursor, como siempre).
    pub fn rueda(&mut self, pasos: f32, ctl: &Control, w: f32, h: f32) -> bool {
        let Some(mapa) = self.mapa.clone() else { return false };
        if pasos == 0.0 {
            return false;
        }
        for (i, nodo) in mapa.nodos.iter().enumerate() {
            match nodo {
                Nodo::Rueda(r) => {
                    let p = if pasos > 0.0 { r.arriba } else { r.abajo };
                    self.levantar(ctl, dedo(i, 0), w, h);
                    self.toque_corto(ctl, dedo(i, 0), (p.x * w, p.y * h), w, h);
                    return true;
                }
                Nodo::Desplazar(d) => {
                    // Rueda hacia arriba = el contenido baja = el dedo baja.
                    let inicio = (d.pos.x * w, d.pos.y * h);
                    let fin = (inicio.0, inicio.1 + pasos.signum() * d.amplitud * h);
                    let ms = (d.amplitud / d.velocidad.max(0.05) * 1000.0).clamp(30.0, 2000.0) as u64;
                    self.levantar(ctl, dedo(i, 0), w, h);
                    self.gesto(ctl, dedo(i, 0), &[inicio, fin], ms, w, h);
                    if d.exclusivo {
                        return true;
                    }
                }
                Nodo::Zoom(z) => {
                    if !z.modificador.is_empty() && !self.presionadas.contains_key(z.modificador.as_str()) {
                        continue;
                    }
                    self.empezar_zoom(i, z, ctl, w, h);
                    if let Some(zm) = &mut self.zoom {
                        let base = zm.objetivo.unwrap_or(zm.separacion);
                        zm.objetivo = Some((base + pasos.signum() * z.amplitud * h).clamp(0.02 * h, 0.45 * h));
                        zm.direccion = 0.0;
                        zm.soltar = None;
                    }
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Hay controles que usan la rueda del mouse.
    pub fn usa_rueda(&self) -> bool {
        self.mapa.as_ref().is_some_and(|m| m.nodos.iter().any(|n| matches!(n, Nodo::Rueda(_) | Nodo::Desplazar(_) | Nodo::Zoom(_))))
    }

    /// La rueda es solo para el mapeo (no desplaza donde está el cursor).
    pub fn rueda_exclusiva(&self) -> bool {
        self.mapa.as_ref().is_some_and(|m| {
            m.nodos.iter().any(|n| matches!(n, Nodo::Rueda(_) | Nodo::Zoom(_)) || matches!(n, Nodo::Desplazar(d) if d.exclusivo))
        })
    }

    /// Dónde está el cursor libre (en píxeles de Android), para los controles MOBA, el
    /// desplazamiento de borde y los scripts con `onMouseMove`.
    pub fn puntero(&mut self, pos: Option<(f32, f32)>, ctl: &Control, w: f32, h: f32) {
        let cambio = pos.is_some() && pos != self.puntero;
        self.puntero = pos;
        if cambio {
            self.mouse_movido(ctl, w, h);
        }
    }

    /// Con la tecla de un script con `onMouseMove` presionada, mover el mouse corre esa parte.
    fn mouse_movido(&mut self, _ctl: &Control, _w: f32, _h: f32) {
        let Some(mapa) = self.mapa.clone() else { return };
        for (i, nodo) in mapa.nodos.iter().enumerate() {
            if let Nodo::Script(sn) = nodo {
                if self.presionadas.contains_key(sn.key.as_str()) && !self.scripts.iter().any(|e| e.nodo == i && e.pc < e.comandos.len()) {
                    let (s, _) = script::leer(&sn.texto);
                    if !s.mover.is_empty() {
                        self.correr(i, s.mover);
                    }
                }
            }
        }
    }

    /// Movimiento del mouse con el cursor bloqueado: mueve la mirada libre si hay una, o si no la
    /// cámara, con la fórmula de BlueStacks (`delta` en cuentas crudas; detalles en
    /// docs/investigacion.md, «Cámara de BlueStacks»):
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
        if delta == Vec2::ZERO {
            return;
        }
        if let Some(m) = &self.mirada {
            if !self.mirada_con_teclado(m) {
                let sens = match m.nodo.and_then(|i| self.mapa.as_ref()?.nodos.get(i)) {
                    Some(Nodo::MiradaLibre(n)) => n.velocidad,
                    _ => self.mapa.as_ref().and_then(|m| m.raton.as_ref()).map_or(1.0, |r| r.sens),
                };
                let escala = sens * h / ALTO_BLUESTACKS as f32;
                self.mover_mirada((delta.x * escala, delta.y * escala), ctl, w, h);
                self.mouse_movido(ctl, w, h);
                return;
            }
        }
        let Some(r) = self.mapa.as_ref().and_then(|m| m.raton.as_ref()) else { return };
        if !self.cursor_bloqueado() {
            return;
        }
        // Aceleración: los movimientos rápidos (más cuentas por reporte) giran más.
        let delta = if r.aceleracion { delta * (1.0 + delta.length().min(40.0) * 0.04) } else { delta };
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

    /// Temporizador: toques programados y todo lo que se mueve solo (D-pad con velocidad, zoom,
    /// rotar, scripts…).
    pub fn tick(&mut self, ctl: &Control, w: f32, h: f32) {
        let ahora = Instant::now();
        let dt = (ahora - self.ultimo_tick).as_secs_f32().min(0.05);
        self.ultimo_tick = ahora;
        if !self.ocupado() {
            return;
        }
        let (listos, faltan): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pendientes).into_iter().partition(|p| p.cuando <= ahora);
        self.pendientes = faltan;
        for p in listos {
            self.tocar(ctl, p.accion, p.dedo, p.x, p.y, w, h);
        }
        let Some(mapa) = self.mapa.clone() else { return };
        let nodo = |i: usize| mapa.nodos.get(i);

        // D-pad: el dedo se desliza hacia donde apuntan las teclas.
        for i in self.dpads.keys().copied().collect::<Vec<_>>() {
            let Some(Nodo::Dpad(d)) = nodo(i) else { continue };
            let Some(m) = self.dpads.get_mut(&i) else { continue };
            if ahora < m.desde || m.actual == m.objetivo {
                continue;
            }
            let velocidad = if m.primero && d.velocidad_inicial > 0.0 { d.velocidad_inicial } else { d.velocidad };
            m.actual = acercar(m.actual, m.objetivo, if velocidad > 0.0 { velocidad * h * dt } else { f32::INFINITY });
            if m.actual == m.objetivo {
                m.primero = false;
            }
            let p = m.actual;
            self.tocar_f(ctl, MOVER, dedo(i, 0), p, w, h);
        }

        // Toques repetidos mientras la tecla siga presionada.
        for (i, proximo) in self.repetidos.clone() {
            let Some(Nodo::Repetido(n)) = nodo(i) else { continue };
            if ahora >= proximo {
                self.toque_corto(ctl, dedo(i, 0), (n.pos.x * w, n.pos.y * h), w, h);
                self.repetidos.insert(i, proximo + Duration::from_secs_f32(1.0 / n.por_segundo.clamp(1.0, 30.0)));
            }
        }

        // Mirada libre: vuelve al ojo tras llegar a la orilla; con teclado, se mueve con las teclas.
        if let Some(m) = &mut self.mirada {
            if m.vuelve.is_some_and(|t| ahora >= t) {
                m.vuelve = None;
                m.punto = m.ancla;
                let p = m.ancla;
                self.tocar_f(ctl, BAJAR, ID_MIRADA, p, w, h);
            }
        }
        if let Some(Some(Nodo::MiradaLibre(n))) = self.mirada.as_ref().and_then(|m| m.nodo).map(nodo) {
            if n.teclado {
                let p = |k: &str| self.presionadas.contains_key(k) as i32 as f32;
                let dir = (p(&n.derecha) - p(&n.izquierda), p(&n.abajo) - p(&n.arriba));
                if dir != (0.0, 0.0) {
                    let paso = n.velocidad * h * dt;
                    self.mover_mirada((dir.0 * paso, dir.1 * paso), ctl, w, h);
                }
            }
        }

        self.avanzar_scripts(ctl, w, h);

        // Deslizar en cuatro direcciones.
        for i in self.deslizar.keys().copied().collect::<Vec<_>>() {
            let Some(Nodo::Deslizar(d)) = nodo(i) else { continue };
            let Some(m) = self.deslizar.get_mut(&i) else { continue };
            if m.actual == m.objetivo {
                continue;
            }
            m.actual = acercar(m.actual, m.objetivo, if d.velocidad > 0.0 { d.velocidad * h * dt } else { f32::INFINITY });
            let (p, llego) = (m.actual, m.actual == m.objetivo);
            self.tocar_f(ctl, MOVER, dedo(i, 0), p, w, h);
            // Sin «mantener», el dedo se levanta al terminar el deslizamiento.
            if llego && !d.mantener {
                self.deslizar.remove(&i);
                self.levantar(ctl, dedo(i, 0), w, h);
            }
        }

        // Zoom: los dos dedos se separan o se juntan.
        if let Some(zm) = &mut self.zoom {
            if let Some(Nodo::Zoom(z)) = nodo(zm.nodo) {
                let rapidez = z.velocidad.max(0.05) * h * dt;
                if zm.direccion != 0.0 {
                    // Mientras se mantiene la tecla, acelera hasta `aceleracion` en un segundo.
                    let t = (ahora - zm.desde).as_secs_f32().min(1.0);
                    let factor = 1.0 + (z.aceleracion.max(1.0) - 1.0) * t;
                    zm.separacion = (zm.separacion + zm.direccion * rapidez * factor).clamp(0.02 * h, 0.45 * h);
                } else if let Some(objetivo) = zm.objetivo {
                    zm.separacion += (objetivo - zm.separacion).clamp(-rapidez, rapidez);
                    if zm.separacion == objetivo {
                        zm.objetivo = None;
                        zm.soltar = Some(ahora + Duration::from_millis(z.espera_ms));
                    }
                }
                let (i, sep, modo, soltar) = (zm.nodo, zm.separacion, z.modo, zm.soltar);
                let c = (z.pos.x * w, z.pos.y * h);
                // Modo 1: un dedo hace casi todo el recorrido, como una mano.
                let (a, b) = if modo == 1 { (sep * 1.7, sep * 0.3) } else { (sep, sep) };
                self.tocar_f(ctl, MOVER, dedo(i, 0), (c.0 - a, c.1), w, h);
                self.tocar_f(ctl, MOVER, dedo(i, 1), (c.0 + b, c.1), w, h);
                if soltar.is_some_and(|t| ahora >= t) {
                    self.levantar(ctl, dedo(i, 0), w, h);
                    self.levantar(ctl, dedo(i, 1), w, h);
                    self.zoom = None;
                }
            }
        }

        // D-pad MOBA: sigue al cursor mientras el clic derecho esté presionado.
        if let Some(mv) = &self.moba {
            let (i, hasta) = (mv.nodo, mv.hasta);
            if let Some(Nodo::MobaDpad(m)) = nodo(i) {
                match hasta {
                    None => self.mover_moba(i, m, ctl, w, h),
                    Some(t) if ahora >= t => self.parar_moba(ctl, w, h),
                    _ => {}
                }
            }
        }

        // Habilidad MOBA apuntando con el cursor.
        if let Some(i) = self.habilidad {
            if let Some(Nodo::MobaHabilidad(hb)) = nodo(i) {
                self.apuntar_habilidad(i, hb, ctl, w, h);
            }
        }

        // Rotar.
        for i in self.rotar.keys().copied().collect::<Vec<_>>() {
            let Some(Nodo::Rotar(r)) = nodo(i) else { continue };
            let Some(m) = self.rotar.get_mut(&i) else { continue };
            if ahora < m.desde {
                continue;
            }
            m.angulo += m.direccion * r.velocidad * dt;
            let p = punto_circulo((r.pos.x * w, r.pos.y * h), r.radio * h, m.angulo);
            self.tocar_f(ctl, MOVER, dedo(i, 0), p, w, h);
        }

        self.desplazar_borde(&mapa, dt, ctl, w, h);
    }

    /// Desplazamiento de borde: con el cursor en una orilla, un dedo arrastra el mapa hacia allá.
    fn desplazar_borde(&mut self, mapa: &Mapa, dt: f32, ctl: &Control, w: f32, h: f32) {
        let control = mapa.nodos.iter().enumerate().find_map(|(i, n)| match n {
            Nodo::Borde(b) if b.enabled => Some((i, b.clone())),
            _ => None,
        });
        let Some((i, b)) = control else { return };
        let ahora = Instant::now();
        let direccion = match self.puntero.filter(|_| !self.cursor_bloqueado()) {
            Some((x, y)) => {
                let lado = |v: f32, total: f32, margen: f32| {
                    if v < margen * total {
                        -1.0
                    } else if v > total - margen * total {
                        1.0
                    } else {
                        0.0
                    }
                };
                (lado(x, w, b.margen_x), lado(y, h, b.margen_y))
            }
            None => (0.0, 0.0),
        };
        if direccion == (0.0, 0.0) {
            if self.borde.actual.take().is_some() {
                self.levantar(ctl, dedo(i, 0), w, h);
            }
            self.borde.en_orilla = None;
            return;
        }
        if self.borde.espera.is_some_and(|t| ahora < t) {
            return;
        }
        let inicio = (b.pos.x * w, b.pos.y * h);
        let desde = *self.borde.en_orilla.get_or_insert(ahora);
        let actual = match self.borde.actual {
            Some(p) => p,
            None => {
                self.tocar_f(ctl, BAJAR, dedo(i, 0), inicio, w, h);
                inicio
            }
        };
        let factor = if ahora - desde > Duration::from_millis(b.acelerar_ms) { b.factor.max(1.0) } else { 1.0 };
        // Para ver lo que hay a la derecha, el mapa se arrastra hacia la izquierda.
        let nuevo = (actual.0 - direccion.0 * b.vel_x * h * dt * factor, actual.1 - direccion.1 * b.vel_y * h * dt * factor);
        self.tocar_f(ctl, MOVER, dedo(i, 0), nuevo, w, h);
        self.borde.actual = Some(nuevo);
        // Tras un buen trecho, el dedo se levanta y vuelve a empezar desde `pos`.
        if (nuevo.0 - inicio.0).hypot(nuevo.1 - inicio.1) > 0.3 * h {
            self.levantar(ctl, dedo(i, 0), w, h);
            self.borde.actual = None;
            self.borde.espera = Some(ahora + Duration::from_millis(b.espera_ms));
        }
    }

    /// Hay algo pendiente que necesita que la interfaz siga refrescando.
    pub fn ocupado(&self) -> bool {
        let borde = self.mapa.as_ref().is_some_and(|m| m.nodos.iter().any(|n| matches!(n, Nodo::Borde(b) if b.enabled))) && self.puntero.is_some();
        !self.pendientes.is_empty()
            || self.dpads.values().any(|m| m.actual != m.objetivo)
            || !self.repetidos.is_empty()
            || self.mirada.is_some()
            || !self.scripts.is_empty()
            || !self.deslizar.is_empty()
            || self.zoom.is_some()
            || self.moba.is_some()
            || self.habilidad.is_some()
            || !self.rotar.is_empty()
            || borde
            // Si el cursor sale de la imagen con el dedo de borde abajo, el tick lo tiene que levantar.
            || self.borde.en_orilla.is_some()
    }

    fn programar(&mut self, cuando: Instant, accion: u8, dedo: u64, x: i32, y: i32) {
        self.pendientes.push(Pendiente { cuando, accion, dedo, x, y });
    }

    /// Un gesto programado por varios puntos: baja en el primero, pasa por todos y sube en `ms`.
    fn gesto(&mut self, ctl: &Control, dedo: u64, puntos: &[(f32, f32)], ms: u64, w: f32, h: f32) {
        let Some(&primero) = puntos.first() else { return };
        self.tocar_f(ctl, BAJAR, dedo, primero, w, h);
        let pasos = ((ms / 16).max(1) as usize).max(puntos.len());
        let inicio = Instant::now();
        let tramos = puntos.len().saturating_sub(1).max(1);
        for k in 1..=pasos {
            let f = k as f32 / pasos as f32 * tramos as f32;
            let tramo = (f.floor() as usize).min(tramos - 1);
            let resto = f - tramo as f32;
            let (a, b) = (puntos[tramo.min(puntos.len() - 1)], puntos[(tramo + 1).min(puntos.len() - 1)]);
            let p = (a.0 + (b.0 - a.0) * resto, a.1 + (b.1 - a.1) * resto);
            let t = inicio + Duration::from_millis(ms * k as u64 / pasos as u64);
            self.programar(t, MOVER, dedo, p.0 as i32, p.1 as i32);
            if k == pasos {
                self.programar(t, SOLTAR, dedo, p.0 as i32, p.1 as i32);
            }
        }
    }

    /// Suelta la puntería (el mouse vuelve a ser un cursor), si estaba activa.
    pub fn soltar_punteria(&mut self, ctl: &Control, w: f32, h: f32) {
        if self.apuntando {
            self.alternar_punteria(ctl, w, h);
        }
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

    /// Recalcula hacia dónde va el D-pad según sus teclas (y el modificador de velocidad).
    fn actualizar_dpad(&mut self, i: usize, d: &Dpad, ctl: &Control, w: f32, h: f32) {
        let p = |k: &str| !k.is_empty() && self.presionadas.contains_key(k);
        let dx = p(&d.derecha) as i32 as f32 - p(&d.izquierda) as i32 as f32;
        let dy = p(&d.abajo) as i32 as f32 - p(&d.arriba) as i32 as f32;
        let centro = (d.centro.x * w, d.centro.y * h);
        if dx == 0.0 && dy == 0.0 {
            if self.dpads.remove(&i).is_some() {
                self.levantar(ctl, dedo(i, 0), w, h);
            }
            return;
        }
        let (rx, ry) = if p(&d.tecla1) && d.radio1 > 0.0 {
            (d.radio1, d.radio1)
        } else if p(&d.tecla2) && d.radio2 > 0.0 {
            (d.radio2, d.radio2)
        } else {
            (if dx > 0.0 { d.d_derecha } else { d.d_izquierda }, if dy > 0.0 { d.d_abajo } else { d.d_arriba })
        };
        // Como en QtScrcpy: desplazamiento horizontal relativo al ancho y vertical al alto.
        let objetivo = (centro.0 + dx * rx * w, centro.1 + dy * ry * h);
        let ahora = Instant::now();
        let m = self.dpads.entry(i).or_insert_with(|| {
            ctl.toque(BAJAR, dedo(i, 0), centro.0 as i32, centro.1 as i32, w as u16, h as u16);
            // La zona muerta se salta de golpe; desde ahí desliza.
            let largo = dx.hypot(dy);
            let salto = (centro.0 + dx / largo * d.zona_muerta * h, centro.1 + dy / largo * d.zona_muerta * h);
            MovDpad { actual: if d.zona_muerta > 0.0 { salto } else { centro }, objetivo, desde: ahora + Duration::from_millis(d.activacion_ms), primero: true }
        });
        m.objetivo = objetivo;
        self.activos.insert(dedo(i, 0), (m.actual.0 as i32, m.actual.1 as i32));
        // Sin velocidad ni activación, el dedo salta ya, como en QtScrcpy.
        if d.velocidad <= 0.0 && d.velocidad_inicial <= 0.0 && d.activacion_ms == 0 {
            m.actual = objetivo;
            m.primero = false;
            self.tocar_f(ctl, MOVER, dedo(i, 0), objetivo, w, h);
        }
    }

    fn empezar_mirada(&mut self, nodo: Option<usize>, ancla: (f32, f32), ctl: &Control, w: f32, h: f32) {
        if self.mirada.is_some() {
            self.terminar_mirada(ctl, w, h);
        }
        self.tocar_f(ctl, BAJAR, ID_MIRADA, ancla, w, h);
        self.mirada = Some(Mirada { nodo, ancla, punto: ancla, vuelve: None });
    }

    fn terminar_mirada(&mut self, ctl: &Control, w: f32, h: f32) {
        if self.mirada.take().is_some() {
            self.levantar(ctl, ID_MIRADA, w, h);
        }
    }

    fn mover_mirada(&mut self, d: (f32, f32), ctl: &Control, w: f32, h: f32) {
        let espera = match self.mirada.as_ref().and_then(|m| m.nodo).and_then(|i| self.mapa.as_ref()?.nodos.get(i)) {
            Some(Nodo::MiradaLibre(n)) => n.espera_orilla_ms,
            _ => 300,
        };
        let Some(m) = &mut self.mirada else { return };
        if m.vuelve.is_some() {
            return;
        }
        m.punto = (m.punto.0 + d.0, m.punto.1 + d.1);
        let p = m.punto;
        if p.0 < 0.0 || p.1 < 0.0 || p.0 > w || p.1 > h {
            // En la orilla: se levanta y vuelve al ojo un momento después, como BlueStacks.
            m.vuelve = Some(Instant::now() + Duration::from_millis(espera));
            self.levantar(ctl, ID_MIRADA, w, h);
        } else {
            self.tocar_f(ctl, MOVER, ID_MIRADA, p, w, h);
        }
    }

    /// Pone (o mueve) el dedo del D-pad MOBA hacia el cursor, desde el personaje.
    fn mover_moba(&mut self, i: usize, m: &MobaDpad, ctl: &Control, w: f32, h: f32) {
        let heroe = (m.heroe.x * w, m.heroe.y * h);
        let cursor = self.puntero.unwrap_or(heroe);
        let (dx, dy) = (cursor.0 - heroe.0, cursor.1 - heroe.1);
        let largo = dx.hypot(dy);
        let centro = (m.pos.x * w, m.pos.y * h);
        if !self.activos.contains_key(&dedo(i, 0)) {
            self.tocar_f(ctl, BAJAR, dedo(i, 0), centro, w, h);
        }
        if largo > 1.0 {
            let r = m.radio * h;
            self.tocar_f(ctl, MOVER, dedo(i, 0), (centro.0 + dx / largo * r, centro.1 + dy / largo * r), w, h);
        }
    }

    fn parar_moba(&mut self, ctl: &Control, w: f32, h: f32) {
        if let Some(mv) = self.moba.take() {
            self.levantar(ctl, dedo(mv.nodo, 0), w, h);
        }
    }

    /// La habilidad apunta hacia el cursor, desde el personaje (el del D-pad MOBA, o el centro).
    fn apuntar_habilidad(&mut self, i: usize, hb: &MobaHabilidad, ctl: &Control, w: f32, h: f32) {
        let heroe = self
            .mapa
            .as_ref()
            .and_then(|m| m.nodos.iter().find_map(|n| if let Nodo::MobaDpad(d) = n { Some((d.heroe.x * w, d.heroe.y * h)) } else { None }))
            .unwrap_or((w / 2.0, h / 2.0));
        let cursor = self.puntero.unwrap_or(heroe);
        let (dx, dy) = (cursor.0 - heroe.0, cursor.1 - heroe.1);
        let largo = dx.hypot(dy);
        let centro = (hb.pos.x * w, hb.pos.y * h);
        let destino = if largo > 1.0 {
            let alcance = (largo / (ALCANCE_HABILIDAD * h)).min(1.0) * hb.radio * h;
            (centro.0 + dx / largo * alcance, centro.1 + dy / largo * alcance)
        } else {
            centro
        };
        self.tocar_f(ctl, MOVER, dedo(i, 0), destino, w, h);
    }

    fn lanzar(&mut self, i: usize, detener_dpad: bool, ctl: &Control, w: f32, h: f32) {
        self.levantar(ctl, dedo(i, 0), w, h);
        self.habilidad = None;
        if detener_dpad {
            self.parar_moba(ctl, w, h);
        }
    }

    fn empezar_zoom(&mut self, i: usize, z: &Zoom, ctl: &Control, w: f32, h: f32) {
        if self.zoom.as_ref().is_some_and(|zm| zm.nodo == i) {
            return;
        }
        let separacion = 0.1 * h;
        let c = (z.pos.x * w, z.pos.y * h);
        self.tocar_f(ctl, BAJAR, dedo(i, 0), (c.0 - separacion, c.1), w, h);
        self.tocar_f(ctl, BAJAR, dedo(i, 1), (c.0 + separacion, c.1), w, h);
        self.zoom = Some(MovZoom { nodo: i, separacion, direccion: 0.0, objetivo: None, desde: Instant::now(), soltar: None });
    }

    // -----------------------------------------------------------------------------------------
    // Scripts
    // -----------------------------------------------------------------------------------------

    fn correr(&mut self, nodo: usize, comandos: Vec<script::Comando>) {
        if !comandos.is_empty() {
            self.scripts.push(Ejecucion { nodo, comandos, pc: 0, espera: Instant::now(), bucles: Vec::new() });
        }
    }

    fn avanzar_scripts(&mut self, ctl: &Control, w: f32, h: f32) {
        let mut k = 0;
        while k < self.scripts.len() {
            // Un tope por cuadro, por si un `loop` no tiene `wait`.
            let mut presupuesto = 200;
            let mut terminado = false;
            while presupuesto > 0 {
                presupuesto -= 1;
                let e = &mut self.scripts[k];
                if Instant::now() < e.espera {
                    break;
                }
                let Some(comando) = e.comandos.get(e.pc).cloned() else {
                    terminado = true;
                    break;
                };
                e.pc += 1;
                let nodo = e.nodo;
                if !self.ejecutar(k, nodo, comando, ctl, w, h) {
                    terminado = true;
                    break;
                }
                if k >= self.scripts.len() || self.scripts[k].nodo != nodo {
                    // `abort` quitó esta ejecución.
                    terminado = false;
                    break;
                }
            }
            if terminado && k < self.scripts.len() {
                self.scripts.remove(k);
            } else {
                k += 1;
            }
        }
    }

    /// Ejecuta un comando del script `k` (del control `nodo`). Devuelve `false` si el script terminó.
    fn ejecutar(&mut self, k: usize, nodo: usize, comando: script::Comando, ctl: &Control, w: f32, h: f32) -> bool {
        use script::Comando as C;
        let pct = |x: f32, y: f32| (x / 100.0 * w, y / 100.0 * h);
        let ahora = Instant::now();
        match comando {
            C::Esperar(ms) => self.scripts[k].espera = ahora + Duration::from_millis(ms),
            C::Tocar { puntos, ms } => {
                for (j, (x, y)) in puntos.iter().enumerate().take(16) {
                    let p = pct(*x, *y);
                    self.tocar_f(ctl, BAJAR, dedo(nodo, 20 + j as u64 % 8), p, w, h);
                    self.programar(ahora + Duration::from_millis(ms), SOLTAR, dedo(nodo, 20 + j as u64 % 8), p.0 as i32, p.1 as i32);
                }
                self.scripts[k].espera = ahora + Duration::from_millis(ms + 10);
            }
            C::Deslizar { puntos, ms } => {
                let puntos: Vec<(f32, f32)> = puntos.iter().map(|(x, y)| pct(*x, *y)).collect();
                self.gesto(ctl, dedo(nodo, 28), &puntos, ms, w, h);
                self.scripts[k].espera = ahora + Duration::from_millis(ms + 10);
            }
            C::Dedos(puntos) => {
                let puntos: Vec<(f32, f32)> = puntos.iter().map(|(x, y)| pct(*x, *y)).collect();
                self.poner_dedos(nodo, &puntos, ctl, w, h);
            }
            C::DedosMouse(valores) => {
                let (mx, my) = self.puntero.map_or((50.0, 50.0), |(x, y)| (x / w * 100.0, y / h * 100.0));
                let valor = |v: script::Valor, base: f32| match v {
                    script::Valor::Fijo(n) => n,
                    script::Valor::Mouse(d) => base + d,
                };
                let puntos: Vec<(f32, f32)> = valores.iter().map(|(x, y)| pct(valor(*x, mx), valor(*y, my))).collect();
                self.poner_dedos(nodo, &puntos, ctl, w, h);
            }
            C::MouseAbajo(x, y) => self.tocar_f(ctl, BAJAR, dedo(nodo, 17), pct(x, y), w, h),
            C::MouseMover(x, y) => {
                if self.activos.contains_key(&dedo(nodo, 17)) {
                    self.tocar_f(ctl, MOVER, dedo(nodo, 17), pct(x, y), w, h);
                }
            }
            C::MouseArriba => self.levantar(ctl, dedo(nodo, 17), w, h),
            C::Tecla { nombre, presionada, directo } => {
                if directo {
                    if let Some(codigo) = codigo_android(&nombre) {
                        ctl.tecla(if presionada { BAJAR } else { SOLTAR }, codigo, 0);
                    }
                } else if !self.tecla(&nombre, presionada, false, ctl, w, h) {
                    // Sin mapeo para esa tecla, llega a Android.
                    if let Some(codigo) = codigo_android(&nombre) {
                        ctl.tecla(if presionada { BAJAR } else { SOLTAR }, codigo, 0);
                    }
                }
            }
            C::Texto(texto) => {
                let texto = texto.replace('\'', "").replace(' ', "%s");
                std::thread::spawn(move || crate::android::shell(&format!("input text '{texto}'")));
            }
            C::Borrar(veces) => {
                for _ in 0..veces.min(200) {
                    ctl.pulsar(67); // KEYCODE_DEL
                }
            }
            C::Rueda(_, _, d) => {
                self.rueda(d, ctl, w, h);
            }
            C::Bucle(veces) => {
                let pc = self.scripts[k].pc;
                self.scripts[k].bucles.push((pc, veces));
            }
            C::FinBucle => {
                let tecla_presionada = match self.mapa.as_ref().and_then(|m| m.nodos.get(nodo)) {
                    Some(Nodo::Script(sn)) => self.presionadas.contains_key(sn.key.as_str()),
                    _ => false,
                };
                let e = &mut self.scripts[k];
                match e.bucles.last_mut() {
                    Some((inicio, Some(veces))) if *veces > 1 => {
                        *veces -= 1;
                        e.pc = *inicio;
                    }
                    Some((inicio, None)) if tecla_presionada => e.pc = *inicio,
                    Some(_) => {
                        e.bucles.pop();
                    }
                    None => {}
                }
            }
            C::Abortar => {
                self.scripts.retain(|e| e.nodo != nodo);
                for j in 0..32 {
                    self.levantar(ctl, dedo(nodo, j), w, h);
                }
                self.dedos_script.remove(&nodo);
                return true;
            }
            C::Disparo(modo) => {
                if self.mapa.as_ref().is_some_and(|m| m.raton.is_some()) {
                    let quiero = modo.unwrap_or(!self.apuntando);
                    if quiero != self.apuntando {
                        self.alternar_punteria(ctl, w, h);
                    }
                }
            }
        }
        true
    }

    /// `touch`: deja puestos exactamente esos dedos (los que sobran se levantan).
    fn poner_dedos(&mut self, nodo: usize, puntos: &[(f32, f32)], ctl: &Control, w: f32, h: f32) {
        let antes = self.dedos_script.get(&nodo).copied().unwrap_or(0);
        for (j, p) in puntos.iter().enumerate().take(16) {
            let accion = if j < antes { MOVER } else { BAJAR };
            self.tocar_f(ctl, accion, dedo(nodo, j as u64), *p, w, h);
        }
        for j in puntos.len().min(16)..antes {
            self.levantar(ctl, dedo(nodo, j as u64), w, h);
        }
        self.dedos_script.insert(nodo, puntos.len().min(16));
    }
}

/// Mueve `de` hacia `a` como mucho `paso` (llega exacto).
fn acercar(de: (f32, f32), a: (f32, f32), paso: f32) -> (f32, f32) {
    let (dx, dy) = (a.0 - de.0, a.1 - de.1);
    let largo = dx.hypot(dy);
    if largo <= paso || largo == 0.0 {
        a
    } else {
        (de.0 + dx / largo * paso, de.1 + dy / largo * paso)
    }
}

/// Un punto del círculo (grados: 0 = a la derecha, 90 = abajo).
fn punto_circulo(centro: (f32, f32), radio: f32, grados: f32) -> (f32, f32) {
    let a = grados.to_radians();
    (centro.0 + a.cos() * radio, centro.1 + a.sin() * radio)
}

/// Código de Android de una tecla del mapeo (para los scripts).
fn codigo_android(nombre: &str) -> Option<u32> {
    egui::Key::ALL.iter().find(|k| nombre_tecla(**k).as_deref() == Some(nombre)).and_then(|k| crate::android::tecla_android(*k))
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
        let ctl = Control::conectar(ruta.to_str().unwrap()).unwrap();
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
        let toques: Vec<(u8, i32, i32)> = datos
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

    /// Corre `pasos` con un control falso y devuelve los toques que habría mandado: (acción, x, y).
    fn toques(mapa: Mapa, pasos: impl FnOnce(&mut Motor, &Control)) -> Vec<(u8, i32, i32)> {
        use std::io::Read;
        use std::os::unix::net::UnixListener;
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let ruta = std::env::temp_dir().join(format!("zykro-prueba-{}-{n}.sock", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        let servidor = UnixListener::bind(&ruta).unwrap();
        let ctl = Control::conectar(ruta.to_str().unwrap()).unwrap();
        let (mut socket, _) = servidor.accept().unwrap();
        let _ = std::fs::remove_file(&ruta);
        let mut motor = Motor::new();
        motor.mapa = Some(mapa);
        pasos(&mut motor, &ctl);
        drop(ctl);
        let mut datos = Vec::new();
        socket.read_to_end(&mut datos).unwrap();
        datos
            .chunks_exact(32)
            .filter(|m| m[0] == 0x02)
            .map(|m| {
                let i32_en = |i: usize| i32::from_be_bytes(m[i..i + 4].try_into().unwrap());
                (m[1], i32_en(10), i32_en(14))
            })
            .collect()
    }

    /// Hace correr el temporizador durante `ms`.
    fn esperar(motor: &mut Motor, ctl: &Control, ms: u64) {
        let fin = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < fin {
            motor.tick(ctl, 1280.0, 720.0);
            std::thread::sleep(Duration::from_millis(4));
        }
    }

    fn con(nodo: Nodo) -> Mapa {
        Mapa { nodos: vec![nodo], ..Mapa::default() }
    }

    const W: f32 = 1280.0;
    const H: f32 = 720.0;

    #[test]
    fn tick_inactivo_actualiza_reloj_y_respeta_toques_pendientes() {
        let eventos = toques(Mapa::default(), |motor, ctl| {
            motor.ultimo_tick = Instant::now() - Duration::from_secs(1);
            let antes = Instant::now();
            motor.tick(ctl, W, H);
            assert!(motor.ultimo_tick >= antes);
            assert!(!motor.ocupado());
            motor.programar(Instant::now(), BAJAR, 0, 12, 34);
            motor.tick(ctl, W, H);
        });
        assert_eq!(eventos, vec![(BAJAR, 12, 34)]);
    }

    #[test]
    fn borde_levanta_el_dedo_si_el_cursor_sale_de_la_imagen() {
        let borde: Borde = serde_json::from_value(serde_json::json!({ "pos": { "x": 0.5, "y": 0.5 } })).unwrap();
        let eventos = toques(con(Nodo::Borde(borde)), |motor, ctl| {
            motor.puntero(Some((1.0, H / 2.0)), ctl, W, H);
            motor.tick(ctl, W, H);
            motor.puntero(None, ctl, W, H);
            motor.tick(ctl, W, H);
            assert!(!motor.ocupado());
        });
        assert_eq!(eventos.first().map(|e| e.0), Some(BAJAR));
        assert_eq!(eventos.last().map(|e| e.0), Some(SOLTAR));
    }

    #[test]
    #[ignore]
    fn medir_rutas_de_entrada() {
        let mapa = Mapa::desde_json(serde_json::from_str(include_str!("../../config/keymap/com.dts.freefireth.json")).unwrap()).unwrap();
        for _ in 0..5 {
            let eventos = toques(mapa.clone(), |motor, ctl| {
                let inicio = Instant::now();
                for _ in 0..100_000 {
                    motor.tick(std::hint::black_box(ctl), W, H);
                }
                eprintln!("[medicion] 100000 ticks inactivos: {:?}", inicio.elapsed());
                motor.apuntando = true;
                let inicio = Instant::now();
                for _ in 0..100_000 {
                    motor.mover(std::hint::black_box(Vec2::new(1.0, -1.0)), ctl, W, H);
                }
                eprintln!("[medicion] 100000 movimientos: {:?}", inicio.elapsed());
            });
            assert_eq!(eventos.len(), 100_001);
        }
    }

    #[test]
    fn toques_repetidos() {
        let n = Repetido { key: "F".into(), pos: Punto::nuevo(0.5, 0.5), veces: 3, hasta_soltar: false, por_segundo: 20.0, extra: Map::new() };
        let t = toques(con(Nodo::Repetido(n)), |m, c| {
            m.tecla("F", true, false, c, W, H);
            m.tecla("F", false, false, c, W, H);
            esperar(m, c, 250);
        });
        assert_eq!(t.iter().filter(|x| x.0 == BAJAR).count(), 3);
        assert_eq!(t.iter().filter(|x| x.0 == SOLTAR).count(), 3);
        assert!(t.iter().all(|x| (x.1, x.2) == (640, 360)));
    }

    #[test]
    fn script_toca_espera_y_desliza() {
        let n = ScriptNodo { key: "G".into(), pos: Punto::nuevo(0.5, 0.5), texto: "tap 50 50 20\nloop 2\ntap 10 10\nloopEnd\nswipe 10 10 20 20 60".into(), extra: Map::new() };
        let t = toques(con(Nodo::Script(n)), |m, c| {
            m.tecla("G", true, false, c, W, H);
            esperar(m, c, 400);
        });
        let bajadas: Vec<(i32, i32)> = t.iter().filter(|x| x.0 == BAJAR).map(|x| (x.1, x.2)).collect();
        assert_eq!(bajadas, vec![(640, 360), (128, 72), (128, 72), (128, 72)]);
        assert_eq!(t.last().map(|x| (x.0, x.1, x.2)), Some((SOLTAR, 256, 144)));
    }

    #[test]
    fn rueda_del_mouse_toca_arriba_y_abajo() {
        let n = RuedaNodo { arriba: Punto::nuevo(0.5, 0.1), abajo: Punto::nuevo(0.5, 0.9), extra: Map::new() };
        let t = toques(con(Nodo::Rueda(n)), |m, c| {
            assert!(m.rueda(1.0, c, W, H));
            esperar(m, c, 60);
            assert!(m.rueda(-1.0, c, W, H));
            esperar(m, c, 60);
        });
        assert_eq!(t, vec![(BAJAR, 640, 72), (SOLTAR, 640, 72), (BAJAR, 640, 648), (SOLTAR, 640, 648)]);
    }

    #[test]
    fn dpad_con_modificador_de_velocidad() {
        let d = Dpad {
            centro: Punto::nuevo(0.2, 0.7),
            arriba: "W".into(),
            abajo: "S".into(),
            izquierda: "A".into(),
            derecha: "D".into(),
            d_arriba: 0.04,
            d_abajo: 0.04,
            d_izquierda: 0.04,
            d_derecha: 0.04,
            tecla1: "Key_Shift".into(),
            radio1: 0.08,
            tecla2: String::new(),
            radio2: 0.0,
            velocidad: 0.0,
            activacion_ms: 0,
            velocidad_inicial: 0.0,
            zona_muerta: 0.0,
            extra: Map::new(),
        };
        let t = toques(con(Nodo::Dpad(d)), |m, c| {
            m.tecla("W", true, false, c, W, H);
            m.tecla("Key_Shift", true, false, c, W, H);
            m.tecla("W", false, false, c, W, H);
        });
        // Al centro, arriba con el radio normal, arriba con el de correr y soltar.
        assert_eq!(t, vec![(BAJAR, 256, 504), (MOVER, 256, 475), (MOVER, 256, 446), (SOLTAR, 256, 446)]);
    }

    #[test]
    fn zoom_con_la_rueda() {
        let z = Zoom {
            pos: Punto::nuevo(0.5, 0.5),
            acercar: "Key_Equal".into(),
            alejar: "Key_Minus".into(),
            modificador: String::new(),
            amplitud: 0.05,
            aceleracion: 1.0,
            velocidad: 2.0,
            modo: 0,
            espera_ms: 30,
            exclusivo: false,
            extra: Map::new(),
        };
        let t = toques(con(Nodo::Zoom(z)), |m, c| {
            assert!(m.rueda(1.0, c, W, H));
            esperar(m, c, 200);
        });
        // Dos dedos a 72 px del centro que se separan a 108 (72 + 0.05·720) y se levantan.
        assert_eq!(&t[..2], &[(BAJAR, 568, 360), (BAJAR, 712, 360)]);
        assert!(t.contains(&(MOVER, 532, 360)) && t.contains(&(MOVER, 748, 360)));
        assert_eq!(t.iter().filter(|x| x.0 == SOLTAR).count(), 2);
    }

    #[test]
    fn dpad_moba_va_hacia_el_cursor() {
        let n = MobaDpad { pos: Punto::nuevo(0.15, 0.75), radio: 0.1, heroe: Punto::nuevo(0.5, 0.5), velocidad_heroe: 2.0, cancelar: String::new(), extra: Map::new() };
        let t = toques(con(Nodo::MobaDpad(n)), |m, c| {
            m.puntero(Some((1000.0, 360.0)), c, W, H);
            m.boton(PointerButton::Secondary, true, c, W, H);
            m.boton(PointerButton::Secondary, false, c, W, H);
            esperar(m, c, 400);
        });
        // El joystick (192, 540) hacia la derecha con radio 72; se suelta solo al llegar.
        assert_eq!(&t[..2], &[(BAJAR, 192, 540), (MOVER, 264, 540)]);
        assert_eq!(t.last().map(|x| x.0), Some(SOLTAR));
    }

    #[test]
    fn habilidad_rapida_apunta_y_lanza_al_soltar() {
        let hb = MobaHabilidad {
            key: "Q".into(),
            pos: Punto::nuevo(0.8, 0.8),
            radio: 0.1,
            modo: 0,
            detener_dpad: false,
            cancelar: String::new(),
            cancelar_pos: None,
            extra: Map::new(),
        };
        let t = toques(con(Nodo::MobaHabilidad(hb)), |m, c| {
            // Cursor arriba del centro (sin D-pad MOBA, el personaje es el centro), a más del alcance.
            m.puntero(Some((640.0, 0.0)), c, W, H);
            m.tecla("Q", true, false, c, W, H);
            m.tecla("Q", false, false, c, W, H);
        });
        assert_eq!(t, vec![(BAJAR, 1024, 576), (MOVER, 1024, 504), (SOLTAR, 1024, 504)]);
    }

    #[test]
    fn rotar_gira_alrededor() {
        let r = Rotar { pos: Punto::nuevo(0.5, 0.5), horario: "E".into(), antihorario: "Q".into(), radio: 0.1, angulo: 0.0, velocidad: 900.0, activacion_ms: 0, extra: Map::new() };
        let t = toques(con(Nodo::Rotar(r)), |m, c| {
            m.tecla("E", true, false, c, W, H);
            esperar(m, c, 100);
            m.tecla("E", false, false, c, W, H);
        });
        assert_eq!(t[0], (BAJAR, 712, 360));
        // A 900°/s, en 100 ms da casi un cuarto de vuelta: pasa por abajo (y mayor que el centro).
        assert!(t.iter().any(|x| x.0 == MOVER && x.2 > 420));
        assert_eq!(t.last().map(|x| x.0), Some(SOLTAR));
    }

    /// Lee el mapeo de Free Fire (el de ZYKRO, o el de SwagPlayer) y comprueba que al volver a guardarlo no se pierde
    /// nada: `cargo test ida_y_vuelta -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn ida_y_vuelta() {
        let ruta = ["zykro", "swagplayer"].iter().map(|app| carpeta(app).join("com.dts.freefireth.json")).find(|r| r.exists()).unwrap();
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
                _ => "nuevo",
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
