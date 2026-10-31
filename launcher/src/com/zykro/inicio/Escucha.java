package com.zykro.inicio;

import android.app.Notification;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.Icon;
import android.os.Bundle;
import android.service.notification.NotificationListenerService;
import android.service.notification.StatusBarNotification;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Escucha las notificaciones para el panel de ZYKRO (al estilo de Android 4, lo dibuja ZYKRO): deja
 * cada una, con su ícono, en la carpeta de esta app y avisa por el registro. ZYKRO le da el permiso
 * con `cmd notification allow_listener` y le pide abrirlas o quitarlas con {@link Acciones}. También
 * marca cuándo avisó cada una, para que ZYKRO la muestre como emergente (las de Android están apagadas).
 */
public class Escucha extends NotificationListenerService {
    /** La que está conectada, para {@link Acciones}. */
    static volatile Escucha activa;

    private final ExecutorService fondo = Executors.newSingleThreadExecutor();
    /** Cuándo avisó cada notificación por última vez (ms), por clave. */
    private final Map<String, Long> alertas = new ConcurrentHashMap<>();
    /** El título y el texto con que se vio cada una, para saber si una actualización cambió algo. */
    private final Map<String, String> textos = new ConcurrentHashMap<>();

    @Override
    public void onListenerConnected() {
        activa = this;
        exportar(false);
    }

    @Override
    public void onListenerDisconnected() {
        if (activa == this) {
            activa = null;
        }
    }

    @Override
    public void onNotificationPosted(StatusBarNotification sbn, RankingMap ranking) {
        boolean aviso = avisa(sbn, ranking);
        if (aviso) {
            alertas.put(sbn.getKey(), System.currentTimeMillis());
        }
        exportar(aviso);
    }

    @Override
    public void onNotificationRemoved(StatusBarNotification sbn) {
        alertas.remove(sbn.getKey());
        textos.remove(sbn.getKey());
        exportar(false);
    }

    /**
     * Si Android avisaría de esta notificación (suena o vibra): canal de importancia normal o más,
     * que «No molestar» no la silencie y, si es una actualización, que cambie el texto y la app no
     * haya pedido avisar una sola vez.
     */
    private boolean avisa(StatusBarNotification sbn, RankingMap ranking) {
        Notification n = sbn.getNotification();
        String texto = n.extras.getCharSequence(Notification.EXTRA_TITLE) + "\n" + n.extras.getCharSequence(Notification.EXTRA_TEXT);
        String antes = textos.put(sbn.getKey(), texto);
        if ((n.flags & Notification.FLAG_GROUP_SUMMARY) != 0) {
            return false;
        }
        Ranking r = new Ranking();
        if (ranking == null || !ranking.getRanking(sbn.getKey(), r)) {
            return false;
        }
        if (r.getImportance() < NotificationManager.IMPORTANCE_DEFAULT || !r.matchesInterruptionFilter()
                || (r.getSuppressedVisualEffects() & NotificationManager.Policy.SUPPRESSED_EFFECT_PEEK) != 0) {
            return false;
        }
        return antes == null || ((n.flags & Notification.FLAG_ONLY_ALERT_ONCE) == 0 && !texto.equals(antes));
    }

    @Override
    public void onDestroy() {
        fondo.shutdownNow();
        super.onDestroy();
    }

    /** Con `aviso`, también le dice a ZYKRO que hay una notificación para mostrar como emergente. */
    private void exportar(boolean aviso) {
        StatusBarNotification[] todas;
        try {
            todas = getActiveNotifications();
        } catch (SecurityException e) {
            return;
        }
        if (todas != null) {
            fondo.execute(() -> escribir(todas, aviso));
        }
    }

    /** notificaciones.json y un PNG por notificación en notificaciones/ (se rehace cada vez). */
    private void escribir(StatusBarNotification[] todas, boolean aviso) {
        File carpeta = getExternalFilesDir(null);
        if (carpeta == null) {
            return;
        }
        File iconos = new File(carpeta, "notificaciones");
        iconos.mkdirs();
        File[] viejos = iconos.listFiles();
        if (viejos != null) {
            for (File f : viejos) {
                f.delete();
            }
        }
        PackageManager pm = getPackageManager();
        int lado = Math.round(40 * getResources().getDisplayMetrics().density);
        JSONArray lista = new JSONArray();
        try {
            for (StatusBarNotification sbn : todas) {
                Notification n = sbn.getNotification();
                // Los resúmenes de grupo repiten lo que ya dicen sus notificaciones.
                if ((n.flags & Notification.FLAG_GROUP_SUMMARY) != 0) {
                    continue;
                }
                Bundle extras = n.extras;
                CharSequence titulo = extras.getCharSequence(Notification.EXTRA_TITLE);
                CharSequence texto = extras.getCharSequence(Notification.EXTRA_TEXT);
                CharSequence largo = extras.getCharSequence(Notification.EXTRA_BIG_TEXT);
                if (titulo == null && texto == null) {
                    continue;
                }
                JSONObject o = new JSONObject();
                o.put("clave", sbn.getKey());
                o.put("paquete", sbn.getPackageName());
                o.put("app", etiqueta(pm, sbn.getPackageName()));
                o.put("titulo", titulo == null ? "" : titulo.toString());
                o.put("texto", largo != null ? largo.toString() : texto == null ? "" : texto.toString());
                o.put("hora", sbn.getPostTime());
                o.put("descartable", sbn.isClearable());
                JSONArray acciones = new JSONArray();
                if (n.actions != null) {
                    for (Notification.Action a : n.actions) {
                        acciones.put(a.title == null ? "" : a.title.toString());
                    }
                }
                o.put("acciones", acciones);
                String archivo = Integer.toHexString(sbn.getKey().hashCode()) + ".png";
                Bitmap imagen = Iconos.dibujar(icono(pm, sbn), lado);
                try (FileOutputStream salida = new FileOutputStream(new File(iconos, archivo))) {
                    imagen.compress(Bitmap.CompressFormat.PNG, 100, salida);
                }
                o.put("icono", archivo);
                Long alerta = alertas.get(sbn.getKey());
                o.put("alerta", alerta == null ? 0 : alerta);
                lista.put(o);
            }
            File temporal = new File(carpeta, "notificaciones.json.tmp");
            try (FileOutputStream salida = new FileOutputStream(temporal)) {
                salida.write(lista.toString().getBytes(StandardCharsets.UTF_8));
            }
            temporal.renameTo(new File(carpeta, "notificaciones.json"));
            Log.i("ZYKRO", aviso ? "emergente" : "notificaciones");
        } catch (IOException | JSONException e) {
            Log.w("ZYKRO", "no se pudieron guardar las notificaciones", e);
        }
    }

    /** La imagen grande de la notificación (la foto de un chat, por ejemplo) o el ícono de la app. */
    private Drawable icono(PackageManager pm, StatusBarNotification sbn) {
        Icon grande = sbn.getNotification().getLargeIcon();
        Drawable d = grande == null ? null : grande.loadDrawable(this);
        if (d == null) {
            try {
                d = pm.getApplicationIcon(sbn.getPackageName());
            } catch (PackageManager.NameNotFoundException e) {
                d = pm.getDefaultActivityIcon();
            }
        }
        return d;
    }

    private static String etiqueta(PackageManager pm, String paquete) {
        try {
            return pm.getApplicationLabel(pm.getApplicationInfo(paquete, 0)).toString();
        } catch (PackageManager.NameNotFoundException e) {
            return paquete;
        }
    }

    private StatusBarNotification buscar(String clave) {
        StatusBarNotification[] todas = getActiveNotifications();
        if (todas != null && clave != null) {
            for (StatusBarNotification sbn : todas) {
                if (clave.equals(sbn.getKey())) {
                    return sbn;
                }
            }
        }
        return null;
    }

    /** Abre la notificación como si se tocara en el panel de Android. */
    void abrir(String clave) {
        StatusBarNotification sbn = buscar(clave);
        if (sbn == null) {
            return;
        }
        Notification n = sbn.getNotification();
        if (n.contentIntent != null) {
            enviar(n.contentIntent);
        }
        if ((n.flags & Notification.FLAG_AUTO_CANCEL) != 0 && sbn.isClearable()) {
            cancelNotification(clave);
        }
    }

    /** Toca uno de los botones de la notificación. */
    void accion(String clave, int indice) {
        StatusBarNotification sbn = buscar(clave);
        Notification.Action[] acciones = sbn == null ? null : sbn.getNotification().actions;
        if (acciones != null && indice >= 0 && indice < acciones.length && acciones[indice].actionIntent != null) {
            enviar(acciones[indice].actionIntent);
        }
    }

    void descartar(String clave) {
        cancelNotification(clave);
    }

    void borrarTodo() {
        cancelAllNotifications();
    }

    private static void enviar(PendingIntent pendiente) {
        try {
            pendiente.send();
        } catch (PendingIntent.CanceledException e) {
            Log.w("ZYKRO", "la notificación ya no se puede abrir", e);
        }
    }
}
