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
- **Extras:** selector de apps abiertas, capturas de pantalla, scroll con la rueda o el touchpad y
  temperatura de la CPU y la GPU en la barra de título.

## Requisitos

- Linux con KDE Plasma en Wayland (probado en CachyOS con Plasma 6).
- SwagPlayer instalado, que crea el contenedor de Android y trae su servidor. ZYKRO no incluye
  programas de SwagPlayer.
- Docker, con tu usuario en los grupos `docker` e `input`.
- Rust (`cargo`) para compilar.
- Probado con una GPU Intel (Iris Xe). Con el driver propietario de NVIDIA, ZYKRO todavía no puede
  mostrar la imagen.

## Instalar

```bash
./instalar.sh
```

Compila ZYKRO, lo copia a `~/.local/bin` y crea su ícono en el menú. Vuelve a correrlo después de
actualizar el repositorio.

## Herramientas para SwagPlayer

Antes de ZYKRO, este repositorio tenía mejoras para el propio SwagPlayer, que siguen aquí:

| Mejora | Archivo | Qué hace |
| --- | --- | --- |
| Lanzador | `bin/swagplayer-iniciar` | Abre SwagPlayer con todo lo de abajo y apaga Android al cerrarlo. |
| Cierre sin cuelgues | `src/swag-closefix.c` | Se precarga (`LD_PRELOAD`) y libera los sockets de video/portapapeles para que SwagPlayer cierre sin quedarse colgado. |
| Sin teclado en pantalla | `src/teclado-fisico.c`, `bin/teclado-android` | Crea un teclado "físico" vacío dentro de Android (uinput) para que Android oculte su teclado en pantalla. |
| Escribir en el chat | `bin/ff-escribir` | Cuando el juego abre su cuadro de texto, toma el teclado y manda las letras directo a Android. |
| Cerrar desde el menú | `bin/cerrar-swagplayer` | Cierra SwagPlayer y apaga Android. |
| Houdini parcheado | `houdini/` | Construye una imagen con un parche de 6 bytes en `libhoudini.so`. |
| Mapeo de Free Fire | `config/keymap/com.dts.freefireth.json` | Nuestro mapeo de teclas para Free Fire (`./instalar.sh --keymap`). |

## Notas

- No se incluye ningún binario de SwagPlayer, BlueStacks ni la `libhoudini.so` de Intel.
