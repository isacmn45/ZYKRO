//! Curva que SwagPlayer aplica al eje X de la puntería: una tabla de 81 puntos sacada de su binario,
//! interpolada con un spline cúbico natural. Solo X pasa por ella; Y es lineal.
//! Detalles en docs/investigacion.md («La curva `S` de X»).

use std::sync::OnceLock;

/// Entrada: sensitivityX × cuentas del mouse desde que se bajó el dedo.
const X: [f64; 81] = [
    1.0, 5.192307692, 10.5885, 16.2857, 22.5, 49.09, 80.4807, 116.5909, 135.0, 270.0, 405.0, 540.0, 675.0, 810.0,
    945.0, 1080.0, 1215.0, 1350.0, 1485.0, 1620.0, 1755.0, 1890.0, 2025.0, 2160.0, 2295.0, 2430.0, 2565.0, 2700.0,
    2835.0, 2970.0, 3105.0, 3240.0, 6480.0, 9720.0, 12960.0, 16200.0, 19440.0, 22680.0, 25920.0, 29160.0, 32400.0,
    35640.0, 38880.0, 42120.0, 45360.0, 48600.0, 51840.0, 55080.0, 58320.0, 61560.0, 64800.0, 68040.0, 7128000.0,
    7452000.0, 7776000.0, 8100000.0, 8424000.0, 8748000.0, 9072000.0, 9396000.0, 9720000.0, 10044000.0,
    10368000.0, 10692000.0, 11016000.0, 11340000.0, 11664000.0, 11988000.0, 12312000.0, 12636000.0, 12960000.0,
    13284000.0, 13608000.0, 13932000.0, 14256000.0, 14580000.0, 14904000.0, 15228000.0, 15552000.0, 15876000.0,
    16200000.0,
];

/// Salida: píxeles de Android que se mueve el dedo desde el ancla.
const Y: [f64; 81] = [
    1.0, 5.0, 10.0, 15.0, 20.0, 40.0, 60.0, 80.0, 85.9, 145.9, 189.9, 226.9, 263.9, 295.9, 324.9, 351.9, 377.9,
    402.9, 425.9, 448.9, 469.9, 489.9, 509.9, 529.9, 547.9, 565.9, 583.9, 599.9, 616.9, 633.9, 648.9, 671.3, 987.3,
    1234.3, 1445.3, 1628.3, 1798.3, 1950.3, 2098.3, 2233.3, 2373.3, 2539.3, 2643.2, 2765.2, 2880.2, 2991.2, 3096.2,
    3197.2, 3297.2, 3391.2, 3487.2, 3577.2, 3662.2, 3750.2, 3836.2, 3921.2, 4009.2, 4096.2, 4174.2, 4255.2, 4339.2,
    4419.2, 4492.2, 4575.8, 4654.2, 4727.2, 4795.2, 4864.2, 4929.2, 4993.2, 5076.2, 5121.2, 5190.2, 5260.2, 5320.2,
    5392.2, 5454.2, 5515.2, 5573.2, 5633.2, 5691.2,
];

/// Tramos `[x, a, b, c, d]`: en cada uno, `y = a + b·t + c·t² + d·t³` con `t = entrada − x`.
/// Se calculan igual que SwagPlayer (spline natural); el último sigue con el polinomio del penúltimo.
fn tramos() -> &'static [[f64; 5]; 81] {
    static TRAMOS: OnceLock<[[f64; 5]; 81]> = OnceLock::new();
    TRAMOS.get_or_init(|| {
        let n = X.len() - 1;
        let h: Vec<f64> = (0..n).map(|i| X[i + 1] - X[i]).collect();
        let dy: Vec<f64> = (0..n).map(|i| Y[i + 1] - Y[i]).collect();
        let (mut mu, mut z) = ([0.0; 81], [0.0; 81]);
        for i in 1..n {
            let alfa = 3.0 * (dy[i] / h[i] - dy[i - 1] / h[i - 1]);
            let l = 2.0 * (X[i + 1] - X[i - 1]) - h[i - 1] * mu[i - 1];
            mu[i] = h[i] / l;
            z[i] = (alfa - h[i - 1] * z[i - 1]) / l;
        }
        let mut t = [[0.0; 5]; 81];
        let mut c_sig = 0.0;
        for j in (0..n).rev() {
            let c = z[j] - mu[j] * c_sig;
            let b = dy[j] / h[j] - h[j] * (c_sig + 2.0 * c) / 3.0;
            let d = (c_sig - c) / (3.0 * h[j]);
            t[j] = [X[j], Y[j], b, c, d];
            c_sig = c;
        }
        t[n] = [X[n], Y[n], t[n - 1][2], t[n - 1][3], t[n - 1][4]];
        t
    })
}

/// Píxeles que se mueve el dedo en X para `v` = sensitivityX × cuentas acumuladas. Conserva el signo.
pub fn horizontal(v: f64) -> f64 {
    if v == 0.0 || v.is_nan() {
        return 0.0;
    }
    let a = v.abs();
    let i = X.partition_point(|&x| x <= a).saturating_sub(1);
    let [x0, a0, b, c, d] = tramos()[i];
    let t = a - x0;
    (a0 + t * (b + t * (c + t * d))).copysign(v)
}

#[cfg(test)]
mod pruebas {
    use super::horizontal;

    #[test]
    fn pasa_por_la_tabla() {
        assert_eq!(horizontal(0.0), 0.0);
        for (x, y) in [(1.0, 1.0), (135.0, 85.9), (3240.0, 671.3), (68040.0, 3577.2)] {
            assert!((horizontal(x) - y).abs() < 1e-6, "S({x}) = {}", horizontal(x));
            assert!((horizontal(-x) + y).abs() < 1e-6);
        }
    }

    #[test]
    fn entre_puntos() {
        // Valores calculados con el mismo spline en Python a partir del binario.
        for (x, y) in [(10.0, 9.46), (100.0, 71.86), (1000.0, 336.09), (10000.0, 1254.87)] {
            assert!((horizontal(x) - y).abs() < 0.01, "S({x}) = {}", horizontal(x));
        }
    }
}
