# ZYKRO

Emulador de Android para jugar en Linux, hecho para KDE Plasma en Wayland. Corre Android 13 en un
contenedor [redroid](https://github.com/remote-android/redroid-doc) (código abierto), con traducción de
apps ARM y Google Play, y le pone una ventana propia: la imagen directo desde la GPU, sonido, mapeo de
teclas y una puntería que se siente como la de BlueStacks. Todo lo que hay entre la ventana y Android
(el servidor de pantalla, toques y sonido) es de este repositorio.

## Qué tiene

- **Android propio:** Android 13 de redroid con la GPU de la laptop (Mesa 24), Houdini 14 para las
  apps ARM y ARM64 (con el parche de la pantalla negra de Free Fire), Google Play (MindTheGapps) y el
  servidor de ZYKRO, que pasa la pantalla, los toques y el sonido. Se presenta como un teléfono normal:
  compilación de usuario, sin depuración ni root (los antitrampas de los juegos lo revisan). ZYKRO crea
  el contenedor la primera vez que lo abres, en el idioma de la laptop.
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
  - *Gráficos:* Vulkan y OpenGL o solo OpenGL, y la GPU de Android (con dos).
  - *Juegos:* las apps que abren ZYKRO en pantalla completa.
  - *Dispositivos* y *Gamepad:* la salida de sonido, y los mandos de la laptop (USB o Bluetooth)
    llegan a Android como mandos propios, con sus botones en su lugar y vibración.
  - *Preferencias:* sonido al tocar, Esc para salir de la pantalla completa, notificaciones de
    Android en el escritorio, accesos directos de las apps en el menú de la laptop y la carpeta de
    capturas.
  - *Teléfono:* el modelo que dice ser Android (Sony Xperia 1 V, Galaxy S23 Ultra, ROG Phone 7,
    OnePlus 11, Xiaomi 13 Pro, Pixel 7 Pro; todos con Android 13 y la huella de una versión real) y la
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
  quepan en una ventana de laptop. Sin barras de estado ni de navegación: las apps usan toda la imagen y
  los botones de Atrás, Inicio y Recientes están en la barra de ZYKRO.
- **Android más ligero:** ZYKRO quita las apps que no sirven en un emulador (protectores de pantalla,
  fondos animados, impresión, copias de seguridad, perfiles de empresa, NFC, MMS, calendario y lo que
  Google Play instala solo, como SafetyCore). Así corren unos 160 MB menos en segundo plano. Llamadas,
  Bluetooth, Wi-Fi, ubicación y cámara se quedan: los juegos los revisan para saber si corren en un
  emulador. Cuando la imagen declara Bluetooth deshabilitado, ZYKRO mantiene apagados el adaptador
  y la búsqueda BLE de Android para evitar que el servicio falle al arrancar sin su interfaz HCI;
  el Bluetooth de Linux y los mandos conectados a la laptop siguen funcionando.
  La lista está en `SOBRANTES` (`zykro/src/android.rs`); para devolver una app, sácala de
  ahí y corre `docker exec zykro-android cmd package install-existing <paquete>`.
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

- Linux con KDE Plasma 6 en Wayland. Probado en CachyOS; otros escritorios y distribuciones no se han
  probado.
- Un kernel con binder (`CONFIG_ANDROID_BINDERFS`), como el de CachyOS. En Ubuntu y Debian viene como
  módulo en el paquete `linux-modules-extra-$(uname -r)`: instálalo y `preparar-android.sh` lo carga.
- La GPU:
  - **Intel:** probada (Iris Xe).
  - **AMD:** sin probar; Android usa Mesa, así que debería funcionar.
  - **NVIDIA con su driver propietario:** ZYKRO todavía no puede mostrar la imagen.
- Docker, con tu usuario en los grupos `docker` e `input` (`sudo usermod -aG docker,input $USER` y
  volver a iniciar sesión).
- Rust (`cargo`), `gcc`, `git` y `curl`.
- PipeWire con `pipewire-pulse`, para el sonido.
- Unos 5 GB libres. La primera instalación baja cerca de 1.1 GB.

## Instalar

```bash
./instalar.sh
```

Compila ZYKRO, lo copia a `~/.local/bin`, crea su ícono en el menú y prepara Android con
`preparar-android.sh`:

1. Activa Docker si no está corriendo.
2. Monta los dispositivos `binder` en cada arranque (servicio `zykro-binderfs`); si tu kernel lo trae
   como módulo, también carga `binder_linux`.
3. Carga `uinput` en cada arranque.
4. Descarga Android 13 de redroid (unos 840 MB) y arma la imagen de ZYKRO con `imagen/construir.sh`:
   baja una vez Houdini 14 y MindTheGapps (unos 270 MB, verificados con su SHA-256) y les suma el
   servidor de ZYKRO.

El contenedor lo crea ZYKRO la primera vez que lo abres. Pide tu contraseña solo para los pasos 1 a 3,
y cada paso se salta si ya está hecho, así que puedes volver a correrlo después de actualizar el
repositorio. Con `--sin-android` solo instala ZYKRO.

Para copiar también nuestro mapeo de Free Fire (reemplaza el que tengas):

```bash
./instalar.sh --keymap
```

## Qué hay en el repositorio

| Carpeta | Qué es |
| --- | --- |
| `zykro/` | El código de ZYKRO (Rust, eframe/egui). |
| `launcher/` | La pantalla de inicio de Android (Java, sin Gradle). `launcher/construir.sh` la compila con el SDK de Android; el APK ya compilado viene incluido y ZYKRO lo instala solo en Android. |
| `servidor/` | El servidor de ZYKRO dentro de Android (Java, corre con `app_process`): la pantalla como DMA-BUF, sin comprimir ni copiar; los toques y teclas; el sonido en un anillo de memoria compartida; la app al frente y si un juego abrió su cuadro de texto. `servidor/construir.sh` lo compila con el SDK de Android; el `.jar` viene incluido. |
| `imagen/` | Arma la imagen de Android de ZYKRO (`imagen/construir.sh`): redroid 13 con Houdini 14 parcheado, Google Play, el servidor, las superposiciones y propiedades de un teléfono normal. Si la imagen cambia, ZYKRO recrea el contenedor con ella al abrirse (mismos datos). |
| `preparar-android.sh` | Deja listo Android: Docker, binder, uinput y la imagen. |
| `bin/teclado-android`, `src/teclado-fisico.c` | Crean un teclado "físico" vacío dentro de Android para que no muestre su teclado en pantalla; ZYKRO lo usa al arrancar. |
| `superposiciones/` | Superposiciones de recursos que adaptan la interfaz de Android a una ventana de laptop: `configuracion/` (la app de Configuración), `sistema/` (centro de control y notificaciones) y `marco/` (sin barras de estado ni de navegación). `superposiciones/construir.sh` las compila; los APK vienen incluidos. |
| `config/keymap/` | Nuestro mapeo de teclas para Free Fire. |
| `desktop/` | Acceso directo e ícono de ZYKRO. |

No se incluye ningún binario de Houdini (Intel), Google ni BlueStacks: `imagen/construir.sh` baja Houdini
y Google Play de sus fuentes públicas.

## Avisos

- **Juegos con antitrampas:** Free Fire usa el antitrampas de Tencent (ACE), que revisa el teléfono.
  Este Android se presenta como un teléfono normal, sin root ni depuración, pero no hay garantía de que
  todos los juegos lo acepten. Una versión anterior de ZYKRO, con Android 9 de depuración, hizo que se
  suspendiera una cuenta al entrar a una partida. Antes de usar tu cuenta principal, pruébalo con una
  que no te importe perder. Usarlo puede ir contra los términos de cada juego: el riesgo es tuyo.
- **Houdini es de Intel:** es software propietario y no viene en este repositorio. `imagen/construir.sh`
  lo baja del repositorio público
  [supremegamers/vendor_intel_proprietary_houdini](https://github.com/supremegamers/vendor_intel_proprietary_houdini)
  (el mismo que usa Waydroid con waydroid_script) y le aplica el parche de Free Fire. El propio Houdini
  avisa al arrancar que su uso requiere una licencia de Intel. Las apps de Google (MindTheGapps)
  también son propietarias.

## Si venías de la versión con SwagPlayer

ZYKRO ya no usa nada de SwagPlayer. Tu Android anterior (el contenedor `redroid-redroid-1`, con sus
apps y datos en el volumen `redroid_redroid_data`) queda intacto, pero apartado: el nuevo guarda sus
datos en el volumen `zykro-android13-datos`. Para llevarte tus apps y sesiones, copia los datos antes
de abrir ZYKRO por primera vez (el original no se toca):

```bash
docker run --rm -v redroid_redroid_data:/de -v zykro-android13-datos:/a alpine:3.20 cp -a /de/. /a/
```

Cuando ya no lo necesites, esto borra el Android anterior y libera unos 9 GB:

```bash
docker rm redroid-redroid-1 && docker volume rm redroid_redroid_data && docker rmi swagplayer-local:13.0.0-houdini geanswag/swagplayer:13.0.0
```

## Licencia

ZYKRO es software libre con licencia [MIT](LICENSE): puedes usarlo, copiarlo, modificarlo y
repartirlo como quieras, también dentro de otros proyectos, siempre que conserves el aviso de copyright
y la licencia. Se entrega tal cual, sin garantías.

La licencia cubre el código de este repositorio. Android, Google Play, Houdini, BlueStacks y Free Fire
son de sus respectivos dueños.
