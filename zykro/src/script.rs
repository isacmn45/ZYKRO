//! Scripts de los controles (el control «Script» de BlueStacks): una secuencia de toques, teclas y
//! esperas que corre al presionar su tecla. Mismos comandos que BlueStacks; las coordenadas van en
//! porcentaje de la pantalla (0 a 100), así el script sirve en cualquier resolución.
//!
//! ```text
//! # comentario                       wait 50
//! tap 44.5 55.2 [espera]             swipe 4 5 40 45 [duración]
//! touch x1 y1 x2 y2 …                mouseDown x y · mouseMove x y · mouseUp
//! keyDown TAB · keyUp TAB            keyDown2 · keyUp2 (sin pasar por el mapeo, directo a Android)
//! text "hola" · text backspace 2     mouseWheel x y d
//! loop [n] … loopEnd                 abort
//! enterShoot · exitShoot · toggleShoot
//! onMouseMove (con dTouch x+a y+b …) onRelease
//! ```

/// Un número del script, o (en `dTouch`) un desplazamiento desde el mouse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Valor {
    Fijo(f32),
    Mouse(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Comando {
    Esperar(u64),
    Tocar { puntos: Vec<(f32, f32)>, ms: u64 },
    Deslizar { puntos: Vec<(f32, f32)>, ms: u64 },
    Dedos(Vec<(f32, f32)>),
    DedosMouse(Vec<(Valor, Valor)>),
    MouseAbajo(f32, f32),
    MouseMover(f32, f32),
    MouseArriba,
    /// Tecla (con `directo`, sin pasar por el mapeo).
    Tecla { nombre: String, presionada: bool, directo: bool },
    Texto(String),
    Borrar(u32),
    Rueda(f32, f32, f32),
    Bucle(Option<u32>),
    FinBucle,
    Abortar,
    Disparo(Option<bool>),
}

/// Un script ya leído: lo que corre al presionar la tecla, al mover el mouse con ella presionada y
/// al soltarla.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Script {
    pub presionar: Vec<Comando>,
    pub mover: Vec<Comando>,
    pub soltar: Vec<Comando>,
}

/// Lee un script. Devuelve también las líneas que no entendió (número de línea y texto).
pub fn leer(texto: &str) -> (Script, Vec<(usize, String)>) {
    let mut script = Script::default();
    let mut errores = Vec::new();
    #[derive(PartialEq)]
    enum Parte {
        Presionar,
        Mover,
        Soltar,
    }
    let mut parte = Parte::Presionar;
    for (n, linea) in texto.lines().enumerate() {
        let linea = linea.split('#').next().unwrap_or("").trim();
        if linea.is_empty() {
            continue;
        }
        let (palabra, resto) = linea.split_once(char::is_whitespace).unwrap_or((linea, ""));
        let resto = resto.trim();
        let comando = match palabra.to_ascii_lowercase().as_str() {
            "onmousemove" => {
                parte = Parte::Mover;
                continue;
            }
            "onrelease" => {
                parte = Parte::Soltar;
                continue;
            }
            otra => comando(otra, resto),
        };
        match comando {
            Some(c) => match parte {
                Parte::Presionar => script.presionar.push(c),
                Parte::Mover => script.mover.push(c),
                Parte::Soltar => script.soltar.push(c),
            },
            None => errores.push((n + 1, linea.to_string())),
        }
    }
    (script, errores)
}

fn numeros(texto: &str) -> Option<Vec<f32>> {
    texto.split_whitespace().map(|n| n.parse::<f32>().ok()).collect()
}

/// Pares de coordenadas y, si sobra un número, la duración (o `defecto`).
fn puntos_y_ms(texto: &str, defecto: u64) -> Option<(Vec<(f32, f32)>, u64)> {
    let mut n = numeros(texto)?;
    let ms = if n.len() % 2 == 1 { n.pop()? as u64 } else { defecto };
    Some((n.chunks(2).map(|p| (p[0], p[1])).collect(), ms))
}

fn comando(palabra: &str, resto: &str) -> Option<Comando> {
    Some(match palabra {
        "wait" => Comando::Esperar(resto.parse::<f32>().ok()?.max(0.0) as u64),
        "tap" => {
            let (puntos, ms) = puntos_y_ms(resto, 50)?;
            if puntos.is_empty() {
                return None;
            }
            Comando::Tocar { puntos, ms }
        }
        "swipe" => {
            let (puntos, ms) = puntos_y_ms(resto, 100)?;
            if puntos.len() < 2 {
                return None;
            }
            Comando::Deslizar { puntos, ms }
        }
        "touch" => {
            let n = numeros(resto)?;
            if n.len() % 2 == 1 {
                return None;
            }
            Comando::Dedos(n.chunks(2).map(|p| (p[0], p[1])).collect())
        }
        "dtouch" => {
            let valores: Vec<Valor> = resto.split_whitespace().map(valor).collect::<Option<_>>()?;
            if valores.len() % 2 == 1 {
                return None;
            }
            Comando::DedosMouse(valores.chunks(2).map(|p| (p[0], p[1])).collect())
        }
        "mousedown" => {
            let n = numeros(resto)?;
            Comando::MouseAbajo(*n.first()?, *n.get(1)?)
        }
        "mousemove" => {
            let n = numeros(resto)?;
            Comando::MouseMover(*n.first()?, *n.get(1)?)
        }
        "mouseup" => Comando::MouseArriba,
        "keydown" | "keyup" | "keydown2" | "keyup2" if !resto.is_empty() => Comando::Tecla {
            nombre: tecla(resto),
            presionada: palabra.starts_with("keydown"),
            directo: palabra.ends_with('2'),
        },
        "text" => {
            if let Some(n) = resto.strip_prefix("backspace") {
                Comando::Borrar(n.trim().parse().unwrap_or(1))
            } else {
                let t = resto.trim_matches(|c| c == '"' || c == '“' || c == '”');
                Comando::Texto(t.to_string())
            }
        }
        "mousewheel" => {
            let n = numeros(resto)?;
            Comando::Rueda(*n.first()?, *n.get(1)?, *n.get(2)?)
        }
        "loop" => Comando::Bucle(resto.parse().ok()),
        "loopend" => Comando::FinBucle,
        "abort" => Comando::Abortar,
        "entershoot" => Comando::Disparo(Some(true)),
        "exitshoot" => Comando::Disparo(Some(false)),
        "toggleshoot" => Comando::Disparo(None),
        _ => return None,
    })
}

/// `x+3`, `y-2.5`, `x` o un número.
fn valor(texto: &str) -> Option<Valor> {
    let t = texto.to_ascii_lowercase();
    if let Some(resto) = t.strip_prefix('x').or_else(|| t.strip_prefix('y')) {
        if resto.is_empty() {
            return Some(Valor::Mouse(0.0));
        }
        return resto.parse::<f32>().ok().map(Valor::Mouse);
    }
    t.parse().ok().map(Valor::Fijo)
}

/// Nombre de tecla del script (como en BlueStacks: TAB, A, SPACE, F1…) al formato del mapeo (Key_Tab).
pub fn tecla(nombre: &str) -> String {
    let n = nombre.trim();
    if n.starts_with("Key_") || n.ends_with("Button") {
        return n.to_string();
    }
    let mayus = n.to_ascii_uppercase();
    if mayus.len() == 1 && mayus.chars().all(|c| c.is_ascii_alphanumeric()) {
        return mayus;
    }
    match mayus.as_str() {
        "TAB" => "Key_Tab",
        "SPACE" | "SPACEBAR" => "Key_Space",
        "ENTER" | "RETURN" => "Key_Return",
        "ESC" | "ESCAPE" => "Key_Escape",
        "BACKSPACE" | "BACK" => "Key_Backspace",
        "SHIFT" | "LSHIFT" | "RSHIFT" => "Key_Shift",
        "CTRL" | "CONTROL" | "LCTRL" | "RCTRL" => "Key_Control",
        "ALT" | "LALT" | "RALT" => "Key_Alt",
        "UP" => "Key_Up",
        "DOWN" => "Key_Down",
        "LEFT" => "Key_Left",
        "RIGHT" => "Key_Right",
        "CAPSLOCK" | "CAPITAL" => "Key_CapsLock",
        "DELETE" | "DEL" => "Key_Delete",
        "HOME" => "Key_Home",
        "END" => "Key_End",
        "PAGEUP" => "Key_PageUp",
        "PAGEDOWN" => "Key_PageDown",
        "LBUTTON" => "LeftButton",
        "RBUTTON" => "RightButton",
        "MBUTTON" => "MiddleButton",
        f if f.starts_with('F') && f[1..].parse::<u8>().is_ok() => return format!("Key_{f}"),
        _ => return format!("Key_{}", &mayus[..1].to_string().to_ascii_uppercase()) + &mayus[1..].to_ascii_lowercase(),
    }
    .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_los_comandos_de_bluestacks() {
        let (s, errores) = leer(
            "# abrir la mochila\ntap 10 20\ntap 10 20 30 40 120\nwait 24\nswipe 4 5 40 45 100\nkeyDown TAB\nkeyup2 a\n\
             text \"hola mundo\"\ntext backspace 2\nloop 3\nmouseWheel 44 55.5 -1\nloopEnd\nexitShoot\n\
             onMouseMove\ndTouch x+3 y x-3 y\nonRelease\nabort\nnoexiste 1",
        );
        assert_eq!(errores, vec![(18, "noexiste 1".to_string())]);
        assert_eq!(s.presionar[0], Comando::Tocar { puntos: vec![(10.0, 20.0)], ms: 50 });
        assert_eq!(s.presionar[1], Comando::Tocar { puntos: vec![(10.0, 20.0), (30.0, 40.0)], ms: 120 });
        assert_eq!(s.presionar[2], Comando::Esperar(24));
        assert_eq!(s.presionar[3], Comando::Deslizar { puntos: vec![(4.0, 5.0), (40.0, 45.0)], ms: 100 });
        assert_eq!(s.presionar[4], Comando::Tecla { nombre: "Key_Tab".into(), presionada: true, directo: false });
        assert_eq!(s.presionar[5], Comando::Tecla { nombre: "A".into(), presionada: false, directo: true });
        assert_eq!(s.presionar[6], Comando::Texto("hola mundo".into()));
        assert_eq!(s.presionar[7], Comando::Borrar(2));
        assert_eq!(s.presionar[8], Comando::Bucle(Some(3)));
        assert_eq!(s.presionar[9], Comando::Rueda(44.0, 55.5, -1.0));
        assert_eq!(s.presionar[11], Comando::Disparo(Some(false)));
        assert_eq!(s.mover, vec![Comando::DedosMouse(vec![(Valor::Mouse(3.0), Valor::Mouse(0.0)), (Valor::Mouse(-3.0), Valor::Mouse(0.0))])]);
        assert_eq!(s.soltar, vec![Comando::Abortar]);
    }
}
