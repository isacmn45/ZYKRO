//! Scroll con la rueda del mouse y el touchpad. Android no aplica el scroll de mouse que inyecta
//! swag-rust-server (mensaje 0x03, ver docs/investigacion.md), así que se imita como en una pantalla
//! táctil: un dedo que se baja donde está el cursor y se arrastra.

use std::time::{Duration, Instant};

use crate::control::{Control, BAJAR, MOVER, SOLTAR};

const DEDO: u64 = 5;
/// Píxeles de Android que arrastra cada paso de la rueda.
pub const PASO_RUEDA: f32 = 100.0;
/// Sin movimiento durante este tiempo se suelta el dedo. Como queda quieto antes de soltarlo,
/// Android no "lanza" la página.
const SOLTAR_TRAS: Duration = Duration::from_millis(120);

#[derive(Default)]
pub struct Rueda {
    /// Posición del dedo (píxeles de Android) mientras está abajo.
    dedo: Option<(f32, f32)>,
    /// Movimiento acumulado que todavía no alcanza para bajar el dedo.
    pendiente: (f32, f32),
    ultimo: Option<Instant>,
}

impl Rueda {
    /// Arrastra `delta` píxeles de Android desde `cursor`. El dedo solo baja cuando lo acumulado
    /// supera `umbral` (el margen de Android para distinguir un arrastre de un toque): así un scroll
    /// mínimo no se convierte en un toque que abra lo que haya bajo el cursor.
    pub fn mover(&mut self, ctl: &Control, cursor: (f32, f32), delta: (f32, f32), umbral: f32, w: f32, h: f32) {
        let (wu, hu) = (w as u16, h as u16);
        self.ultimo = Some(Instant::now());
        if let Some(pos) = self.dedo {
            let nuevo = (pos.0 + delta.0, pos.1 + delta.1);
            if nuevo.0 >= 0.0 && nuevo.1 >= 0.0 && nuevo.0 < w && nuevo.1 < h {
                ctl.toque(MOVER, DEDO, nuevo.0 as i32, nuevo.1 as i32, wu, hu);
                self.dedo = Some(nuevo);
                return;
            }
            // Se acabó la pantalla: soltar y seguir desde el cursor.
            ctl.toque(SOLTAR, DEDO, pos.0 as i32, pos.1 as i32, wu, hu);
            self.dedo = None;
        }
        self.pendiente = (self.pendiente.0 + delta.0, self.pendiente.1 + delta.1);
        if self.pendiente.0.abs().max(self.pendiente.1.abs()) < umbral {
            return;
        }
        let destino = ((cursor.0 + self.pendiente.0).clamp(0.0, w - 1.0), (cursor.1 + self.pendiente.1).clamp(0.0, h - 1.0));
        ctl.toque(BAJAR, DEDO, cursor.0 as i32, cursor.1 as i32, wu, hu);
        ctl.toque(MOVER, DEDO, destino.0 as i32, destino.1 as i32, wu, hu);
        self.dedo = Some(destino);
        self.pendiente = (0.0, 0.0);
    }

    /// Suelta el dedo si ya no se está moviendo. Devuelve cuánto falta para revisar otra vez.
    pub fn revisar(&mut self, ctl: &Control, w: f32, h: f32) -> Option<Duration> {
        let falta = SOLTAR_TRAS.saturating_sub(self.ultimo?.elapsed());
        if !falta.is_zero() {
            return Some(falta);
        }
        if let Some(pos) = self.dedo.take() {
            ctl.toque(SOLTAR, DEDO, pos.0 as i32, pos.1 as i32, w as u16, h as u16);
        }
        self.pendiente = (0.0, 0.0);
        self.ultimo = None;
        None
    }
}
