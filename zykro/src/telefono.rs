//! Pestaña Teléfono, como la de BlueStacks: con qué teléfono se presenta Android ante las apps
//! (algunos juegos dan más gráficos o FPS según el modelo) y qué operadora de telefonía dice tener.

/// Un teléfono: lo que leen las apps de `ro.product.*` y `ro.build.*`.
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
}

/// El primero es con el que se crea Android (`preparar-android.sh`).
#[rustfmt::skip]
pub const PERFILES: [Perfil; 6] = [
    Perfil { nombre: "Sony Xperia 1 V", marca: "Sony", fabricante: "Sony", modelo: "XQ-DQ72", producto: "XQ-DQ72", dispositivo: "XQ-DQ72",
        huella: "Sony/XQ-DQ72/XQ-DQ72:13/67.0.A.4.51/067000A004005100:user/release-keys",
        descripcion: "XQ-DQ72-user 13 67.0.A.4.51 067000A004005100 release-keys" },
    Perfil { nombre: "Samsung Galaxy S23 Ultra", marca: "samsung", fabricante: "samsung", modelo: "SM-S918B", producto: "dm3qxxx", dispositivo: "dm3q",
        huella: "samsung/dm3qxxx/dm3q:13/TP1A.220624.014/S918BXXU1AWBD:user/release-keys",
        descripcion: "dm3qxxx-user 13 TP1A.220624.014 S918BXXU1AWBD release-keys" },
    Perfil { nombre: "ASUS ROG Phone 7", marca: "asus", fabricante: "asus", modelo: "ASUS_AI2205", producto: "WW_AI2205", dispositivo: "ASUS_AI2205",
        huella: "asus/WW_AI2205/ASUS_AI2205:13/TKQ1.221013.002/33.0610.2810.98:user/release-keys",
        descripcion: "WW_AI2205-user 13 TKQ1.221013.002 33.0610.2810.98 release-keys" },
    Perfil { nombre: "OnePlus 11", marca: "OnePlus", fabricante: "OnePlus", modelo: "CPH2449", producto: "CPH2449", dispositivo: "OP594DL1",
        huella: "OnePlus/CPH2449/OP594DL1:13/TP1A.220905.001/R.1038728_2_1:user/release-keys",
        descripcion: "CPH2449-user 13 TP1A.220905.001 R.1038728_2_1 release-keys" },
    Perfil { nombre: "Xiaomi 13 Pro", marca: "Xiaomi", fabricante: "Xiaomi", modelo: "2210132G", producto: "nuwa_global", dispositivo: "nuwa",
        huella: "Xiaomi/nuwa_global/nuwa:13/TKQ1.221114.001/V14.0.4.0.TMBMIXM:user/release-keys",
        descripcion: "nuwa_global-user 13 TKQ1.221114.001 V14.0.4.0.TMBMIXM release-keys" },
    Perfil { nombre: "Google Pixel 7 Pro", marca: "google", fabricante: "Google", modelo: "Pixel 7 Pro", producto: "cheetah", dispositivo: "cheetah",
        huella: "google/cheetah/cheetah:13/TQ3A.230901.001/10750268:user/release-keys",
        descripcion: "cheetah-user 13 TQ3A.230901.001 10750268 release-keys" },
];

/// Las particiones que también anuncian el teléfono (`ro.product.<particion>.*`).
const PARTICIONES: [&str; 6] = ["", "system.", "vendor.", "product.", "odm.", "system_ext."];

impl Perfil {
    /// Argumentos de arranque de Android con este teléfono.
    pub fn argumentos(&self) -> Vec<(String, String)> {
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
        args
    }

    /// El perfil de los argumentos actuales (por el modelo), si es uno de la lista.
    pub fn desde_argumentos(args: &[String]) -> Option<usize> {
        let modelo = args.iter().find_map(|a| a.strip_prefix("ro.product.model="))?;
        PERFILES.iter().position(|p| p.modelo == modelo)
    }
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
