# swagplayer-mods

Mejoras propias para [SwagPlayer](https://swagplayer.com) (Android en Linux con redroid) en
CachyOS + KDE Plasma. SwagPlayer no publica su código, así que estas mejoras viven **alrededor**
del programa: un lanzador, scripts y pequeños programas que actúan sobre Android y el teclado.

## Qué incluye

| Mejora | Archivo | Qué hace |
| --- | --- | --- |
| Lanzador | `bin/swagplayer-iniciar` | Abre SwagPlayer con todo lo de abajo y apaga Android al cerrarlo. |
| Cierre sin cuelgues | `src/swag-closefix.c` | Se precarga (`LD_PRELOAD`) y libera los sockets de video/portapapeles para que SwagPlayer cierre sin quedarse colgado. |
| Sin teclado en pantalla | `src/teclado-fisico.c`, `bin/teclado-android` | Crea un teclado "físico" vacío dentro de Android (uinput) para que Android oculte su teclado en pantalla, como BlueStacks. |
| Escribir en el chat | `bin/ff-escribir` | Cuando el juego abre su cuadro de texto, toma el teclado y manda las letras directo a Android, sin pasar por el mapeo. Al cerrarse el cuadro (Enter o Esc) devuelve el teclado. |
| Cerrar desde el menú | `bin/cerrar-swagplayer` | Acción "Cerrar Swagplayer" del ícono: cierra SwagPlayer y apaga Android. |
| Houdini parcheado | `houdini/` | Construye `swagplayer-local:13.0.0-houdini` con nuestro parche de 6 bytes en `libhoudini.so`. |
| Mapeo de Free Fire | `config/keymap/com.dts.freefireth.json` | Nuestro mapeo de teclas para Free Fire. |

## Instalar

Requisitos: SwagPlayer instalado (`/usr/bin/swagplayer`), `gcc`, `docker` y el usuario en los
grupos `docker` e `input`.

```bash
./instalar.sh
```

Para copiar también el mapeo de Free Fire (sobrescribe el actual):

```bash
./instalar.sh --keymap
```

Para la imagen con Houdini (solo la primera vez o si cambia la imagen base):

```bash
houdini/construir.sh
```

## Uso

- **Escribir en el chat:** haz clic en el cuadro de texto del juego y escribe. Enter envía, Esc
  cierra. Si el teclado se queda "atrapado", **Alt** o la **tecla Windows** lo liberan al instante.
- **Teclado en pantalla:** `teclado-android` lo muestra u oculta (alterna). También acepta
  `ocultar` o `mostrar`.
- **`` ` ``** (tecla de SwagPlayer) suspende el puntero del modo aim; no cambia el mapeo.

## Cómo funciona el modo escribir

1. Escucha los cambios de foco de Android (`logcat -b events -s input_focus`), sin gastar recursos
   mientras juegas.
2. Si la ventana nueva es el cuadro de texto del juego (no es `BASE_APPLICATION` y pide teclado
   `always_visible`), toma el teclado de la laptop en exclusiva (`EVIOCGRAB`). SwagPlayer deja de
   recibir teclas y no hay toques mapeados.
3. Cada tecla se traduce (distribución US) y se manda con `input text` / `input keyevent` por una
   shell abierta dentro de Android (~40 ms por tecla).
4. Al cerrarse el cuadro devuelve el teclado. Además revisa cada 1.5 s por si Android no avisó.

## Notas

- `SWAG_DMABUF_FORCE_CPU=1` en el lanzador evita la pantalla negra al iniciar, a costa de algo de
  fluidez.
- No se incluye ningún binario de SwagPlayer ni la `libhoudini.so` de Intel.
