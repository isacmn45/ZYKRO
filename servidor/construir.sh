#!/bin/sh
# Compila el servidor de ZYKRO para Android (servidor/zykro-servidor.jar): Java pasado a dex, que
# Android corre con app_process (sin APK ni Gradle). Necesita Java y, del SDK de Android, la
# plataforma 33 y las build-tools 34 ($ANDROID_HOME o ~/Android/Sdk).
set -e
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
HERRAMIENTAS="$SDK/build-tools/34.0.0"
ANDROID_JAR="$SDK/platforms/android-33/android.jar"
for f in "$HERRAMIENTAS/d8" "$ANDROID_JAR"; do
    [ -e "$f" ] || { echo "Falta $f"; exit 1; }
done

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Concatenar texto sin invokedynamic: así d8 no tiene nada que traducir.
javac --release 17 -XDstringConcat=inline -nowarn -encoding UTF-8 -classpath "$ANDROID_JAR" -d "$TMP/clases" \
    $(find src -name '*.java')
# Android 9 (API 28) en adelante.
"$HERRAMIENTAS/d8" --release --min-api 28 --lib "$ANDROID_JAR" --output "$TMP" $(find "$TMP/clases" -name '*.class')
(cd "$TMP" && jar cMf zykro-servidor.jar classes.dex)
cp "$TMP/zykro-servidor.jar" zykro-servidor.jar
echo "Listo: $(pwd)/zykro-servidor.jar"
