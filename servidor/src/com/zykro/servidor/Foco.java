package com.zykro.servidor;

import android.os.IBinder;
import android.os.ParcelFileDescriptor;
import android.system.Os;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/**
 * Si un juego abrió su cuadro de texto (el chat de Unity es una ventana que pide el teclado siempre
 * visible), en el archivo `texto` (1 o 0): ZYKRO apaga el mapeo para poder escribir. Lo dice el
 * gestor de teclados de Android en su volcado (como `dumpsys input_method`), que se lee cada 400 ms.
 */
final class Foco {
    private Foco() {}

    static void vigilar(String ruta) throws Exception {
        IBinder teclados = (IBinder) Class.forName("android.os.ServiceManager").getMethod("getService", String.class)
            .invoke(null, "input_method");
        String anterior = null;
        while (true) {
            String estado = escribiendo(volcado(teclados)) ? "1\n" : "0\n";
            if (!estado.equals(anterior)) {
                escribir(ruta, estado);
                anterior = estado;
            }
            Servidor.dormir(400);
        }
    }

    /** «mCurFocusedWindow=… softInputMode=STATE_ALWAYS_VISIBLE|ADJUST_RESIZE …» */
    static boolean escribiendo(String volcado) {
        for (String linea : volcado.split("\n")) {
            if (linea.contains("mCurFocusedWindow=")) {
                return linea.contains("STATE_ALWAYS_VISIBLE");
            }
        }
        return false;
    }

    private static String volcado(IBinder servicio) throws Exception {
        ParcelFileDescriptor[] tubo = ParcelFileDescriptor.createPipe();
        ByteArrayOutputStream texto = new ByteArrayOutputStream();
        // Se lee mientras Android escribe: si el volcado no cupiera en la tubería, se trabaría.
        Thread lector = new Thread(() -> {
            try (InputStream entrada = new ParcelFileDescriptor.AutoCloseInputStream(tubo[0])) {
                byte[] bloque = new byte[8192];
                int n;
                while ((n = entrada.read(bloque)) > 0) {
                    texto.write(bloque, 0, n);
                }
            } catch (Exception e) {
                // se cerró
            }
        }, "zykro-foco-lector");
        lector.start();
        try {
            servicio.dump(tubo[1].getFileDescriptor(), new String[0]);
        } finally {
            tubo[1].close();
        }
        lector.join(2000);
        return texto.toString(StandardCharsets.UTF_8.name());
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
