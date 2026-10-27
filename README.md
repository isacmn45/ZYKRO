# ZYKRO

Emulador de Android para jugar en Linux, hecho para KDE Plasma en Wayland. Usa el Android de
[SwagPlayer](https://swagplayer.com) (un contenedor redroid con `swag-rust-server`) y le pone una
ventana propia, con la imagen directo desde la GPU, sonido, mapeo de teclas y una puntería que se siente
como la de BlueStacks.

## Qué tiene

- **Imagen sin comprimir y sin copias:** cada cuadro de Android llega como DMA-BUF y se dibuja directo
  desde la GPU. La ventana lleva su propio ritmo de cuadros con los avisos de KWin, para no sumar
  retraso.
- **Mapeo de teclas para juegos:** mismo formato que SwagPlayer/QtScrcpy, con un editor visual de
  controles sobre el juego y un menú rápido, al estilo de BlueStacks 5.
- **Puntería como BlueStacks:** el mouse se lee directo (evdev) y la cámara usa la misma fórmula que
  BlueStacks 5, así que la sensibilidad se siente igual. `` ` `` suspende o activa la puntería.
- **Sonido** de Android.
- **Modos de rendimiento:** perfil de energía, núcleos y prioridad de Android, y una velocidad mínima
  para la GPU Intel, que evita los tirones cuando la GPU baja de velocidad (en AMD, Ultra la deja al
  máximo).
- **Pantalla:** resolución, DPI y FPS de Android; pantalla completa con FPS y temperatura en una esquina;
  la ventana mantiene la proporción de la imagen al cambiar de tamaño.
- **Pantalla de inicio propia**, al estilo de BlueStacks 5 y compacta: buscador, tus apps y juegos, y
  una carpeta con las del sistema. Su engrane abre unos **ajustes de Android compactos** dentro de
  ZYKRO: apps (abrir, detener, borrar caché o datos, desinstalar), idioma, letra, animaciones y
  almacenamiento.
- **Configuración de Android compacta:** la app de Configuración de Android viene con letra, filas,
  íconos y encabezados más chicos, para que quepa en una ventana de laptop.
- **Extras:** selector de apps abiertas, capturas de pantalla, scroll con la rueda o el touchpad y
  temperatura de la CPU y la GPU en la barra de título.

## Requisitos

- Linux con KDE Plasma en Wayland (probado en CachyOS con Plasma 6).
- Un kernel con binder (`CONFIG_ANDROID_BINDERFS`), como el de CachyOS.
- Docker, con tu usuario en los grupos `docker` e `input`.
- Rust (`cargo`) y `gcc` para compilar.
- Unos 12 GB libres para la imagen de Android.
- Probado con una GPU Intel (Iris Xe). Con el driver propietario de NVIDIA, ZYKRO todavía no puede
  mostrar la imagen.

## Instalar

```bash
./instalar.sh
```

Compila ZYKRO, lo copia a `~/.local/bin`, crea su ícono en el menú y prepara Android con
`preparar-android.sh`:

1. Activa Docker si no está corriendo.
2. Monta los dispositivos `binder` en cada arranque (servicio `zykro-binderfs`).
3. Carga `uinput` en cada arranque.
4. Descarga la imagen de Android (`geanswag/swagplayer:13.0.0`, unos 6 GB; trae el servidor de video y
   control que usa ZYKRO) y construye la de ZYKRO (Houdini parcheado y Configuración compacta).
5. Crea el contenedor de Android.

Pide tu contraseña solo para los pasos 1 a 3, y cada paso se salta si ya está hecho, así que puedes
volver a correrlo después de actualizar el repositorio. No hace falta tener SwagPlayer instalado.
Con `--sin-android` solo instala ZYKRO.

Para copiar también nuestro mapeo de Free Fire (reemplaza el que tengas):

```bash
./instalar.sh --keymap
```

## Qué hay en el repositorio

| Carpeta | Qué es |
| --- | --- |
| `zykro/` | El código de ZYKRO (Rust, eframe/egui). |
| `launcher/` | La pantalla de inicio de Android (Java, sin Gradle). `launcher/construir.sh` la compila con el SDK de Android; el APK ya compilado viene incluido y ZYKRO lo instala solo en Android. |
| `preparar-android.sh` | Deja listo Android: Docker, binder, uinput, la imagen y el contenedor. |
| `bin/teclado-android`, `src/teclado-fisico.c` | Crean un teclado "físico" vacío dentro de Android para que no muestre su teclado en pantalla; ZYKRO lo usa al arrancar. |
| `houdini/` | Construye la imagen de Android de ZYKRO (`houdini/construir.sh`): un parche de 6 bytes en `libhoudini.so` y la Configuración compacta. Si la imagen cambia, ZYKRO recrea el contenedor con ella al abrirse (mismos datos). |
| `ajustes-compactos/` | Superposición de recursos que hace compacta la app de Configuración de Android (`ajustes-compactos/construir.sh`; el APK viene incluido). |
| `config/keymap/` | Nuestro mapeo de teclas para Free Fire. |
| `desktop/` | Acceso directo e ícono de ZYKRO. |

No se incluye ningún binario de SwagPlayer, BlueStacks ni la `libhoudini.so` de Intel.

## Licencia

ZYKRO es software libre con licencia [MIT](LICENSE): puedes usarlo, copiarlo, modificarlo y
repartirlo como quieras, también dentro de otros proyectos, siempre que conserves el aviso de copyright
y la licencia. Se entrega tal cual, sin garantías.

La licencia cubre el código de este repositorio. SwagPlayer, Android, BlueStacks y Free Fire son de
sus respectivos dueños.
