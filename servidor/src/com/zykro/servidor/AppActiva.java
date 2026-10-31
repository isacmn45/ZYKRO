package com.zykro.servidor;

import android.app.ActivityManager;
import android.system.Os;
import java.io.File;
import java.io.FileOutputStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.List;

/**
 * La app en primer plano, en el archivo `active_app` (`package=…`): ZYKRO lo lee para elegir el
 * mapeo de teclas. Se revisa dos veces por segundo y el archivo solo se escribe si cambió.
 */
final class AppActiva {
    private AppActiva() {}

    static void vigilar(String ruta) throws Exception {
        Object am = ActivityManager.class.getMethod("getService").invoke(null);
        Method tareas = am.getClass().getMethod("getTasks", int.class);
        String anterior = null;
        while (true) {
            List<?> lista = (List<?>) tareas.invoke(am, 1);
            String paquete = null;
            if (lista != null && !lista.isEmpty()) {
                ActivityManager.RunningTaskInfo tarea = (ActivityManager.RunningTaskInfo) lista.get(0);
                if (tarea.topActivity != null) {
                    paquete = tarea.topActivity.getPackageName();
                }
            }
            if (paquete != null && !paquete.equals(anterior)) {
                escribir(ruta, "package=" + paquete + "\n");
                anterior = paquete;
            }
            Servidor.dormir(500);
        }
    }

    private static void escribir(String ruta, String texto) throws Exception {
        File temporal = new File(ruta + ".tmp");
        try (FileOutputStream salida = new FileOutputStream(temporal)) {
            salida.write(texto.getBytes(StandardCharsets.UTF_8));
        }
        Os.chmod(temporal.getPath(), 0644);
        if (!temporal.renameTo(new File(ruta))) {
            throw new Exception("no se pudo escribir " + ruta);
        }
    }
}
