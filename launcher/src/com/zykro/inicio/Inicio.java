package com.zykro.inicio;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.BroadcastReceiver;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.ActivityInfo;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.graphics.Canvas;
import android.graphics.RenderEffect;
import android.graphics.Shader;
import android.graphics.Typeface;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.StateListDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.text.Editable;
import android.text.TextUtils;
import android.text.TextWatcher;
import android.util.Log;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.GridLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.ScrollView;
import android.widget.TextClock;
import android.widget.TextView;
import android.widget.Toast;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.text.Collator;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Pantalla de inicio de ZYKRO, al estilo de BlueStacks 5: buscador arriba, tus apps y juegos juntos y
 * las del sistema en una carpeta. Mantener presionado un espacio vacío abre la ventana de fondos de
 * pantalla de ZYKRO.
 */
public class Inicio extends Activity implements TextWatcher {
    // Colores de ZYKRO (los mismos de su ventana).
    private static final int FONDO_ARRIBA = 0xFF1B1729;
    private static final int FONDO_ABAJO = 0xFF0E0B17;
    private static final int PANEL = 0xFF221C33;
    private static final int CONTROL = 0xFF282239;
    private static final int RESALTE = 0x26FFFFFF;
    private static final int TEXTO = 0xFFEEEBF6;
    private static final int TENUE = 0xFF8A84A3;
    private static final int ACENTO = 0xFFA855F7;

    private static final int MATCH = ViewGroup.LayoutParams.MATCH_PARENT;
    private static final int WRAP = ViewGroup.LayoutParams.WRAP_CONTENT;

    /** Etiqueta del registro de Android que lee ZYKRO. */
    private static final String ETIQUETA = "ZYKRO";

    /** La que está abierta, para que {@link Fondo} le avise que cambió el fondo. */
    static volatile Inicio activa;

    private static final class App {
        String paquete;
        ComponentName actividad;
        String nombre;
        boolean sistema;
        Bitmap icono;
    }

    private final ExecutorService fondo = Executors.newSingleThreadExecutor();
    private final Handler principal = new Handler(Looper.getMainLooper());
    private List<App> apps = new ArrayList<>();
    /** Número de la última carga pedida: si llega una vieja, se descarta. */
    private int carga;
    private float dp;
    private FrameLayout raiz;
    private ImageView imagenFondo;
    private View veloFondo;
    private ScrollView desplazable;
    private LinearLayout contenido;
    private EditText buscador;
    private View carpetaAbierta;
    private BroadcastReceiver cambios;

    @Override
    protected void onCreate(Bundle guardado) {
        super.onCreate(guardado);
        dp = getResources().getDisplayMetrics().density;

        activa = this;
        raiz = new FrameLayout(this);
        raiz.setBackground(new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM, new int[] {FONDO_ARRIBA, FONDO_ABAJO}));
        // El fondo elegido, si hay, con un velo oscuro para que los nombres se lean sobre cualquier imagen.
        imagenFondo = new ImageView(this);
        imagenFondo.setScaleType(ImageView.ScaleType.CENTER_CROP);
        imagenFondo.setVisibility(View.GONE);
        raiz.addView(imagenFondo, new FrameLayout.LayoutParams(MATCH, MATCH));
        veloFondo = new View(this);
        veloFondo.setBackground(new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM, new int[] {0x80000000, 0x33000000, 0x59000000}));
        veloFondo.setVisibility(View.GONE);
        raiz.addView(veloFondo, new FrameLayout.LayoutParams(MATCH, MATCH));

        LinearLayout columna = new LinearLayout(this);
        columna.setOrientation(LinearLayout.VERTICAL);
        columna.setPadding(px(24), px(14), px(24), 0);
        columna.addView(barraSuperior(), new LinearLayout.LayoutParams(MATCH, WRAP));

        desplazable = new ScrollView(this);
        desplazable.setVerticalScrollBarEnabled(false);
        desplazable.setOverScrollMode(View.OVER_SCROLL_NEVER);
        // Que el contenido llegue hasta abajo: así todo el espacio vacío recibe el toque largo.
        desplazable.setFillViewport(true);
        contenido = new LinearLayout(this);
        contenido.setOrientation(LinearLayout.VERTICAL);
        contenido.setPadding(0, px(2), 0, px(20));
        desplazable.addView(contenido, new FrameLayout.LayoutParams(MATCH, WRAP));
        columna.addView(desplazable, new LinearLayout.LayoutParams(MATCH, 0, 1));

        raiz.addView(columna, new FrameLayout.LayoutParams(MATCH, MATCH));
        setContentView(raiz);
        pantallaCompleta();
        cargarFondo();

        // Mantener presionado un espacio vacío: ZYKRO escucha esta línea del registro y abre su
        // ventana de fondos de pantalla. Los toques que ninguna app ni botón toma llegan hasta aquí.
        View.OnLongClickListener fondos = vista -> {
            Log.i(ETIQUETA, "fondos");
            return true;
        };
        raiz.setOnLongClickListener(fondos);
        columna.setOnLongClickListener(fondos);
        contenido.setOnLongClickListener(fondos);

        // Apps instaladas, quitadas o actualizadas: se vuelve a armar la lista.
        cambios = new Cambios(this);
        IntentFilter filtro = new IntentFilter();
        filtro.addAction(Intent.ACTION_PACKAGE_ADDED);
        filtro.addAction(Intent.ACTION_PACKAGE_REMOVED);
        filtro.addAction(Intent.ACTION_PACKAGE_CHANGED);
        filtro.addAction(Intent.ACTION_PACKAGE_REPLACED);
        filtro.addDataScheme("package");
        registerReceiver(cambios, filtro, Context.RECEIVER_NOT_EXPORTED);
    }

    @Override
    protected void onResume() {
        super.onResume();
        cargar();
    }

    @Override
    public void onWindowFocusChanged(boolean conFoco) {
        super.onWindowFocusChanged(conFoco);
        if (conFoco) {
            pantallaCompleta();
        }
    }

    /** Volver a Inicio (botón de inicio) deja la pantalla como recién abierta. */
    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        cerrarCarpeta();
        if (buscador.length() > 0) {
            buscador.setText("");
        }
        ocultarTeclado();
        desplazable.smoothScrollTo(0, 0);
    }

    /** Atrás cierra la carpeta o borra la búsqueda; Inicio nunca se cierra. */
    @Override
    public void onBackPressed() {
        if (carpetaAbierta != null) {
            cerrarCarpeta();
        } else if (buscador.length() > 0) {
            buscador.setText("");
            ocultarTeclado();
        }
    }

    @Override
    protected void onDestroy() {
        if (activa == this) {
            activa = null;
        }
        unregisterReceiver(cambios);
        fondo.shutdownNow();
        super.onDestroy();
    }

    // Sin clases anónimas: javac 27 les pone datos que el d8 de las build-tools 34 no sabe leer.

    /** Avisos de apps instaladas, quitadas o actualizadas. */
    private static final class Cambios extends BroadcastReceiver {
        private final Inicio inicio;

        Cambios(Inicio inicio) {
            this.inicio = inicio;
        }

        @Override
        public void onReceive(Context contexto, Intent intent) {
            inicio.cargar();
        }
    }

    /** Cada letra del buscador vuelve a filtrar las apps. */
    @Override
    public void afterTextChanged(Editable texto) {
        mostrar();
    }

    @Override
    public void beforeTextChanged(CharSequence s, int inicio, int quitados, int puestos) {}

    @Override
    public void onTextChanged(CharSequence s, int inicio, int quitados, int puestos) {}

    /** Dibuja el fondo elegido (ver {@link Fondo}) o, si no hay, el degradado de ZYKRO. */
    void cargarFondo() {
        File archivo = Fondo.archivo(this, Fondo.IMAGEN);
        fondo.execute(() -> {
            Bitmap imagen = archivo != null && archivo.exists() ? BitmapFactory.decodeFile(archivo.getPath()) : null;
            principal.post(() -> {
                if (isDestroyed()) {
                    return;
                }
                imagenFondo.setImageBitmap(imagen);
                int visible = imagen == null ? View.GONE : View.VISIBLE;
                imagenFondo.setVisibility(visible);
                veloFondo.setVisibility(visible);
            });
        });
    }

    // -----------------------------------------------------------------------------------------
    // Apps
    // -----------------------------------------------------------------------------------------

    /** Lee las apps que se pueden abrir y dibuja sus íconos (en otro hilo). */
    private void cargar() {
        final int esta = ++carga;
        final int lado = px(48);
        fondo.execute(() -> {
            PackageManager pm = getPackageManager();
            Intent abrir = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER);
            List<App> nuevas = new ArrayList<>();
            for (ResolveInfo ri : pm.queryIntentActivities(abrir, 0)) {
                ActivityInfo actividad = ri.activityInfo;
                if (actividad.packageName.equals(getPackageName())) {
                    continue;
                }
                ApplicationInfo info = actividad.applicationInfo;
                App a = new App();
                a.paquete = actividad.packageName;
                a.actividad = new ComponentName(actividad.packageName, actividad.name);
                a.nombre = String.valueOf(ri.loadLabel(pm));
                a.sistema = (info.flags & (ApplicationInfo.FLAG_SYSTEM | ApplicationInfo.FLAG_UPDATED_SYSTEM_APP)) != 0;
                a.icono = Iconos.dibujar(ri.loadIcon(pm), lado);
                nuevas.add(a);
            }
            Collator orden = Collator.getInstance(new Locale("es"));
            nuevas.sort((x, y) -> orden.compare(x.nombre, y.nombre));
            exportar(nuevas);
            principal.post(() -> {
                if (esta != carga || isDestroyed()) {
                    return;
                }
                apps = nuevas;
                mostrar();
            });
        });
    }

    /**
     * Deja la lista de apps (nombre y si es del sistema) y sus íconos en la carpeta de esta app
     * (/sdcard/Android/data/com.zykro.inicio/files), para que ZYKRO los muestre: desde la laptop no
     * hay forma de saber el nombre ni el ícono de una app.
     */
    private void exportar(List<App> lista) {
        File carpeta = getExternalFilesDir(null);
        if (carpeta == null) {
            return;
        }
        File iconos = new File(carpeta, "iconos");
        iconos.mkdirs();
        JSONArray todas = new JSONArray();
        try {
            for (App a : lista) {
                JSONObject app = new JSONObject();
                app.put("paquete", a.paquete);
                app.put("nombre", a.nombre);
                app.put("sistema", a.sistema);
                todas.put(app);
                try (FileOutputStream salida = new FileOutputStream(new File(iconos, a.paquete + ".png"))) {
                    a.icono.compress(Bitmap.CompressFormat.PNG, 100, salida);
                }
            }
            // ZYKRO actualiza los accesos directos de las apps cuando la lista cambia.
            byte[] nueva = todas.toString().getBytes(StandardCharsets.UTF_8);
            File json = new File(carpeta, "apps.json");
            boolean cambio = !json.exists() || !Arrays.equals(nueva, Files.readAllBytes(json.toPath()));
            File temporal = new File(carpeta, "apps.json.tmp");
            try (FileOutputStream salida = new FileOutputStream(temporal)) {
                salida.write(nueva);
            }
            temporal.renameTo(json);
            if (cambio) {
                Log.i(ETIQUETA, "apps");
            }
        } catch (IOException | JSONException e) {
            Log.w(ETIQUETA, "no se pudo guardar la lista de apps", e);
        }
    }

    /** Arma el contenido: los resultados de la búsqueda, o tus apps y la carpeta del sistema. */
    private void mostrar() {
        contenido.removeAllViews();
        String buscado = buscador.getText().toString().trim();
        if (!buscado.isEmpty()) {
            String clave = buscado.toLowerCase(Locale.ROOT);
            List<App> halladas = new ArrayList<>();
            for (App a : apps) {
                if (a.nombre.toLowerCase(Locale.ROOT).contains(clave)) {
                    halladas.add(a);
                }
            }
            titulo("Resultados");
            if (halladas.isEmpty()) {
                aviso("No hay apps que coincidan con «" + buscado + "».");
            } else {
                contenido.addView(mosaico(halladas), new LinearLayout.LayoutParams(MATCH, WRAP));
            }
            return;
        }

        // Juegos y apps van juntos; solo las del sistema (que no se pueden desinstalar) van en la carpeta.
        List<App> propias = new ArrayList<>();
        List<App> sistema = new ArrayList<>();
        for (App a : apps) {
            (a.sistema ? sistema : propias).add(a);
        }
        titulo("Mis apps");
        Mosaico deApps = mosaico(propias);
        if (!sistema.isEmpty()) {
            deApps.addView(carpeta("Sistema", "Apps del sistema", sistema));
        }
        contenido.addView(deApps, new LinearLayout.LayoutParams(MATCH, WRAP));
    }

    private void abrirApp(App a) {
        abrir(new Intent(Intent.ACTION_MAIN)
                .addCategory(Intent.CATEGORY_LAUNCHER)
                .setComponent(a.actividad)
                .addFlags(Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
    }

    private void abrir(Intent intent) {
        try {
            startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        } catch (ActivityNotFoundException | SecurityException e) {
            Toast.makeText(this, "No se pudo abrir", Toast.LENGTH_SHORT).show();
        }
    }

    /** Enter en el buscador abre la primera app de los resultados. */
    private void abrirPrimerResultado() {
        String clave = buscador.getText().toString().trim().toLowerCase(Locale.ROOT);
        for (App a : apps) {
            if (!clave.isEmpty() && a.nombre.toLowerCase(Locale.ROOT).contains(clave)) {
                abrirApp(a);
                return;
            }
        }
    }

    /** Mantener presionada una app: su información o desinstalarla. */
    private void menu(View ancla, App a) {
        PopupMenu menu = new PopupMenu(this, ancla);
        menu.getMenu().add(0, 1, 0, "Información de la app");
        if (!a.sistema) {
            menu.getMenu().add(0, 2, 1, "Desinstalar");
        }
        menu.setOnMenuItemClickListener(opcion -> {
            Uri paquete = Uri.parse("package:" + a.paquete);
            if (opcion.getItemId() == 1) {
                abrir(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, paquete));
            } else {
                abrir(new Intent(Intent.ACTION_DELETE, paquete));
            }
            return true;
        });
        menu.show();
    }

    // -----------------------------------------------------------------------------------------
    // Vistas
    // -----------------------------------------------------------------------------------------

    private View barraSuperior() {
        LinearLayout barra = new LinearLayout(this);
        barra.setOrientation(LinearLayout.HORIZONTAL);
        barra.setGravity(Gravity.CENTER_VERTICAL);

        LinearLayout caja = new LinearLayout(this);
        caja.setGravity(Gravity.CENTER_VERTICAL);
        caja.setPadding(px(12), 0, px(12), 0);
        caja.setBackground(redondeado(CONTROL, px(17)));
        ImageView lupa = new ImageView(this);
        lupa.setImageResource(R.drawable.lupa);
        caja.addView(lupa, new LinearLayout.LayoutParams(px(16), px(16)));
        buscador = new EditText(this);
        buscador.setHint("Buscar apps y juegos");
        buscador.setHintTextColor(TENUE);
        buscador.setTextColor(TEXTO);
        buscador.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        buscador.setSingleLine(true);
        buscador.setBackground(null);
        buscador.setPadding(px(8), 0, 0, 0);
        buscador.setImeOptions(EditorInfo.IME_ACTION_SEARCH);
        buscador.addTextChangedListener(this);
        buscador.setOnEditorActionListener((vista, accion, evento) -> {
            abrirPrimerResultado();
            return true;
        });
        caja.addView(buscador, new LinearLayout.LayoutParams(0, px(34), 1));
        barra.addView(caja, new LinearLayout.LayoutParams(px(320), px(34)));

        barra.addView(new View(this), new LinearLayout.LayoutParams(0, 1, 1));

        TextClock reloj = new TextClock(this);
        reloj.setFormat24Hour("HH:mm");
        reloj.setFormat12Hour("h:mm a");
        reloj.setTextColor(TEXTO);
        reloj.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        reloj.setPadding(px(12), 0, px(4), 0);
        sombra(reloj);
        barra.addView(reloj);
        return barra;
    }

    private void titulo(String nombre) {
        LinearLayout fila = new LinearLayout(this);
        fila.setGravity(Gravity.CENTER_VERTICAL);
        View marca = new View(this);
        marca.setBackground(redondeado(ACENTO, px(2)));
        fila.addView(marca, new LinearLayout.LayoutParams(px(3), px(14)));
        TextView texto = texto(nombre, 13, TEXTO, true);
        texto.setPadding(px(8), 0, 0, 0);
        fila.addView(texto);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(MATCH, WRAP);
        lp.topMargin = px(12);
        lp.bottomMargin = px(6);
        contenido.addView(fila, lp);
    }

    private void aviso(String mensaje) {
        TextView t = texto(mensaje, 12, TENUE, false);
        t.setPadding(px(11), 0, 0, px(6));
        contenido.addView(t);
    }

    private Mosaico mosaico(List<App> lista) {
        Mosaico m = new Mosaico(this, px(4));
        for (App a : lista) {
            m.addView(ficha(a));
        }
        return m;
    }

    /** Una app: su ícono y su nombre debajo. */
    private View ficha(App a) {
        int lado = px(46);
        LinearLayout f = base();
        ImageView icono = new ImageView(this);
        icono.setImageBitmap(a.icono);
        f.addView(icono, new LinearLayout.LayoutParams(lado, lado));
        f.addView(nombre(a.nombre), nombreParams());
        f.setContentDescription(a.nombre);
        f.setOnClickListener(vista -> abrirApp(a));
        f.setOnLongClickListener(vista -> {
            menu(vista, a);
            return true;
        });
        return f;
    }

    /** Carpeta con hasta cuatro íconos en miniatura; al tocarla se abre encima. */
    private View carpeta(String nombre, String titulo, List<App> dentro) {
        int lado = px(46);
        LinearLayout f = base();
        GridLayout cuadro = new GridLayout(this);
        cuadro.setColumnCount(2);
        cuadro.setPadding(px(4), px(4), px(4), px(4));
        cuadro.setBackground(acrilico(lado * Iconos.REDONDEO));
        cuadro.setClipToOutline(true);
        for (int i = 0; i < Math.min(4, dentro.size()); i++) {
            ImageView mini = new ImageView(this);
            mini.setImageBitmap(dentro.get(i).icono);
            GridLayout.LayoutParams lp = new GridLayout.LayoutParams();
            lp.width = px(17);
            lp.height = px(17);
            lp.setMargins(px(1), px(1), px(1), px(1));
            cuadro.addView(mini, lp);
        }
        f.addView(cuadro, new LinearLayout.LayoutParams(lado, lado));
        f.addView(nombre(nombre), nombreParams());
        f.setContentDescription(nombre);
        f.setOnClickListener(vista -> abrirCarpeta(titulo, dentro));
        return f;
    }

    private void abrirCarpeta(String nombre, List<App> dentro) {
        cerrarCarpeta();
        FrameLayout velo = new FrameLayout(this);
        if (Build.VERSION.SDK_INT >= 31 && raiz.isHardwareAccelerated() && raiz.getWidth() > 0 && raiz.getHeight() > 0) {
            Bitmap fondoCarpeta = Bitmap.createBitmap(Math.max(1, raiz.getWidth() / 4), Math.max(1, raiz.getHeight() / 4), Bitmap.Config.ARGB_8888);
            Canvas lienzo = new Canvas(fondoCarpeta);
            lienzo.scale((float) fondoCarpeta.getWidth() / raiz.getWidth(), (float) fondoCarpeta.getHeight() / raiz.getHeight());
            raiz.draw(lienzo);
            ImageView difuminado = new ImageView(this);
            difuminado.setImageBitmap(fondoCarpeta);
            difuminado.setScaleType(ImageView.ScaleType.FIT_XY);
            difuminado.setRenderEffect(RenderEffect.createBlurEffect(px(12), px(12), Shader.TileMode.CLAMP));
            velo.addView(difuminado, new FrameLayout.LayoutParams(MATCH, MATCH));
        }
        View tinte = new View(this);
        tinte.setBackgroundColor(0x66200F35);
        velo.addView(tinte, new FrameLayout.LayoutParams(MATCH, MATCH));
        velo.setOnClickListener(vista -> cerrarCarpeta());

        LinearLayout panel = new LinearLayout(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        panel.setPadding(px(22), px(20), px(22), px(20));
        panel.setBackground(acrilico(px(22)));
        panel.setClipToOutline(true);
        panel.setElevation(px(16));
        panel.setClickable(true); // tocar el panel no lo cierra
        LinearLayout encabezado = new LinearLayout(this);
        encabezado.setGravity(Gravity.CENTER_VERTICAL);
        View marca = new View(this);
        marca.setBackground(new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM, new int[] {0xFF8DE5EB, ACENTO}));
        LinearLayout.LayoutParams marcaParams = new LinearLayout.LayoutParams(px(3), px(22));
        marcaParams.rightMargin = px(12);
        encabezado.addView(marca, marcaParams);
        encabezado.addView(texto(nombre, 16, TEXTO, true), new LinearLayout.LayoutParams(0, WRAP, 1));
        TextView cerrar = texto("×", 22, TEXTO, false);
        cerrar.setGravity(Gravity.CENTER);
        cerrar.setContentDescription("Cerrar carpeta");
        cerrar.setBackground(presionable(px(16)));
        cerrar.setOnClickListener(vista -> cerrarCarpeta());
        encabezado.addView(cerrar, new LinearLayout.LayoutParams(px(32), px(32)));
        panel.addView(encabezado);
        View separador = new View(this);
        separador.setBackground(new GradientDrawable(GradientDrawable.Orientation.LEFT_RIGHT, new int[] {0x557FDFE9, 0x44A855F7, 0x00A855F7}));
        LinearLayout.LayoutParams lineaParams = new LinearLayout.LayoutParams(MATCH, px(1));
        lineaParams.topMargin = px(12);
        panel.addView(separador, lineaParams);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(MATCH, WRAP);
        lp.topMargin = px(18);
        ScrollView listado = new ScrollView(this);
        listado.setFillViewport(false);
        listado.addView(mosaico(dentro));
        panel.addView(listado, lp);

        int anchoPanel = Math.min(px(420), Math.max(px(100), raiz.getWidth() - px(40)));
        int altoMaximo = Math.max(px(100), raiz.getHeight() - px(48));
        panel.measure(View.MeasureSpec.makeMeasureSpec(anchoPanel, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(altoMaximo, View.MeasureSpec.AT_MOST));
        velo.addView(panel, new FrameLayout.LayoutParams(anchoPanel, panel.getMeasuredHeight(), Gravity.CENTER));
        raiz.addView(velo, new FrameLayout.LayoutParams(MATCH, MATCH));
        carpetaAbierta = velo;
    }

    private void cerrarCarpeta() {
        if (carpetaAbierta != null) {
            raiz.removeView(carpetaAbierta);
            carpetaAbierta = null;
        }
    }

    private LinearLayout base() {
        LinearLayout f = new LinearLayout(this);
        f.setOrientation(LinearLayout.VERTICAL);
        f.setGravity(Gravity.CENTER_HORIZONTAL);
        f.setPadding(px(4), px(6), px(4), px(6));
        f.setBackground(presionable(px(10)));
        return f;
    }

    private TextView nombre(String nombre) {
        TextView t = texto(nombre, 11, TEXTO, false);
        t.setSingleLine(true);
        t.setEllipsize(TextUtils.TruncateAt.END);
        t.setGravity(Gravity.CENTER_HORIZONTAL);
        return t;
    }

    private LinearLayout.LayoutParams nombreParams() {
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(px(66), WRAP);
        lp.topMargin = px(5);
        return lp;
    }

    private ImageView botonIcono(Drawable icono, View.OnClickListener accion) {
        ImageView b = new ImageView(this);
        b.setImageDrawable(icono);
        b.setPadding(px(6), px(6), px(6), px(6));
        b.setBackground(presionable(px(16)));
        b.setOnClickListener(accion);
        b.setLayoutParams(new LinearLayout.LayoutParams(px(32), px(32)));
        return b;
    }

    private TextView texto(String contenido, float sp, int color, boolean negrita) {
        TextView t = new TextView(this);
        t.setText(contenido);
        t.setTextColor(color);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sp);
        if (negrita) {
            t.setTypeface(Typeface.DEFAULT_BOLD);
        }
        sombra(t);
        return t;
    }

    /** Una sombra suave, para que el texto se lea sobre cualquier fondo de pantalla. */
    private void sombra(TextView t) {
        t.setShadowLayer(px(3), 0, px(1), 0x99000000);
    }

    private static GradientDrawable redondeado(int color, float radio) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(color);
        d.setCornerRadius(radio);
        return d;
    }

    private GradientDrawable acrilico(float radio) {
        GradientDrawable superficie = new GradientDrawable(GradientDrawable.Orientation.TL_BR, new int[] {0xE8473D60, 0xDC292238, 0xEB211D32});
        superficie.setCornerRadius(radio);
        superficie.setStroke(Math.max(1, px(0.7f)), 0x557EDDE7);
        return superficie;
    }

    /** Fondo que se ilumina al presionar (el mouse de ZYKRO llega como toque). */
    private static StateListDrawable presionable(float radio) {
        StateListDrawable d = new StateListDrawable();
        d.addState(new int[] {android.R.attr.state_pressed}, redondeado(RESALTE, radio));
        d.addState(new int[] {}, new ColorDrawable(Color.TRANSPARENT));
        return d;
    }

    /** Sin barra de estado ni de navegación, como BlueStacks. */
    private void pantallaCompleta() {
        Window ventana = getWindow();
        ventana.setDecorFitsSystemWindows(false);
        ventana.setStatusBarColor(Color.TRANSPARENT);
        ventana.setNavigationBarColor(Color.TRANSPARENT);
        WindowInsetsController barras = ventana.getInsetsController();
        if (barras != null) {
            barras.hide(WindowInsets.Type.systemBars());
            barras.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
        }
    }

    private void ocultarTeclado() {
        InputMethodManager teclado = getSystemService(InputMethodManager.class);
        if (teclado != null) {
            teclado.hideSoftInputFromWindow(buscador.getWindowToken(), 0);
        }
        buscador.clearFocus();
    }

    private int px(float valor) {
        return Math.round(valor * dp);
    }
}
