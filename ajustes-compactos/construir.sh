#!/bin/sh
# Compila la superposición que hace compacta la Configuración de Android
# (ajustes-compactos/ajustes-compactos.apk). Solo recursos: no necesita Java para el código, pero sí
# para firmar. Usa el SDK de Android como launcher/construir.sh:
#   $ANDROID_HOME (o ~/Android/Sdk)/platforms/android-33 y build-tools/34.0.0
# La llave de firma se crea la primera vez (firma.keystore) y no se sube al repositorio.
set -e
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
HERRAMIENTAS="$SDK/build-tools/34.0.0"
ANDROID_JAR="$SDK/platforms/android-33/android.jar"
for f in "$HERRAMIENTAS/aapt2" "$HERRAMIENTAS/zipalign" "$HERRAMIENTAS/apksigner" "$ANDROID_JAR"; do
    [ -e "$f" ] || { echo "Falta $f"; exit 1; }
done

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

"$HERRAMIENTAS/aapt2" compile --dir res -o "$TMP/recursos.zip"
"$HERRAMIENTAS/aapt2" link -o "$TMP/base.apk" -I "$ANDROID_JAR" --manifest AndroidManifest.xml \
    --min-sdk-version 30 --target-sdk-version 33 --version-code 1 --version-name 1.0 "$TMP/recursos.zip"
"$HERRAMIENTAS/zipalign" -f 4 "$TMP/base.apk" "$TMP/alineado.apk"

if [ ! -f firma.keystore ]; then
    keytool -genkeypair -keystore firma.keystore -storepass zykro-ajustes -keypass zykro-ajustes \
        -alias zykro -keyalg RSA -keysize 2048 -validity 36500 -dname "CN=ZYKRO" >/dev/null 2>&1
fi
"$HERRAMIENTAS/apksigner" -J-enable-native-access=ALL-UNNAMED sign --ks firma.keystore \
    --ks-pass pass:zykro-ajustes --key-pass pass:zykro-ajustes --out ajustes-compactos.apk "$TMP/alineado.apk"
rm -f ajustes-compactos.apk.idsig
echo "Listo: $(pwd)/ajustes-compactos.apk"
