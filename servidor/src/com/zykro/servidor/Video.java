package com.zykro.servidor;

import android.graphics.PixelFormat;
import android.hardware.HardwareBuffer;
import android.media.Image;
import android.media.ImageReader;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Build;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.IBinder;
import android.os.Parcel;
import android.os.ParcelFileDescriptor;
import android.os.Process;
import android.hardware.SyncFence;
import android.system.Os;
import android.system.OsConstants;
import android.util.Log;
import java.io.FileDescriptor;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayDeque;
import java.util.HashMap;

/**
 * La pantalla, sin comprimir ni copiar: Android dibuja una copia de su pantalla en buffers de la GPU
 * (DMA-BUF) y ZYKRO los muestra directo. Socket `video.sock` (SOCK_SEQPACKET: cada mensaje llega
 * entero y solo). Enteros en big-endian:
 *
 * <pre>
 * Al conectar: 0xFD "DMB2" ancho alto formato 0                       (21 bytes)
 * Buffer nuevo: "DMBR" id ancho alto stride tamaño formato ancho alto
 *               modificador(u64) 0, con el fd del dma-buf             (45 bytes)
 * Mostrar:      "DMBX" id número-de-cuadro                             (12 bytes)
 * </pre>
 *
 * Un buffer se vuelve a usar unos cuadros después (se retienen los últimos {@link #RETENIDAS}), así
 * que ZYKRO alcanza a mostrarlo. Atiende a un cliente a la vez: uno nuevo reemplaza al anterior.
 */
final class Video {
    /** Buffers de la copia de la pantalla. */
    private static final int IMAGENES = 6;
    /** Cuadros que se guardan mientras ZYKRO los muestra (el actual y dos en su cola). */
    private static final int RETENIDAS = 3;
    private static final int GB01 = 0x47423031;
    private static final int GBFR = 0x47424652;
    /** Marca de los buffers de gbm_gralloc (el gralloc de redroid). */
    private static final int GRALLOC_GBM = 0x60585350;
    /** DRM_FORMAT_MOD_INVALID: gbm no lo dijo; con uso de CPU el buffer es lineal. */
    private static final long MODIFICADOR_INVALIDO = 0x00ffffffffffffffL;

    private static Sesion actual;

    private Video() {}

    static void servir(String ruta) throws Exception {
        LocalServerSocket servidor = Servidor.escuchar(ruta, LocalSocket.SOCKET_SEQPACKET);
        while (true) {
            LocalSocket cliente = servidor.accept();
            Sesion sesion = new Sesion(cliente);
            synchronized (Video.class) {
                if (actual != null) {
                    actual.terminar();
                }
                actual = sesion;
            }
            try {
                sesion.iniciar();
            } catch (Exception e) {
                Log.e(Servidor.ETIQUETA, "video: no se pudo copiar la pantalla", e);
                sesion.terminar();
            }
        }
    }

    private static final class Sesion implements ImageReader.OnImageAvailableListener {
        private final LocalSocket cliente;
        private final OutputStream salida;
        private final HandlerThread hilo = new HandlerThread("zykro-cuadros", Process.THREAD_PRIORITY_DISPLAY);
        private Handler handler;
        private ImageReader lector;
        private IBinder copia;
        private final ArrayDeque<Image> retenidas = new ArrayDeque<>();
        /** Id del buffer de Android -> el número que se le dio a ZYKRO. */
        private final HashMap<Long, Integer> registrados = new HashMap<>();
        private int cuadros;
        private boolean liberada;
        private volatile boolean terminada;

        Sesion(LocalSocket cliente) throws IOException {
            this.cliente = cliente;
            salida = cliente.getOutputStream();
        }

        void iniciar() throws Exception {
            Pantalla p = Pantalla.actual();
            hilo.start();
            handler = new Handler(hilo.getLooper());
            // Buffers que la CPU también puede leer: así son lineales y ZYKRO puede leerlos (capturas).
            lector = Build.VERSION.SDK_INT >= 29
                ? ImageReader.newInstance(p.ancho, p.alto, PixelFormat.RGBA_8888, IMAGENES,
                    HardwareBuffer.USAGE_CPU_READ_OFTEN | HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE)
                : ImageReader.newInstance(p.ancho, p.alto, PixelFormat.RGBA_8888, IMAGENES);
            lector.setOnImageAvailableListener(this, handler);
            ByteBuffer cabecera = ByteBuffer.allocate(21);
            cabecera.put((byte) 0xFD).put("DMB2".getBytes(StandardCharsets.US_ASCII)).putInt(p.ancho).putInt(p.alto).putInt(1).putInt(0);
            salida.write(cabecera.array());
            copia = p.copiar(lector.getSurface());
            new Thread(this::esperarCierre, "zykro-video-cierre").start();
            Log.i(Servidor.ETIQUETA, "video: cliente conectado, " + p.ancho + "x" + p.alto);
        }

        /** ZYKRO nunca escribe aquí: leer solo sirve para notar que cerró. */
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
            terminar();
        }

        void terminar() {
            if (terminada) {
                return;
            }
            terminada = true;
            try {
                // Despierta al hilo que espera leyendo (cerrar el fd no lo haría).
                cliente.shutdownInput();
            } catch (IOException e) {
                // ya estaba cerrado
            }
            if (handler != null) {
                handler.post(this::liberar);
            } else {
                Servidor.cerrar(cliente);
            }
        }

        private void liberar() {
            if (liberada) {
                return;
            }
            liberada = true;
            if (copia != null) {
                Pantalla.destruir(copia);
            }
            for (Image imagen : retenidas) {
                imagen.close();
            }
            retenidas.clear();
            if (lector != null) {
                lector.close();
            }
            Servidor.cerrar(cliente);
            hilo.quitSafely();
            Log.i(Servidor.ETIQUETA, "video: cliente desconectado");
        }

        @Override
        public void onImageAvailable(ImageReader r) {
            if (terminada) {
                return;
            }
            Image imagen;
            try {
                imagen = r.acquireLatestImage();
            } catch (IllegalStateException e) {
                // Sin buffers libres: se suelta el más viejo y llegará el siguiente.
                if (!retenidas.isEmpty()) {
                    retenidas.removeFirst().close();
                }
                return;
            }
            if (imagen == null) {
                return;
            }
            try {
                esperarDibujo(imagen);
                mostrar(imagen);
            } catch (IOException e) {
                imagen.close();
                terminar();
                return;
            } catch (RuntimeException e) {
                Log.w(Servidor.ETIQUETA, "video: cuadro perdido", e);
            }
            retenidas.addLast(imagen);
            while (retenidas.size() > RETENIDAS) {
                retenidas.removeFirst().close();
            }
        }

        /** Espera a que la GPU termine de dibujar el cuadro, así ZYKRO no muestra uno a medias. */
        private void esperarDibujo(Image imagen) {
            if (Build.VERSION.SDK_INT >= 33) {
                SyncFence fence;
                try {
                    fence = imagen.getFence();
                } catch (IOException e) {
                    return;
                }
                try {
                    if (fence.isValid()) {
                        fence.await(Duration.ofMillis(50));
                    }
                } finally {
                    try {
                        fence.close();
                    } catch (Exception e) {
                        // nada
                    }
                }
            } else {
                // Sin getFence: bloquear el buffer para la CPU espera a que la GPU lo suelte.
                imagen.getPlanes();
            }
        }

        private void mostrar(Image imagen) throws IOException {
            HardwareBuffer buffer = imagen.getHardwareBuffer();
            if (buffer == null) {
                return;
            }
            int id;
            try {
                id = registrar(buffer);
            } finally {
                buffer.close();
            }
            ByteBuffer m = ByteBuffer.allocate(12);
            m.put("DMBX".getBytes(StandardCharsets.US_ASCII)).putInt(id).putInt(++cuadros);
            salida.write(m.array());
        }

        private int registrar(HardwareBuffer buffer) throws IOException {
            // Android 13 todavía no expone HardwareBuffer.getId() en Java. Del Parcel solo hace
            // falta leer el identificador cuando el cliente ya tiene el descriptor del buffer.
            Parcel p = Parcel.obtain();
            try {
                buffer.writeToParcel(p, 0);
                p.setDataPosition(0);
                int largo = p.readInt() / Integer.BYTES;
                int nfds = p.readInt();
                if (nfds <= 0 || largo < 11) {
                    throw new IOException("buffer sin dma-buf");
                }
                int inicio = p.dataPosition();
                int tipo = p.readInt();
                int indice;
                if (tipo == GB01) {
                    indice = 7;
                } else if (tipo == GBFR) {
                    indice = 6;
                } else {
                    throw new IOException("formato de buffer desconocido " + Integer.toHexString(tipo));
                }
                p.setDataPosition(inicio + indice * Integer.BYTES);
                long clave = ((long) p.readInt() << 32) | (p.readInt() & 0xffffffffL);
                Integer conocido = registrados.get(clave);
                if (conocido != null) {
                    return conocido;
                }

                p.setDataPosition(inicio);
                Descripcion d = Descripcion.leer(p, largo, nfds, tipo == GB01 ? 13 : 11);
                int id = registrados.size() + 1;
                try {
                    ByteBuffer m = ByteBuffer.allocate(45);
                    m.put("DMBR".getBytes(StandardCharsets.US_ASCII)).putInt(id).putInt(d.ancho).putInt(d.alto).putInt(d.stride)
                        .putInt(d.tamano).putInt(d.formato).putInt(d.ancho).putInt(d.alto).putLong(d.modificador).put((byte) 0);
                    cliente.setFileDescriptorsForSend(new FileDescriptor[] {d.fds[0].getFileDescriptor()});
                    try {
                        salida.write(m.array());
                    } finally {
                        cliente.setFileDescriptorsForSend(null);
                    }
                    registrados.put(clave, id);
                    Log.i(Servidor.ETIQUETA, "video: buffer " + id + " " + d.ancho + "x" + d.alto + " stride " + d.stride
                        + " tamaño " + d.tamano + " modificador " + d.modificador);
                    return id;
                } finally {
                    d.cerrar();
                }
            } finally {
                p.recycle();
            }
        }
    }

    /** Lo que ZYKRO necesita de un buffer, sacado de cómo Android lo pasa entre procesos. */
    private static final class Descripcion {
        int ancho;
        int alto;
        int stride;
        int formato;
        int tamano;
        long modificador;
        ParcelFileDescriptor[] fds;

        /**
         * Un HardwareBuffer se escribe en un Parcel como su GraphicBuffer: largo, cantidad de fds,
         * los enteros de GraphicBuffer::flatten y los fds (copias). Así se obtiene el dma-buf sin NDK.
         */
        static Descripcion leer(Parcel p, int largo, int nfds, int enteros) {
            Descripcion d = new Descripcion();
            try {
                int[] g = new int[largo];
                for (int i = 0; i < g.length; i++) {
                    g[i] = p.readInt();
                }
                d.fds = new ParcelFileDescriptor[nfds];
                for (int i = 0; i < nfds; i++) {
                    d.fds[i] = p.readFileDescriptor();
                }
                d.ancho = g[1];
                d.alto = g[2];
                d.stride = g[3] * 4;
                d.formato = g[4];
                if (g.length > enteros + 9 && g[enteros] == GRALLOC_GBM) {
                    d.stride = g[enteros + 6];
                    d.modificador = (g[enteros + 8] & 0xffffffffL) | ((long) g[enteros + 9] << 32);
                    if (d.modificador == MODIFICADOR_INVALIDO) {
                        d.modificador = 0;
                    }
                }
                try {
                    d.tamano = (int) Os.lseek(d.fds[0].getFileDescriptor(), 0, OsConstants.SEEK_END);
                } catch (Exception e) {
                    d.tamano = d.stride * d.alto;
                }
                return d;
            } catch (RuntimeException e) {
                d.cerrar();
                throw e;
            }
        }

        void cerrar() {
            if (fds == null) {
                return;
            }
            for (ParcelFileDescriptor f : fds) {
                Servidor.cerrar(f);
            }
        }
    }
}
