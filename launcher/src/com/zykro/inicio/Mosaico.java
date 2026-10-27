package com.zykro.inicio;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;

/** Acomoda los íconos de izquierda a derecha y pasa a la fila siguiente cuando no caben. */
class Mosaico extends ViewGroup {
    private final int espacio;

    Mosaico(Context contexto, int espacio) {
        super(contexto);
        this.espacio = espacio;
    }

    @Override
    protected void onMeasure(int anchoPedido, int altoPedido) {
        int ancho = MeasureSpec.getSize(anchoPedido);
        int libre = MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED);
        int x = 0, y = 0, altoFila = 0;
        for (int i = 0; i < getChildCount(); i++) {
            View hijo = getChildAt(i);
            hijo.measure(libre, libre);
            int w = hijo.getMeasuredWidth(), h = hijo.getMeasuredHeight();
            if (x > 0 && x + w > ancho) {
                x = 0;
                y += altoFila + espacio;
                altoFila = 0;
            }
            x += w + espacio;
            altoFila = Math.max(altoFila, h);
        }
        setMeasuredDimension(ancho, getChildCount() == 0 ? 0 : y + altoFila);
    }

    @Override
    protected void onLayout(boolean cambio, int l, int t, int r, int b) {
        int ancho = r - l;
        int x = 0, y = 0, altoFila = 0;
        for (int i = 0; i < getChildCount(); i++) {
            View hijo = getChildAt(i);
            int w = hijo.getMeasuredWidth(), h = hijo.getMeasuredHeight();
            if (x > 0 && x + w > ancho) {
                x = 0;
                y += altoFila + espacio;
                altoFila = 0;
            }
            hijo.layout(x, y, x + w, y + h);
            x += w + espacio;
            altoFila = Math.max(altoFila, h);
        }
    }
}
