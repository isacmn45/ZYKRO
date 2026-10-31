package com.zykro.servidor;

import android.graphics.Rect;
import android.os.IBinder;
import android.view.Surface;
import java.lang.reflect.Method;

/**
 * La pantalla de Android y su copia virtual. Con APIs ocultas (como scrcpy): se crea una pantalla
 * virtual que muestra lo mismo que la de verdad (la misma pila de capas) y dibuja en `Surface`.
 */
final class Pantalla {
    final int ancho;
    final int alto;
    final int rotacion;
    final int capas;

    private Pantalla(int ancho, int alto, int rotacion, int capas) {
        this.ancho = ancho;
        this.alto = alto;
        this.rotacion = rotacion;
        this.capas = capas;
    }

    /** Tamaño, giro y pila de capas de la pantalla principal. */
    static Pantalla actual() throws Exception {
        Class<?> global = Class.forName("android.hardware.display.DisplayManagerGlobal");
        Object dm = global.getMethod("getInstance").invoke(null);
        Object info = global.getMethod("getDisplayInfo", int.class).invoke(dm, 0);
        Class<?> c = info.getClass();
        return new Pantalla(c.getField("logicalWidth").getInt(info), c.getField("logicalHeight").getInt(info),
            c.getField("rotation").getInt(info), c.getField("layerStack").getInt(info));
    }

    private static Class<?> sc() throws ClassNotFoundException {
        return Class.forName("android.view.SurfaceControl");
    }

    /** Crea la copia virtual de la pantalla, del tamaño de esta, dibujando en `destino`. */
    IBinder copiar(Surface destino) throws Exception {
        Class<?> sc = sc();
        Method crear = sc.getMethod("createDisplay", String.class, boolean.class);
        IBinder token;
        try {
            // «Segura» para que se vean también las ventanas protegidas (como en el teléfono).
            token = (IBinder) crear.invoke(null, "zykro", true);
        } catch (Exception e) {
            token = (IBinder) crear.invoke(null, "zykro", false);
        }
        Rect rect = new Rect(0, 0, ancho, alto);
        sc.getMethod("openTransaction").invoke(null);
        try {
            sc.getMethod("setDisplaySurface", IBinder.class, Surface.class).invoke(null, token, destino);
            sc.getMethod("setDisplayProjection", IBinder.class, int.class, Rect.class, Rect.class).invoke(null, token, 0, rect, rect);
            sc.getMethod("setDisplayLayerStack", IBinder.class, int.class).invoke(null, token, capas);
        } finally {
            sc.getMethod("closeTransaction").invoke(null);
        }
        return token;
    }

    static void destruir(IBinder token) {
        try {
            sc().getMethod("destroyDisplay", IBinder.class).invoke(null, token);
        } catch (Exception e) {
            // Android ya la quitó
        }
    }
}
