# ZYKRO

Emulador de Android para jugar en Linux, hecho para KDE Plasma en Wayland. Usa el Android de
[SwagPlayer](https://swagplayer.com) (un contenedor redroid con `swag-rust-server`) y le pone una
ventana propia, con la imagen directo desde la GPU, sonido, mapeo de teclas y una puntería que se siente
como la de BlueStacks.

## Qué tiene

- **Imagen sin comprimir y sin copias:** cada cuadro de Android llega como DMA-BUF y se dibuja directo
  desde la GPU. La ventana lleva su propio ritmo de cuadros con los avisos de KWin, para no sumar
  retraso.
- **Mapeo de teclas para juegos con los controles de BlueStacks 5:** tocar el punto, toques
  repetidos, D-pad (con teclas para correr, velocidad, activación y zona muerta), apuntar y disparar,
  mirada libre, script (mismos comandos que BlueStacks), deslizar, zoom, D-pad y habilidad MOBA
  (lanzamiento rápido, automático o manual), rotar, desplazamiento de borde, desplazarse y rueda del
  mouse, cada uno con sus ajustes avanzados. Apuntar tiene punto de mira (estilo, tamaño, opacidad y
  color), aceleración del mouse y mirar alrededor sin mover la mira. Varios esquemas de controles por
  juego, que se importan, exportan y cambian desde el menú rápido. Lee los mapas de SwagPlayer y
  QtScrcpy. La inclinación no está: este Android no tiene sensores.
- **Puntería como BlueStacks:** el mouse se lee directo (evdev) y la cámara usa la misma fórmula que
  BlueStacks 5, así que la sensibilidad se siente igual. `` ` `` suspende o activa la puntería.
- **Sonido** de Android.
- **Configuración con las secciones de BlueStacks 5**, solo con lo que de verdad funciona aquí:
  - *Rendimiento:* modos (perfil de energía, prioridad de Android y una velocidad mínima para la GPU
    Intel, que evita los tirones; en AMD, Ultra la deja al máximo), núcleos para Android y FPS
    (hasta 240).
  - *Pantalla:* orientación, resolución y DPI (también personalizados), tamaño de la interfaz de
    ZYKRO y un cursor propio, grande y visible sobre cualquier juego.
  - *Gráficos:* Vulkan y OpenGL o solo OpenGL, la GPU de Android (con dos) y texturas suaves.
  - *Juegos:* las apps que abren ZYKRO en pantalla completa.
  - *Dispositivos* y *Gamepad:* la salida de sonido, y los mandos de la laptop (USB o Bluetooth)
    llegan a Android como mandos propios, con sus botones en su lugar y vibración.
  - *Preferencias:* sonido al tocar, Esc para salir de la pantalla completa, notificaciones de
    Android en el escritorio, accesos directos de las apps en el menú de la laptop y la carpeta de
    capturas.
  - *Teléfono:* el modelo que dice ser Android (Sony, Samsung, ASUS ROG, OnePlus, Xiaomi, Pixel) y la
    operadora de telefonía.
  - *Atajos:* las teclas de cada acción (ocultar ZYKRO, captura, volumen, inicio, controles…).
  - *Datos del usuario:* limpieza del disco y respaldo y restauración de todo Android en un archivo.
  - *Avanzado* y *Acerca de:* las arquitecturas, ADB (se autoriza solo) y buscar actualizaciones.

  No están la cámara ni el micrófono (este Android no los tiene) ni la memoria (Android comparte la de
  la laptop).
- **Pantalla completa** con FPS y temperatura en una esquina; la ventana mantiene la proporción de la
  imagen al cambiar de tamaño. La pantalla de carga es un cielo con planetas, como la de BlueStacks.
- **Pantalla de inicio propia**, al estilo de BlueStacks 5 y compacta: buscador, tus apps y juegos, y
  una carpeta con las del sistema. Mantener presionado un espacio vacío abre los **fondos de
  pantalla**: doce fondos de ZYKRO, una imagen de tu computadora o una de la galería de Android
  (se recorta sola a la pantalla y respeta las fotos giradas del celular).
- **Ajustes de Android compactos** dentro de ZYKRO, con el botón «Ajustes» del panel de
  notificaciones: apps (abrir, detener, borrar caché o datos, desinstalar), idioma, letra,
  animaciones y almacenamiento.
- **Interfaz de Android compacta:** la app de Configuración, el centro de control, el panel de
  notificaciones y el de volumen vienen con letra, filas, botones y encabezados más chicos, para que
  quepan en una ventana de laptop.
- **Android más ligero:** ZYKRO quita las apps que no sirven en un emulador (protectores de pantalla,
  fondos animados, impresión, copias de seguridad, perfiles de empresa, NFC, MMS, calendario y lo que
  Google Play instala solo, como SafetyCore). Así corren unos 160 MB menos en segundo plano. Llamadas,
  Bluetooth, Wi-Fi, ubicación y cámara se quedan: los juegos los revisan para saber si corren en un
  emulador. Cuando la imagen declara Bluetooth deshabilitado, ZYKRO mantiene apagados el adaptador
  y la búsqueda BLE de Android para evitar que el servicio falle al arrancar sin su interfaz HCI;
  el Bluetooth de Linux y los mandos conectados a la laptop siguen funcionando.
  La lista está en `SOBRANTES` (`zykro/src/android.rs`); para devolver una app, sácala de
  ahí y corre `docker exec redroid-redroid-1 cmd package install-existing <paquete>`.
- **Panel de notificaciones propio**, compacto como el de Android 4 y con el tema de ZYKRO: se baja
  arrastrando desde el borde de arriba de la imagen (Android no muestra barra de estado, como en
  BlueStacks), la cubre entera y se sube arrastrándolo hacia arriba (o con Atrás o Esc).
  Interruptores (avión, no molestar, girar, captura, ajustes), volumen y las notificaciones en
  tarjetas, que se abren, se quitan o se borran todas. El panel moderno de Android sigue a un toque,
  para la música o responder mensajes.
- **Notificaciones emergentes con el tema de ZYKRO:** las que suenan aparecen arriba de la imagen
  unos segundos; se abren con un toque, se quitan con la X o se despachan arrastrándolas hacia
  arriba. Respetan «No molestar», y con la puntería activa solo se ven, sin quitarle el mouse al
  juego.
- **Extras:** selector de apps abiertas, capturas de pantalla, scroll con la rueda o el touchpad y
  temperatura de la CPU y la GPU en la barra de título.
- **Reiniciar:** el diálogo de cierre permite cancelar, cerrar o reiniciar ZYKRO y Android.
  Reiniciar conserva las apps, datos y configuración, y carga el ejecutable instalado más reciente.

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
   control que usa ZYKRO) y construye la de ZYKRO (Houdini parcheado e interfaz compacta).
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
| `houdini/` | Construye la imagen de Android de ZYKRO (`houdini/construir.sh`): un parche de 6 bytes en `libhoudini.so` y las superposiciones. Si la imagen cambia, ZYKRO recrea el contenedor con ella al abrirse (mismos datos). |
| `superposiciones/` | Superposiciones de recursos que hacen compacta la interfaz de Android: `configuracion/` (la app de Configuración) y `sistema/` (centro de control y notificaciones). `superposiciones/construir.sh` las compila; los APK vienen incluidos. |
| `config/keymap/` | Nuestro mapeo de teclas para Free Fire. |
| `desktop/` | Acceso directo e ícono de ZYKRO. |

No se incluye ningún binario de SwagPlayer, BlueStacks ni la `libhoudini.so` de Intel.

## Licencia

ZYKRO es software libre con licencia [MIT](LICENSE): puedes usarlo, copiarlo, modificarlo y
repartirlo como quieras, también dentro de otros proyectos, siempre que conserves el aviso de copyright
y la licencia. Se entrega tal cual, sin garantías.

La licencia cubre el código de este repositorio. SwagPlayer, Android, BlueStacks y Free Fire son de
sus respectivos dueños.
