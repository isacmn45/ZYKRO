package com.zykro.servidor;

import android.app.Application;
import android.content.AttributionSource;
import android.content.Context;
import android.content.ContextWrapper;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import android.os.Process;
import android.util.Log;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

/**
 * app_process no es una app: no tiene contexto ni nombre de paquete, y AudioRecord los pide para
 * anotar quién graba. Se arma un ActivityThread con una Application que envuelve el contexto de
 * sistema (como hace scrcpy): así lo que lo consulta (AppOpsManager en Android 13, por ejemplo)
 * encuentra un PackageManager de verdad. Corriendo como root («root» es su nombre de paquete para
 * Android), no se revisan más permisos para capturar el sonido.
 */
final class Contexto {
    static final String PAQUETE = "root";

    private Contexto() {}

    static void preparar() {
        try {
            Class<?> clase = Class.forName("android.app.ActivityThread");
            Constructor<?> constructor = clase.getDeclaredConstructor();
            constructor.setAccessible(true);
            Object hilo = constructor.newInstance();
            campo(clase, "sCurrentActivityThread").set(null, hilo);
            campo(clase, "mSystemThread").setBoolean(hilo, true);
            Method sistema = clase.getDeclaredMethod("getSystemContext");
            sistema.setAccessible(true);
            Application aplicacion = new Application();
            Method adjuntar = ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
            adjuntar.setAccessible(true);
            adjuntar.invoke(aplicacion, new Falso((Context) sistema.invoke(hilo)));
            campo(clase, "mInitialApplication").set(hilo, aplicacion);
        } catch (Throwable e) {
            Log.w(Servidor.ETIQUETA, "no se pudo preparar el contexto", e);
        }
    }

    private static Field campo(Class<?> clase, String nombre) throws NoSuchFieldException {
        Field f = clase.getDeclaredField(nombre);
        f.setAccessible(true);
        return f;
    }

    /** Graba lo que suena en Android (no el micrófono): REMOTE_SUBMIX, PCM de 16 bits en estéreo. */
    static AudioRecord grabadora(int frecuencia, int bufer) {
        AudioFormat formato = new AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
            .setSampleRate(frecuencia)
            .setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
            .build();
        int minimo = AudioRecord.getMinBufferSize(frecuencia, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT);
        AudioRecord grabadora = new AudioRecord.Builder()
            .setAudioSource(MediaRecorder.AudioSource.REMOTE_SUBMIX)
            .setAudioFormat(formato)
            .setBufferSizeInBytes(Math.max(minimo, bufer))
            .build();
        if (grabadora.getState() != AudioRecord.STATE_INITIALIZED) {
            grabadora.release();
            throw new IllegalStateException("AudioRecord no se inició");
        }
        return grabadora;
    }

    /** El contexto de sistema, pero con el nombre de paquete de root (lo que AudioRecord anota). */
    private static final class Falso extends ContextWrapper {
        Falso(Context sistema) {
            super(sistema);
        }

        @Override
        public String getPackageName() {
            return PAQUETE;
        }

        // Oculto en Context (no está en android.jar), pero lo sobrescribe igual.
        public String getOpPackageName() {
            return PAQUETE;
        }

        @Override
        public Context getApplicationContext() {
            return this;
        }

        // Android 12 en adelante: AudioRecord toma de aquí quién graba.
        @Override
        public AttributionSource getAttributionSource() {
            return new AttributionSource.Builder(Process.myUid()).setPackageName(PAQUETE).build();
        }
    }
}
