# Comprobación de rendimiento (31 de octubre de 2026)

Equipo observado: Intel Core i7-1185G7 (8 hilos), GPU Intel, 32 GB de RAM.
Android estaba en el launcher, a 1920 × 1080, 60 FPS y DPI 360. El modo guardado
era Ultra; los límites mínimo y máximo de la GPU leídos en sysfs eran 1350 MHz.
No se cambiaron resolución, FPS, calidad, energía ni parámetros del sistema.

## Cambios

- La puntería toma prestados sus ajustes en lugar de clonar cadenas y valores
  JSON con cada reporte del mouse. Conserva la fórmula y los eventos enviados.
- El temporizador actualiza su reloj pero evita copiar el mapa y recorrer los
  controles cuando no hay acciones pendientes. Los toques programados, D-pad,
  scripts y otros controles animados siguen por el camino habitual.
- El lector evdev consulta si la puntería está activa al recibir el reporte,
  después de la espera de `poll`, en lugar de conservar el estado de antes de
  esperar. Evita descartar el primer movimiento o procesar con un estado viejo
  al activar o desactivar la puntería durante esa espera (hasta 200 ms).
- El ayudante `teclado-android` se espera en un hilo de fondo que ya existía.
  Su terminación se recoge sin bloquear la interfaz. El cliente anterior tenía
  dos hijos de ese nombre en estado zombie; no consumían CPU por estar zombies.

## Comparación reproducible

Comando, desde `zykro/`:

```sh
cargo test --release medir_rutas_de_entrada -- --ignored --nocapture
```

La prueba usa el mapa de Free Fire incluido, cinco pasadas de 100.000 ticks sin
acciones y 100.000 movimientos de puntería, contra un socket Unix de prueba.
Comprueba que cada pasada produce los 100.001 toques esperados (bajar el dedo
y un movimiento por reporte). El tiempo de movimiento mide el cálculo y envío
a la cola; no mide la llegada a Android, la GPU, KWin ni la pantalla.

Medianas en la misma sesión, con compilación optimizada y prioridad baja:

| Trabajo | Antes (base `0cb52ac`) | Después | Reducción |
| --- | ---: | ---: | ---: |
| 100.000 ticks sin acciones | 576,34 ms | 3,81 ms | 99,3 % |
| 100.000 movimientos | 63,24 ms | 10,33 ms | 83,7 % |

Son mejoras de estas rutas, no porcentajes de FPS ni de latencia total.
La prueba funcional adicional comprueba que un tick inactivo renueva su reloj
y que un toque programado todavía se entrega. La suite también cubre la
fórmula de puntería, scripts y controles animados.

## Límites de la observación en vivo

Una muestra acotada de cinco segundos del cliente anterior, con el launcher
al frente de Android y sin interacción sintética, dio 0,8 % de un núcleo de CPU.
Su RSS observado fue 154.752 KiB. No constituye una comparación de CPU durante
un juego. Los contadores acumulados de `gfxinfo` del launcher incluían cambios
y aperturas anteriores; no se usan como medida de FPS ni de mejora. La consulta
global de latencia de SurfaceFlinger contenía timestamps centinela y tampoco
permite calcular una latencia válida de presentación del cliente.

La ruta de video conserva DMA-BUF/EGL, la cola de dos cuadros y los avisos de
Wayland. Audio conserva su búfer de 60 ms y límite de retraso de 120 ms.
No se modificaron sin una medición comparable con juego activo.

El proceso ya abierto no carga las modificaciones del nuevo ejecutable.
La compilación nueva debe usarse en la siguiente apertura del cliente. Cerrar
ZYKRO normalmente también apaga Android; no se hizo durante esta revisión.
Para la siguiente comparación en un juego, usar el mismo escenario, resolución,
FPS y modo, arrancar con `ZYKRO_RITMO=1` y comparar los registros de intervalos,
cuadros tapados/repetidos y retraso. Esas medidas aún no se han tomado.

## Aplicación de fondos

El codificador PNG de los fondos pasa de `Balanced` a `Fast`, sin cambiar los
píxeles ni la resolución. La prueba de ida y vuelta decodifica el PNG y comprueba
que coincide con los píxeles originales. Para repetir la medición:

```sh
cargo test --release medir_compresion_de_fondos -- --ignored --nocapture
```

Con el fondo Neón a 1920 × 1080, tres pasadas en esta máquina dieron medianas
de 713,69 ms con Balanced y 27,90 ms con Fast. Los archivos fueron de 1.561.377
y 2.300.042 bytes, respectivamente. Se mide solamente la compresión en memoria,
no la latencia completa hasta la pantalla.

El launcher muestra el bitmap preparado antes de persistirlo y no vuelve a
decodificar el PNG recién guardado. Si ya coincide en tamaño y no necesita giro,
reutiliza el bitmap sin una copia de pantalla completa. Una carga antigua del
disco tampoco puede reemplazar una selección nueva. Estas mejoras no cambian
los colores, el recorte, la resolución ni la persistencia del fondo.
