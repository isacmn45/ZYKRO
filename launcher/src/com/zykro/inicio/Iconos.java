package com.zykro.inicio;

import android.graphics.Bitmap;
import android.graphics.BitmapShader;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Shader;
import android.graphics.drawable.AdaptiveIconDrawable;
import android.graphics.drawable.Drawable;

/** Íconos de las apps como cuadrados de esquinas redondeadas, igual para todas (como BlueStacks). */
final class Iconos {
    private Iconos() {}

    /** Radio de las esquinas, en proporción al lado del ícono. */
    static final float REDONDEO = 0.24f;

    static Bitmap dibujar(Drawable icono, int lado) {
        Bitmap listo = Bitmap.createBitmap(lado, lado, Bitmap.Config.ARGB_8888);
        Canvas lienzo = new Canvas(listo);
        if (icono instanceof AdaptiveIconDrawable) {
            // Las capas miden 108 dp y solo se ven los 72 del centro: sobra 1/4 del lado visible por cada
            // borde. Se dibujan completas y se recortan con un rectángulo redondeado con antialias.
            AdaptiveIconDrawable adaptable = (AdaptiveIconDrawable) icono;
            Bitmap capas = Bitmap.createBitmap(lado, lado, Bitmap.Config.ARGB_8888);
            Canvas c = new Canvas(capas);
            int sobra = lado / 4;
            for (Drawable capa : new Drawable[] {adaptable.getBackground(), adaptable.getForeground()}) {
                if (capa != null) {
                    capa.setBounds(-sobra, -sobra, lado + sobra, lado + sobra);
                    capa.draw(c);
                }
            }
            Paint pintura = new Paint(Paint.ANTI_ALIAS_FLAG);
            pintura.setShader(new BitmapShader(capas, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP));
            float radio = lado * REDONDEO;
            lienzo.drawRoundRect(0, 0, lado, lado, radio, radio, pintura);
            capas.recycle();
        } else {
            // Íconos viejos (un PNG con su propia forma): se dejan como vienen.
            icono.setBounds(0, 0, lado, lado);
            icono.draw(lienzo);
        }
        return listo;
    }
}
