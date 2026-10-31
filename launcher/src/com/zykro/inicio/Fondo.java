package com.zykro.inicio;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Matrix;
import android.graphics.Paint;
import android.media.ExifInterface;
import android.util.DisplayMetrics;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/**
 * El fondo de pantalla de la pantalla de inicio. Lo elige ZYKRO (sus fondos, una imagen de la laptop)
 * o {@link ElegirFondo} (la galería de Android); aquí se ajusta a la pantalla y se guarda en la
 * carpeta de esta app. {@link Inicio} lo dibuja detrás de los íconos: sin el servicio de fondos de
 * Android, que dibujaría una capa más en cada cuadro.
 */
final class Fondo {
    static final String IMAGEN = "fondo.png";
    /** Una miniatura, para la ventana de fondos de ZYKRO. */
    static final String MINIATURA = "fondo-mini.png";
    /** El nombre del fondo de ZYKRO elegido (vacío si es una imagen propia). */
    static final String NOMBRE = "fondo-nombre";
    private static final int ANCHO_MINIATURA = 256;

    /** De dónde se lee la imagen (se abre más de una vez: primero solo para medirla). */
    interface Origen {
        InputStream abrir() throws IOException;
    }

    private Fondo() {}

    static File archivo(Context contexto, String nombre) {
        File carpeta = contexto.getExternalFilesDir(null);
        return carpeta == null ? null : new File(carpeta, nombre);
    }

    /** Pone de fondo la imagen de `origen` (PNG, JPEG, WebP…), recortada para cubrir la pantalla. */
    static boolean poner(Context contexto, Origen origen, String nombre) {
        DisplayMetrics m = contexto.getResources().getDisplayMetrics();
        int ancho = m.widthPixels;
        int alto = m.heightPixels;
        File imagen = archivo(contexto, IMAGEN);
        if (imagen == null) {
            return false;
        }
        try {
            BitmapFactory.Options medidas = new BitmapFactory.Options();
            medidas.inJustDecodeBounds = true;
            try (InputStream entrada = origen.abrir()) {
                BitmapFactory.decodeStream(entrada, null, medidas);
            }
            if (medidas.outWidth <= 0 || medidas.outHeight <= 0) {
                return false;
            }
            // Una foto enorme se lee a la mitad, la cuarta parte…, mientras siga cubriendo la pantalla.
            BitmapFactory.Options opciones = new BitmapFactory.Options();
            opciones.inSampleSize = 1;
            while (medidas.outWidth / (opciones.inSampleSize * 2) >= ancho && medidas.outHeight / (opciones.inSampleSize * 2) >= alto) {
                opciones.inSampleSize *= 2;
            }
            Bitmap leida;
            try (InputStream entrada = origen.abrir()) {
                leida = BitmapFactory.decodeStream(entrada, null, opciones);
            }
            if (leida == null) {
                return false;
            }
            int grados = girar(origen);
            Bitmap ajustada = grados == 0 && leida.getWidth() == ancho && leida.getHeight() == alto
                ? leida : cubrir(leida, grados, ancho, alto);
            mostrar(ajustada);
            guardar(ajustada, imagen);
            guardar(cubrir(ajustada, 0, ANCHO_MINIATURA, Math.round(ANCHO_MINIATURA * alto / (float) ancho)), archivo(contexto, MINIATURA));
            try (FileOutputStream salida = new FileOutputStream(archivo(contexto, NOMBRE))) {
                salida.write(nombre.getBytes(StandardCharsets.UTF_8));
            }
        } catch (IOException | RuntimeException | OutOfMemoryError e) {
            Log.w("ZYKRO", "no se pudo poner el fondo", e);
            Inicio inicio = Inicio.activa;
            if (inicio != null) {
                inicio.runOnUiThread(inicio::cargarFondo);
            }
            return false;
        }
        avisar();
        return true;
    }

    /** Vuelve al fondo de ZYKRO de siempre. */
    static void quitar(Context contexto) {
        for (String nombre : new String[] {IMAGEN, MINIATURA, NOMBRE}) {
            File f = archivo(contexto, nombre);
            if (f != null) {
                f.delete();
            }
        }
        mostrar(null);
        avisar();
    }

    /** Los grados que hay que girar una foto de celular (la cámara lo anota en la foto). */
    private static int girar(Origen origen) {
        try (InputStream entrada = origen.abrir()) {
            switch (new ExifInterface(entrada).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)) {
                case ExifInterface.ORIENTATION_ROTATE_90:
                    return 90;
                case ExifInterface.ORIENTATION_ROTATE_180:
                    return 180;
                case ExifInterface.ORIENTATION_ROTATE_270:
                    return 270;
                default:
                    return 0;
            }
        } catch (IOException | RuntimeException e) {
            return 0;
        }
    }

    /** Gira `b` y lo escala y recorta al centro para que cubra `ancho` × `alto`. */
    private static Bitmap cubrir(Bitmap b, int grados, int ancho, int alto) {
        boolean acostada = grados == 90 || grados == 270;
        float w = acostada ? b.getHeight() : b.getWidth();
        float h = acostada ? b.getWidth() : b.getHeight();
        float escala = Math.max(ancho / w, alto / h);
        Matrix matriz = new Matrix();
        matriz.postTranslate(-b.getWidth() / 2f, -b.getHeight() / 2f);
        matriz.postRotate(grados);
        matriz.postScale(escala, escala);
        matriz.postTranslate(ancho / 2f, alto / 2f);
        Bitmap salida = Bitmap.createBitmap(ancho, alto, Bitmap.Config.ARGB_8888);
        new Canvas(salida).drawBitmap(b, matriz, new Paint(Paint.FILTER_BITMAP_FLAG));
        return salida;
    }

    private static void guardar(Bitmap b, File destino) throws IOException {
        File temporal = new File(destino.getPath() + ".tmp");
        try (FileOutputStream salida = new FileOutputStream(temporal)) {
            b.compress(Bitmap.CompressFormat.PNG, 100, salida);
        }
        if (!temporal.renameTo(destino)) {
            throw new IOException("no se pudo guardar " + destino);
        }
    }

    /** La pantalla de inicio lo vuelve a dibujar y ZYKRO actualiza su ventana de fondos. */
    private static void mostrar(Bitmap imagen) {
        Inicio inicio = Inicio.activa;
        if (inicio != null) {
            inicio.runOnUiThread(() -> inicio.mostrarFondo(imagen));
        }
    }

    private static void avisar() {
        Log.i("ZYKRO", "fondo");
    }
}
