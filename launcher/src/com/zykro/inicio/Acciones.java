package com.zykro.inicio;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * Lo que el panel de notificaciones de ZYKRO pide (con `am broadcast` desde la shell de Android):
 * abrir una notificación, tocar uno de sus botones, quitarla o quitar todas. Solo se aceptan órdenes
 * de la shell o del sistema (permiso DUMP, ver AndroidManifest.xml).
 */
public class Acciones extends BroadcastReceiver {
    static final String ABRIR = "com.zykro.inicio.ABRIR";
    static final String ACCION = "com.zykro.inicio.ACCION";
    static final String DESCARTAR = "com.zykro.inicio.DESCARTAR";
    static final String BORRAR_TODO = "com.zykro.inicio.BORRAR_TODO";

    @Override
    public void onReceive(Context contexto, Intent intent) {
        Escucha escucha = Escucha.activa;
        String accion = intent.getAction();
        if (escucha == null || accion == null) {
            return;
        }
        String clave = intent.getStringExtra("clave");
        switch (accion) {
            case ABRIR:
                escucha.abrir(clave);
                break;
            case ACCION:
                escucha.accion(clave, intent.getIntExtra("indice", -1));
                break;
            case DESCARTAR:
                escucha.descartar(clave);
                break;
            case BORRAR_TODO:
                escucha.borrarTodo();
                break;
            default:
                break;
        }
    }
}
