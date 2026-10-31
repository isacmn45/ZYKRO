#!/bin/sh
# Compila la pantalla de inicio de ZYKRO (launcher/zykro-inicio.apk) sin Gradle ni Android Studio.
# Necesita Java y, del SDK de Android, la plataforma 33 y las build-tools 34:
#   $ANDROID_HOME (o ~/Android/Sdk)/platforms/android-33 y build-tools/34.0.0
# La llave de firma se crea la primera vez (launcher/firma.keystore) y no se sube al repositorio.
set -e
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
HERRAMIENTAS="$SDK/build-tools/34.0.0"
ANDROID_JAR="$SDK/platforms/android-33/android.jar"
for f in "$HERRAMIENTAS/aapt2" "$HERRAMIENTAS/d8" "$HERRAMIENTAS/zipalign" "$HERRAMIENTAS/apksigner" "$ANDROID_JAR"; do
    [ -e "$f" ] || { echo "Falta $f"; exit 1; }
done

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

"$HERRAMIENTAS/aapt2" compile --dir res -o "$TMP/recursos.zip"
"$HERRAMIENTAS/aapt2" link -o "$TMP/base.apk" -I "$ANDROID_JAR" --manifest AndroidManifest.xml \
    --min-sdk-version 30 --target-sdk-version 33 --version-code 4 --version-name 1.3 \
    --java "$TMP/gen" "$TMP/recursos.zip"

# Concatenar texto sin invokedynamic: así d8 no tiene nada que traducir.
javac --release 17 -XDstringConcat=inline -nowarn -encoding UTF-8 -classpath "$ANDROID_JAR" -d "$TMP/clases" \
    $(find src "$TMP/gen" -name '*.java')
"$HERRAMIENTAS/d8" --release --min-api 30 --lib "$ANDROID_JAR" --output "$TMP" $(find "$TMP/clases" -name '*.class')
(cd "$TMP" && jar uMf base.apk classes.dex)
"$HERRAMIENTAS/zipalign" -f 4 "$TMP/base.apk" "$TMP/alineado.apk"

if [ ! -f firma.keystore ]; then
    keytool -genkeypair -keystore firma.keystore -storepass zykro-inicio -keypass zykro-inicio \
        -alias zykro -keyalg RSA -keysize 2048 -validity 36500 -dname "CN=ZYKRO" >/dev/null 2>&1
fi
"$HERRAMIENTAS/apksigner" -J-enable-native-access=ALL-UNNAMED sign --ks firma.keystore --ks-pass pass:zykro-inicio --key-pass pass:zykro-inicio \
    --out zykro-inicio.apk "$TMP/alineado.apk"
rm -f zykro-inicio.apk.idsig
echo "Listo: $(pwd)/zykro-inicio.apk"
