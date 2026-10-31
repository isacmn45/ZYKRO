//! Pestaña Teléfono, como la de BlueStacks: con qué teléfono se presenta Android ante las apps
//! (algunos juegos dan más gráficos o FPS según el modelo) y qué operadora de telefonía dice tener.

use std::path::PathBuf;

/// Un teléfono: lo que leen las apps de `ro.product.*` y `ro.build.*`. Todos salieron con Android 9,
/// como el de ZYKRO, y la huella es la de su versión real: Google Play revisa que coincida con un
/// teléfono certificado.
pub struct Perfil {
    pub nombre: &'static str,
    pub marca: &'static str,
    pub fabricante: &'static str,
    pub modelo: &'static str,
    /// `ro.product.name` y `ro.product.device`.
    pub producto: &'static str,
    pub dispositivo: &'static str,
    pub huella: &'static str,
    pub descripcion: &'static str,
    /// Parche de seguridad de esa versión (`ro.build.version.security_patch`).
    pub parche: &'static str,
}

/// El primero es con el que se crea Android.
#[rustfmt::skip]
pub const PERFILES: [Perfil; 6] = [
    Perfil { nombre: "Google Pixel 3", marca: "google", fabricante: "Google", modelo: "Pixel 3", producto: "blueline", dispositivo: "blueline",
        huella: "google/blueline/blueline:9/PQ3A.190801.002/5670241:user/release-keys",
        descripcion: "blueline-user 9 PQ3A.190801.002 5670241 release-keys", parche: "2019-08-01" },
    Perfil { nombre: "Samsung Galaxy S10+", marca: "samsung", fabricante: "samsung", modelo: "SM-G975U", producto: "beyond2qlteue", dispositivo: "beyond2q",
        huella: "samsung/beyond2qlteue/beyond2q:9/PPR1.180610.011/G975U1UEU1ASD3:user/release-keys",
        descripcion: "beyond2qlteue-user 9 PPR1.180610.011 G975U1UEU1ASD3 release-keys", parche: "2019-03-01" },
    Perfil { nombre: "ASUS ZenFone 6", marca: "asus", fabricante: "asus", modelo: "ASUS_I01WD", producto: "WW_I01WD", dispositivo: "ASUS_I01WD",
        huella: "asus/WW_I01WD/ASUS_I01WD:9/PPR2.181005.003/16.1220.1909.193-0:user/release-keys",
        descripcion: "WW_I01WD-user 9 PPR2.181005.003 16.1220.1909.193-0 release-keys", parche: "2019-09-01" },
    Perfil { nombre: "OnePlus 7 Pro", marca: "OnePlus", fabricante: "OnePlus", modelo: "GM1913", producto: "OnePlus7Pro_EEA", dispositivo: "OnePlus7Pro",
        huella: "OnePlus/OnePlus7Pro_EEA/OnePlus7Pro:9/PKQ1.190110.001/1907281556:user/release-keys",
        descripcion: "OnePlus7Pro_EEA-user 9 PKQ1.190110.001 1907281556 release-keys", parche: "2019-08-01" },
    Perfil { nombre: "Xiaomi Mi 9", marca: "Xiaomi", fabricante: "Xiaomi", modelo: "MI 9", producto: "cepheus_eea", dispositivo: "cepheus",
        huella: "Xiaomi/cepheus_eea/cepheus:9/PKQ1.181121.001/V10.2.27.0.PFAEUXM:user/release-keys",
        descripcion: "cepheus_eea-user 9 PKQ1.181121.001 V10.2.27.0.PFAEUXM release-keys", parche: "2019-06-01" },
    Perfil { nombre: "Xiaomi Redmi Note 7", marca: "xiaomi", fabricante: "Xiaomi", modelo: "Redmi Note 7", producto: "lavender", dispositivo: "lavender",
        huella: "xiaomi/lavender/lavender:9/PKQ1.180904.001/V11.0.5.0.PFGMIXM:user/release-keys",
        descripcion: "lavender-user 9 PKQ1.180904.001 V11.0.5.0.PFGMIXM release-keys", parche: "2020-01-05" },
];

/// Las particiones que también anuncian el teléfono (`ro.product.<particion>.*`).
const PARTICIONES: [&str; 5] = ["", "system.", "vendor.", "product.", "odm."];

impl Perfil {
    /// Las propiedades de Android con este teléfono.
    pub fn propiedades(&self) -> Vec<(String, String)> {
        let mut args = Vec::new();
        for p in PARTICIONES {
            args.push((format!("ro.product.{p}model"), self.modelo.to_string()));
            args.push((format!("ro.product.{p}brand"), self.marca.to_string()));
            args.push((format!("ro.product.{p}name"), self.producto.to_string()));
            args.push((format!("ro.product.{p}device"), self.dispositivo.to_string()));
            args.push((format!("ro.product.{p}manufacturer"), self.fabricante.to_string()));
        }
        args.push(("ro.build.product".into(), self.dispositivo.to_string()));
        args.push(("ro.build.flavor".into(), format!("{}-user", self.producto)));
        args.push(("ro.build.description".into(), self.descripcion.to_string()));
        args.push(("ro.build.fingerprint".into(), self.huella.to_string()));
        args.push(("ro.build.version.security_patch".into(), self.parche.to_string()));
        // La imagen es de pruebas (userdebug, test-keys): un teléfono de verdad dice user y release-keys.
        args.push(("ro.build.type".into(), "user".into()));
        args.push(("ro.build.tags".into(), "release-keys".into()));
        args
    }
}

/// Archivo con las propiedades del teléfono elegido. Android lo ve como /product/build.prop y lo lee
/// antes que las suyas. No van como argumentos de arranque porque redroid los corta en el primer
/// espacio («Pixel 3» quedaba «Pixel»).
pub fn archivo() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"))
        .join("zykro/telefono.prop")
}

/// Escribe el archivo del teléfono `perfil` (vacío con `None`: Android se presenta como es).
pub fn escribir(perfil: Option<usize>) -> std::io::Result<()> {
    let texto: String = perfil
        .and_then(|i| PERFILES.get(i))
        .map(|p| p.propiedades().into_iter().map(|(k, v)| format!("{k}={v}\n")).collect())
        .unwrap_or_default();
    let ruta = archivo();
    if let Some(carpeta) = ruta.parent() {
        std::fs::create_dir_all(carpeta)?;
    }
    std::fs::write(ruta, texto)
}

/// Una operadora: país, nombre y su código (MCC y MNC).
pub struct Operadora {
    pub pais: &'static str,
    /// Código ISO del país, en minúsculas, como lo da Android.
    pub iso: &'static str,
    pub nombre: &'static str,
    pub codigo: &'static str,
}

#[rustfmt::skip]
pub const OPERADORAS: [Operadora; 21] = [
    Operadora { pais: "México", iso: "mx", nombre: "Telcel", codigo: "334020" },
    Operadora { pais: "México", iso: "mx", nombre: "AT&T", codigo: "334050" },
    Operadora { pais: "México", iso: "mx", nombre: "Movistar", codigo: "334030" },
    Operadora { pais: "Estados Unidos", iso: "us", nombre: "AT&T", codigo: "310410" },
    Operadora { pais: "Estados Unidos", iso: "us", nombre: "T-Mobile", codigo: "310260" },
    Operadora { pais: "Estados Unidos", iso: "us", nombre: "Verizon", codigo: "311480" },
    Operadora { pais: "España", iso: "es", nombre: "Movistar", codigo: "21407" },
    Operadora { pais: "España", iso: "es", nombre: "Vodafone", codigo: "21401" },
    Operadora { pais: "España", iso: "es", nombre: "Orange", codigo: "21403" },
    Operadora { pais: "Colombia", iso: "co", nombre: "Claro", codigo: "732101" },
    Operadora { pais: "Colombia", iso: "co", nombre: "Movistar", codigo: "732123" },
    Operadora { pais: "Colombia", iso: "co", nombre: "Tigo", codigo: "732103" },
    Operadora { pais: "Argentina", iso: "ar", nombre: "Claro", codigo: "722310" },
    Operadora { pais: "Argentina", iso: "ar", nombre: "Movistar", codigo: "72207" },
    Operadora { pais: "Argentina", iso: "ar", nombre: "Personal", codigo: "72234" },
    Operadora { pais: "Chile", iso: "cl", nombre: "Entel", codigo: "73001" },
    Operadora { pais: "Chile", iso: "cl", nombre: "Movistar", codigo: "73002" },
    Operadora { pais: "Chile", iso: "cl", nombre: "Claro", codigo: "73003" },
    Operadora { pais: "Perú", iso: "pe", nombre: "Claro", codigo: "71610" },
    Operadora { pais: "Perú", iso: "pe", nombre: "Movistar", codigo: "71606" },
    Operadora { pais: "Perú", iso: "pe", nombre: "Entel", codigo: "71617" },
];

/// Orden de la shell de Android que anuncia la operadora (la de la SIM y la de la red); con `None`,
/// que no hay ninguna.
pub fn orden_operadora(codigo: Option<&str>) -> String {
    let (nombre, codigo, iso) = match codigo.and_then(|c| OPERADORAS.iter().find(|o| o.codigo == c)) {
        Some(o) => (o.nombre, o.codigo, o.iso),
        None => ("", "", ""),
    };
    ["gsm.sim.operator", "gsm.operator"]
        .iter()
        .map(|p| format!("setprop {p}.alpha '{nombre}'; setprop {p}.numeric '{codigo}'; setprop {p}.iso-country '{iso}'"))
        .collect::<Vec<_>>()
        .join("; ")
}
