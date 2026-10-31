package com.zykro.servidor;

import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.net.LocalSocketAddress;
import android.os.Looper;
import android.system.ErrnoException;
import android.system.Os;
import android.util.Log;
import java.io.File;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/**
 * El servidor de ZYKRO dentro de Android. Comparte la pantalla sin copiarla ({@link Video}), recibe
 * toques y teclas ({@link Control}) y pasa el sonido ({@link Audio}) por sockets Unix en una carpeta
 * que Linux ve (en ZYKRO, /tmp/zykro-android). Ahí deja también la app al frente ({@link AppActiva})
 * y si un juego abrió su cuadro de texto ({@link Foco}). Corre como root con app_process: lo arranca
 * init al terminar de encender Android (zykro-servidor.rc).
 */
public final class Servidor {
    static final String ETIQUETA = "zykro-servidor";

    /** Los sockets recién creados: si el recolector se llevara su objeto, cerraría el fd. */
    private static final List<LocalSocket> VIVOS = new ArrayList<>();

    private Servidor() {}

    public static void main(String[] args) throws Exception {
        String carpeta = args.length > 0 ? args[0] : "/data/local/tmp/zykro";
        new File(carpeta).mkdirs();
        Looper.prepareMainLooper();
        Contexto.preparar();
        hilo("video", () -> Video.servir(carpeta + "/video.sock"));
        hilo("control", () -> Control.servir(carpeta + "/control.sock"));
        hilo("audio", () -> Audio.servir(carpeta + "/audio.sock"));
        hilo("app", () -> AppActiva.vigilar(carpeta + "/active_app"));
        hilo("foco", () -> Foco.vigilar(carpeta + "/texto"));
        Log.i(ETIQUETA, "listo en " + carpeta);
        Looper.loop();
    }

    interface Tarea {
        void correr() throws Exception;
    }

    /** Corre `tarea` en su hilo y la vuelve a empezar si falla. */
    static void hilo(String nombre, Tarea tarea) {
        new Thread(() -> {
            while (true) {
                try {
                    tarea.correr();
                } catch (Throwable e) {
                    Log.e(ETIQUETA, nombre + ": " + e, e);
                }
                dormir(1000);
            }
        }, "zykro-" + nombre).start();
    }

    /** Socket Unix en un archivo, abierto para todos (lo usa ZYKRO desde Linux con tu usuario). */
    static LocalServerSocket escuchar(String ruta, int tipo) throws IOException {
        new File(ruta).delete();
        LocalSocket socket = new LocalSocket(tipo);
        socket.bind(new LocalSocketAddress(ruta, LocalSocketAddress.Namespace.FILESYSTEM));
        synchronized (VIVOS) {
            VIVOS.add(socket);
        }
        try {
            Os.chmod(ruta, 0666);
        } catch (ErrnoException e) {
            throw new IOException(e);
        }
        return new LocalServerSocket(socket.getFileDescriptor());
    }

    static void dormir(long ms) {
        try {
            Thread.sleep(ms);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    static void cerrar(AutoCloseable c) {
        if (c == null) {
            return;
        }
        try {
            c.close();
        } catch (Exception e) {
            // ya estaba cerrado
        }
    }
}
