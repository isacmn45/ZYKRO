package com.zykro.inicio;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.widget.Toast;

/**
 * Elige el fondo de pantalla en la galería de Android (o en Descargas, con el selector de archivos).
 * La abre ZYKRO desde su ventana de fondos; es transparente y se cierra sola al terminar.
 */
public class ElegirFondo extends Activity {
    private static final int ELEGIR = 1;

    @Override
    protected void onCreate(Bundle guardado) {
        super.onCreate(guardado);
        if (guardado != null) {
            return;
        }
        Intent elegir = new Intent(Intent.ACTION_GET_CONTENT).setType("image/*").addCategory(Intent.CATEGORY_OPENABLE);
        try {
            startActivityForResult(elegir, ELEGIR);
        } catch (ActivityNotFoundException e) {
            Toast.makeText(this, "No hay una galería para elegir la imagen", Toast.LENGTH_SHORT).show();
            finish();
        }
    }

    @Override
    protected void onActivityResult(int pedido, int resultado, Intent datos) {
        Uri imagen = datos == null ? null : datos.getData();
        if (pedido != ELEGIR || resultado != RESULT_OK || imagen == null) {
            finish();
            return;
        }
        new Thread(() -> {
            boolean bien = Fondo.poner(getApplicationContext(), () -> getContentResolver().openInputStream(imagen), "");
            runOnUiThread(() -> {
                if (!bien) {
                    Toast.makeText(this, "No se pudo usar esa imagen", Toast.LENGTH_SHORT).show();
                }
                finish();
            });
        }).start();
    }
}
