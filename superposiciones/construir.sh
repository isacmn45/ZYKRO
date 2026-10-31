#!/bin/sh
# Compila las superposiciones de la interfaz de Android:
#   marco/ -> marco.apk (sin barra de estado ni de navegación)
# Solo recursos. Necesita Java para firmar y, del SDK de Android, la plataforma 33 y las build-tools 34:
#   $ANDROID_HOME (o ~/Android/Sdk)/platforms/android-33 y build-tools/34.0.0
# La llave de firma se crea la primera vez (firma.keystore) y no se sube al repositorio. Van en la
# imagen de Android de ZYKRO (imagen/construir.sh).
set -e
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
HERRAMIENTAS="$SDK/build-tools/34.0.0"
ANDROID_JAR="$SDK/platforms/android-33/android.jar"
for f in "$HERRAMIENTAS/aapt2" "$HERRAMIENTAS/zipalign" "$HERRAMIENTAS/apksigner" "$ANDROID_JAR"; do
    [ -e "$f" ] || { echo "Falta $f"; exit 1; }
done
if [ ! -f firma.keystore ]; then
    keytool -genkeypair -keystore firma.keystore -storepass zykro-ajustes -keypass zykro-ajustes \
        -alias zykro -keyalg RSA -keysize 2048 -validity 36500 -dname "CN=ZYKRO" >/dev/null 2>&1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
# Sin deduplicar: aapt2 quita una variante (por ejemplo, horizontal) si vale lo mismo que la general,
# pero aquí hace falta para tapar la variante propia de la app.
for nombre in marco; do
    "$HERRAMIENTAS/aapt2" compile --dir "$nombre/res" -o "$TMP/$nombre.zip"
    "$HERRAMIENTAS/aapt2" link -o "$TMP/$nombre-base.apk" -I "$ANDROID_JAR" --manifest "$nombre/AndroidManifest.xml" \
        --min-sdk-version 28 --target-sdk-version 28 --version-code 1 --version-name 1.0 --no-resource-deduping \
        "$TMP/$nombre.zip"
    "$HERRAMIENTAS/zipalign" -f 4 "$TMP/$nombre-base.apk" "$TMP/$nombre-alineado.apk"
    "$HERRAMIENTAS/apksigner" -J-enable-native-access=ALL-UNNAMED sign --ks firma.keystore \
        --ks-pass pass:zykro-ajustes --key-pass pass:zykro-ajustes --out "$nombre.apk" "$TMP/$nombre-alineado.apk"
    rm -f "$nombre.apk.idsig"
    echo "Listo: $(pwd)/$nombre.apk"
done
