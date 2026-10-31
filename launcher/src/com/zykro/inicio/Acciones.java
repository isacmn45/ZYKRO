package com.zykro.inicio;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import java.io.File;
import java.io.FileInputStream;

/**
 * Lo que ZYKRO pide (con `am broadcast` desde la shell de Android): abrir una notificación, tocar
 * uno de sus botones, quitarla o quitar todas, «No molestar» y cambiar el fondo de pantalla. Solo se aceptan
 * órdenes de la shell o del sistema (permiso DUMP, ver AndroidManifest.xml).
 */
public class Acciones extends BroadcastReceiver {
    static final String ABRIR = "com.zykro.inicio.ABRIR";
    static final String ACCION = "com.zykro.inicio.ACCION";
    static final String DESCARTAR = "com.zykro.inicio.DESCARTAR";
    static final String BORRAR_TODO = "com.zykro.inicio.BORRAR_TODO";
    /** Con `activo` (booleano), enciende o apaga «No molestar». */
    static final String NO_MOLESTAR = "com.zykro.inicio.NO_MOLESTAR";
    /** Con `archivo` (una imagen que ZYKRO dejó en la carpeta de esta app), la pone de fondo; sin él, vuelve al de siempre. */
    static final String FONDO = "com.zykro.inicio.FONDO";

    @Override
    public void onReceive(Context contexto, Intent intent) {
        String accion = intent.getAction();
        if (FONDO.equals(accion)) {
            cambiarFondo(contexto.getApplicationContext(), intent.getStringExtra("archivo"), intent.getStringExtra("nombre"));
            return;
        }
        Escucha escucha = Escucha.activa;
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
            case NO_MOLESTAR:
                escucha.noMolestar(intent.getBooleanExtra("activo", false));
                break;
            default:
                break;
        }
    }

    /** Leer y ajustar una foto grande tarda: se hace en otro hilo, con el aviso abierto hasta terminar. */
    private void cambiarFondo(Context contexto, String archivo, String nombre) {
        PendingResult pendiente = goAsync();
        new Thread(() -> {
            try {
                File imagen = archivo == null ? null : Fondo.archivo(contexto, archivo);
                if (imagen == null) {
                    Fondo.quitar(contexto);
                } else {
                    Fondo.poner(contexto, () -> new FileInputStream(imagen), nombre == null ? "" : nombre);
                    imagen.delete();
                }
            } finally {
                pendiente.finish();
            }
        }).start();
    }
}
