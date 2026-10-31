package com.zykro.servidor;

import android.hardware.input.InputManager;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.SystemClock;
import android.util.Log;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;
import java.io.BufferedInputStream;
import java.io.DataInputStream;
import java.io.EOFException;
import java.io.IOException;
import java.lang.reflect.Method;

/**
 * Toques y teclas desde ZYKRO (`control.sock`). Mensajes sin largo delante; el primer byte es el tipo
 * y los enteros van en big-endian (los de tecla, toque y scroll son los de scrcpy):
 *
 * <pre>
 * 0x00 tecla:  acción u8, keycode u32, repeticiones u32, meta u32
 * 0x02 toque:  acción u8, dedo u64, x i32, y i32, ancho u16, alto u16, presión u16, botón u32, botones u32
 * 0x03 scroll: x i32, y i32, ancho u16, alto u16, horizontal i16, vertical i16, botones u32
 * 0x12 distribución del teclado: largo u8 y el nombre (se ignora)
 * 0x14 5 bytes (se ignoran)
 * </pre>
 */
final class Control {
    private static final int INYECTAR_SIN_ESPERAR = 0;
    private static final int MAX_DEDOS = 10;

    private static Object entrada;
    private static Method inyectar;

    private Control() {}

    static void servir(String ruta) throws Exception {
        entrada = InputManager.class.getMethod("getInstance").invoke(null);
        inyectar = InputManager.class.getMethod("injectInputEvent", InputEvent.class, int.class);
        LocalServerSocket servidor = Servidor.escuchar(ruta, LocalSocket.SOCKET_STREAM);
        while (true) {
            LocalSocket cliente = servidor.accept();
            new Thread(() -> atender(cliente), "zykro-control-cliente").start();
        }
    }

    private static void atender(LocalSocket cliente) {
        Dedos dedos = new Dedos();
        try (DataInputStream in = new DataInputStream(new BufferedInputStream(cliente.getInputStream()))) {
            Log.i(Servidor.ETIQUETA, "control: cliente conectado");
            while (true) {
                int tipo = in.read();
                if (tipo < 0) {
                    break;
                }
                switch (tipo) {
                    case 0x00: {
                        int accion = in.readUnsignedByte();
                        int codigo = in.readInt();
                        int repeticiones = in.readInt();
                        int meta = in.readInt();
                        tecla(accion, codigo, repeticiones, meta);
                        break;
                    }
                    case 0x02: {
                        int accion = in.readUnsignedByte();
                        long dedo = in.readLong();
                        int x = in.readInt();
                        int y = in.readInt();
                        int ancho = in.readUnsignedShort();
                        int alto = in.readUnsignedShort();
                        int presion = in.readUnsignedShort();
                        in.readInt(); // botón de acción
                        in.readInt(); // botones
                        dedos.toque(accion, dedo, x, y, ancho, alto, presion / 65535f);
                        break;
                    }
                    case 0x03: {
                        int x = in.readInt();
                        int y = in.readInt();
                        int ancho = in.readUnsignedShort();
                        int alto = in.readUnsignedShort();
                        short horizontal = in.readShort();
                        short vertical = in.readShort();
                        in.readInt(); // botones
                        scroll(x, y, ancho, alto, horizontal / 32768f * 16, vertical / 32768f * 16);
                        break;
                    }
                    case 0x12:
                        in.skipBytes(in.readUnsignedByte());
                        break;
                    case 0x14:
                        in.skipBytes(5);
                        break;
                    default:
                        // Sin largo no hay forma de saltarlo: se corta para no leer basura como toques.
                        throw new IOException("mensaje desconocido " + tipo);
                }
            }
        } catch (EOFException e) {
            // ZYKRO se cerró
        } catch (Exception e) {
            Log.w(Servidor.ETIQUETA, "control: " + e);
        } finally {
            dedos.soltarTodos();
            Servidor.cerrar(cliente);
            Log.i(Servidor.ETIQUETA, "control: cliente desconectado");
        }
    }

    static void inyectar(InputEvent evento) {
        try {
            inyectar.invoke(entrada, evento, INYECTAR_SIN_ESPERAR);
        } catch (Exception e) {
            Log.w(Servidor.ETIQUETA, "control: no se pudo inyectar " + evento, e);
        }
    }

    private static void tecla(int accion, int codigo, int repeticiones, int meta) {
        long ahora = SystemClock.uptimeMillis();
        inyectar(new KeyEvent(ahora, ahora, accion, codigo, repeticiones, meta, KeyCharacterMap.VIRTUAL_KEYBOARD, 0, 0,
            InputDevice.SOURCE_KEYBOARD));
    }

    private static void scroll(int x, int y, int ancho, int alto, float horizontal, float vertical) {
        float[] p = escalar(x, y, ancho, alto);
        MotionEvent.PointerProperties[] props = {new MotionEvent.PointerProperties()};
        props[0].id = 0;
        props[0].toolType = MotionEvent.TOOL_TYPE_MOUSE;
        MotionEvent.PointerCoords[] coords = {new MotionEvent.PointerCoords()};
        coords[0].x = p[0];
        coords[0].y = p[1];
        coords[0].setAxisValue(MotionEvent.AXIS_HSCROLL, horizontal);
        coords[0].setAxisValue(MotionEvent.AXIS_VSCROLL, vertical);
        long ahora = SystemClock.uptimeMillis();
        MotionEvent e = MotionEvent.obtain(ahora, ahora, MotionEvent.ACTION_SCROLL, 1, props, coords, 0, 0, 1f, 1f, 0, 0,
            InputDevice.SOURCE_MOUSE, 0);
        inyectar(e);
        e.recycle();
    }

    private static volatile Pantalla pantalla;
    private static long medida;

    /** Pasa un punto de la pantalla de ZYKRO (ancho × alto) a la de Android. */
    static float[] escalar(int x, int y, int ancho, int alto) {
        long ahora = SystemClock.uptimeMillis();
        if (pantalla == null || ahora - medida > 2000) {
            try {
                pantalla = Pantalla.actual();
            } catch (Exception e) {
                // se usa la anterior
            }
            medida = ahora;
        }
        Pantalla p = pantalla;
        if (p == null || ancho <= 0 || alto <= 0) {
            return new float[] {x, y};
        }
        return new float[] {x * (float) p.ancho / ancho, y * (float) p.alto / alto};
    }

    /** Los dedos que están tocando, para armar los toques con varios dedos a la vez. */
    private static final class Dedos {
        private final long[] ids = new long[MAX_DEDOS];
        private final MotionEvent.PointerProperties[] props = new MotionEvent.PointerProperties[MAX_DEDOS];
        private final MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[MAX_DEDOS];
        private int cuantos;
        private long abajo;

        Dedos() {
            for (int i = 0; i < MAX_DEDOS; i++) {
                props[i] = new MotionEvent.PointerProperties();
                props[i].toolType = MotionEvent.TOOL_TYPE_FINGER;
                coords[i] = new MotionEvent.PointerCoords();
                coords[i].size = 0.01f;
            }
        }

        private int buscar(long dedo) {
            for (int i = 0; i < cuantos; i++) {
                if (ids[i] == dedo) {
                    return i;
                }
            }
            return -1;
        }

        /** El id de Android más chico que no está en uso (los ids de ZYKRO son números grandes). */
        private int idLibre() {
            for (int id = 0; ; id++) {
                boolean usado = false;
                for (int i = 0; i < cuantos; i++) {
                    usado |= props[i].id == id;
                }
                if (!usado) {
                    return id;
                }
            }
        }

        void toque(int accion, long dedo, int x, int y, int ancho, int alto, float presion) {
            int i = buscar(dedo);
            long ahora = SystemClock.uptimeMillis();
            int evento;
            if (accion == MotionEvent.ACTION_DOWN && i >= 0) {
                // Ya estaba abajo: es un movimiento.
                evento = MotionEvent.ACTION_MOVE;
            } else if (accion == MotionEvent.ACTION_DOWN) {
                if (cuantos == MAX_DEDOS) {
                    return;
                }
                i = cuantos;
                ids[i] = dedo;
                props[i].id = idLibre();
                cuantos++;
                if (cuantos == 1) {
                    abajo = ahora;
                    evento = MotionEvent.ACTION_DOWN;
                } else {
                    evento = MotionEvent.ACTION_POINTER_DOWN | (i << MotionEvent.ACTION_POINTER_INDEX_SHIFT);
                }
            } else if (i < 0) {
                return;
            } else if (accion == MotionEvent.ACTION_UP) {
                evento = cuantos == 1 ? MotionEvent.ACTION_UP : MotionEvent.ACTION_POINTER_UP | (i << MotionEvent.ACTION_POINTER_INDEX_SHIFT);
            } else {
                evento = MotionEvent.ACTION_MOVE;
            }
            float[] p = escalar(x, y, ancho, alto);
            coords[i].x = p[0];
            coords[i].y = p[1];
            coords[i].pressure = accion == MotionEvent.ACTION_UP ? 0f : Math.max(presion, 0.01f);
            enviar(evento, ahora);
            if (accion == MotionEvent.ACTION_UP) {
                quitar(i);
            }
        }

        private void enviar(int evento, long ahora) {
            MotionEvent e = MotionEvent.obtain(abajo, ahora, evento, cuantos, props, coords, 0, 0, 1f, 1f, 0, 0,
                InputDevice.SOURCE_TOUCHSCREEN, 0);
            inyectar(e);
            e.recycle();
        }

        private void quitar(int i) {
            cuantos--;
            for (int j = i; j < cuantos; j++) {
                ids[j] = ids[j + 1];
                MotionEvent.PointerProperties pp = props[j];
                props[j] = props[j + 1];
                props[j + 1] = pp;
                MotionEvent.PointerCoords pc = coords[j];
                coords[j] = coords[j + 1];
                coords[j + 1] = pc;
            }
        }

        /** ZYKRO se desconectó con dedos abajo: se levantan para que Android no los deje pegados. */
        void soltarTodos() {
            while (cuantos > 0) {
                int i = cuantos - 1;
                coords[i].pressure = 0f;
                long ahora = SystemClock.uptimeMillis();
                enviar(cuantos == 1 ? MotionEvent.ACTION_UP : MotionEvent.ACTION_POINTER_UP | (i << MotionEvent.ACTION_POINTER_INDEX_SHIFT), ahora);
                cuantos--;
            }
        }
    }
}
