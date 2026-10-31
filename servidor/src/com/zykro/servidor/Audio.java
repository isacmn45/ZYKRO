package com.zykro.servidor;

import android.media.AudioRecord;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Process;
import android.os.SharedMemory;
import android.util.Log;
import java.io.FileDescriptor;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;

/**
 * El sonido de Android (lo que suena, capturado con REMOTE_SUBMIX) en un anillo de memoria
 * compartida que ZYKRO lee sin pasar por el socket (`audio.sock`):
 *
 * <pre>
 * Al conectar: 0xFC con el fd del anillo, y "ASHM" frecuencia canales bytes-por-cuadro tamaño (big-endian)
 * Anillo (little-endian): 0x00 "ASHM", 0x04 frecuencia, 0x08 canales, 0x0c bytes por cuadro,
 *   0x10 capacidad, 0x14 dónde empieza el último bloque, 0x18 largo de bloque, 0x1c contador (sube de
 *   2 en 2 por bloque; impar mientras se escribe). En 0x40, 32 entradas {contador, desplazamiento,
 *   largo, 0}: la del bloque n en 0x40 + (n &amp; 0x3e) * 8. PCM s16 intercalado desde 0x400.
 * </pre>
 *
 * Solo graba mientras hay un cliente: con la captura activa, Android manda el sonido a ella.
 */
final class Audio {
    private static final int FRECUENCIA = 48000;
    private static final int CANALES = 2;
    private static final int BYTES_CUADRO = 4;
    /** 20 ms. */
    private static final int BLOQUE = 3840;
    private static final int CABECERA = 0x400;
    private static final int ENTRADAS = 0x40;
    private static final int CAPACIDAD = 65536;

    private static Sesion actual;

    private Audio() {}

    static void servir(String ruta) throws Exception {
        LocalServerSocket servidor = Servidor.escuchar(ruta, LocalSocket.SOCKET_STREAM);
        while (true) {
            LocalSocket cliente = servidor.accept();
            Sesion sesion = new Sesion(cliente);
            synchronized (Audio.class) {
                if (actual != null) {
                    actual.terminar();
                }
                actual = sesion;
            }
            new Thread(sesion::correr, "zykro-audio-sesion").start();
        }
    }

    private static final class Sesion {
        private final LocalSocket cliente;
        private volatile boolean terminada;

        Sesion(LocalSocket cliente) {
            this.cliente = cliente;
        }

        void terminar() {
            terminada = true;
            try {
                cliente.shutdownInput();
            } catch (IOException e) {
                // ya estaba cerrado
            }
        }

        void correr() {
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO);
            SharedMemory memoria = null;
            AudioRecord grabadora = null;
            try {
                memoria = SharedMemory.create("zykro-audio", CABECERA + CAPACIDAD);
                ByteBuffer anillo = memoria.mapReadWrite().order(ByteOrder.LITTLE_ENDIAN);
                anillo.put("ASHM".getBytes(StandardCharsets.US_ASCII));
                anillo.putInt(0x04, FRECUENCIA).putInt(0x08, CANALES).putInt(0x0c, BYTES_CUADRO).putInt(0x10, CAPACIDAD)
                    .putInt(0x18, BLOQUE);

                FileDescriptor fd = (FileDescriptor) SharedMemory.class.getMethod("getFileDescriptor").invoke(memoria);
                OutputStream salida = cliente.getOutputStream();
                cliente.setFileDescriptorsForSend(new FileDescriptor[] {fd});
                try {
                    salida.write(0xFC);
                } finally {
                    cliente.setFileDescriptorsForSend(null);
                }
                ByteBuffer saludo = ByteBuffer.allocate(20);
                saludo.put("ASHM".getBytes(StandardCharsets.US_ASCII)).putInt(FRECUENCIA).putInt(CANALES).putInt(BYTES_CUADRO)
                    .putInt(CABECERA + CAPACIDAD);
                salida.write(saludo.array());
                new Thread(this::esperarCierre, "zykro-audio-cierre").start();

                grabadora = Contexto.grabadora(FRECUENCIA, BLOQUE * 4);
                grabadora.startRecording();
                Log.i(Servidor.ETIQUETA, "audio: cliente conectado");
                byte[] bloque = new byte[BLOQUE];
                int contador = 0;
                int posicion = 0;
                while (!terminada) {
                    int leido = 0;
                    while (leido < BLOQUE && !terminada) {
                        int n = grabadora.read(bloque, leido, BLOQUE - leido);
                        if (n < 0) {
                            throw new IOException("AudioRecord.read: " + n);
                        }
                        leido += n;
                    }
                    anillo.putInt(0x1c, contador + 1);
                    int primera = Math.min(leido, CAPACIDAD - posicion);
                    anillo.position(CABECERA + posicion);
                    anillo.put(bloque, 0, primera);
                    if (primera < leido) {
                        anillo.position(CABECERA);
                        anillo.put(bloque, primera, leido - primera);
                    }
                    int siguiente = contador + 2;
                    int entrada = ENTRADAS + (siguiente & 0x3e) * 8;
                    anillo.putInt(entrada, siguiente).putInt(entrada + 4, posicion).putInt(entrada + 8, leido).putInt(entrada + 12, 0);
                    anillo.putInt(0x14, posicion);
                    posicion = (posicion + leido) % CAPACIDAD;
                    contador = siguiente;
                    anillo.putInt(0x1c, contador);
                }
            } catch (Exception e) {
                if (!terminada) {
                    Log.w(Servidor.ETIQUETA, "audio: " + e, e);
                }
            } finally {
                if (grabadora != null) {
                    try {
                        grabadora.stop();
                    } catch (IllegalStateException e) {
                        // no había empezado
                    }
                    grabadora.release();
                }
                if (memoria != null) {
                    memoria.close();
                }
                terminar();
                Servidor.cerrar(cliente);
                Log.i(Servidor.ETIQUETA, "audio: cliente desconectado");
            }
        }

        private void esperarCierre() {
            try {
                InputStream entrada = cliente.getInputStream();
                byte[] basura = new byte[64];
                while (entrada.read(basura) >= 0) {
                    // nada
                }
            } catch (IOException e) {
                // se cerró
            }
            terminada = true;
        }
    }
}
