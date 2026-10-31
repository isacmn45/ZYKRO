#!/bin/sh
# Corre dentro de la imagen (etapa «piezas» del Dockerfile): acomoda Houdini, Google Play y las
# propiedades en /salida, que luego se copia sobre el Android de redroid.
set -e
S=/salida/system
V=/salida/vendor
mkdir -p "$S/lib" "$S/priv-app" "$S/app" "$S/etc" "$S/framework" "$V"

# Houdini: el traductor y las librerías de Android para ARM en /system/lib/arm; el puente que carga
# Android (ro.dalvik.vm.native.bridge) en /system/lib.
unsquashfs -q -d "$S/lib/arm" /tmp/houdini.sfs >/dev/null
mv "$S/lib/arm/libhoudini.so" "$S/lib/libhoudini.so"
# Android 9 solo deja abrir librerías de rutas conocidas: se suma la de Houdini (abre arm/nb/libtcb.so).
mkdir -p "$S/etc"
awk '{ print } /^namespace\.default\.permitted\.paths \+= \/system\/product\/priv-app$/ && !hecho {
    print "namespace.default.permitted.paths += /system/${LIB}/arm"; hecho = 1 }' /tmp/ld.config.28.txt > "$S/etc/ld.config.28.txt"
grep -q 'permitted.paths += /system/${LIB}/arm' "$S/etc/ld.config.28.txt"

# libbinder: Android 9 lleva la cuenta de la memoria compartida que pasa por binder y aborta si al
# soltarla la resta da negativo. Con memfd (el kernel ya no trae ashmem) un archivo puede crecer
# entre que se suma y se resta, y Google Play Services se caía en bucle. El salto al aborto pasa a
# saltarse solo la resta.
parchear() { # archivo desplazamiento byte-original byte-nuevo
    actual=$(od -An -tx1 -j "$2" -N 1 "$1" | tr -d ' ')
    [ "$actual" = "$3" ] || { echo "$1: byte inesperado en $2 ($actual)"; exit 1; }
    printf "\\x$4" | dd of="$1" bs=1 seek="$2" conv=notrunc 2>/dev/null
}
mkdir -p "$S/lib64"
cp /tmp/libbinder64.so "$S/lib64/libbinder.so"
cp /tmp/libbinder32.so "$S/lib/libbinder.so"
parchear "$S/lib64/libbinder.so" $((0x4ea65)) 46 03 # jb +0x46 (aborto) -> jb +3 (sin restar)
parchear "$S/lib/libbinder.so" $((0x4a124)) 3d 02   # jb +0x3d (aborto) -> jb +2 (sin restar)

# Google Play: solo lo necesario para iniciar sesión, instalar y actualizar apps. Sin el asistente
# de configuración (frenaría el primer arranque) ni las apps de Google que reemplazan a las de AOSP.
mkdir /tmp/g
cd /tmp/g
bsdtar xf /tmp/gapps.zip 'Core/*'
for p in gsfcore-all gmscore-x86_64 vending-x86_64 configupdater-all googlebackuptransport-all \
    googleonetimeinitializer-all googlepartnersetup-all defaultetc-common defaultframework-common; do
    bsdtar xf "Core/$p.tar.lz"
done
for d in */nodpi/priv-app/* */nodpi/app/*; do
    [ -e "$d" ] || continue
    case "$d" in */priv-app/*) cp -r "$d" "$S/priv-app/" ;; *) cp -r "$d" "$S/app/" ;; esac
done
cp -r defaultetc-common/common/etc/. "$S/etc/"
cp -r defaultframework-common/common/. "$S/"

# Propiedades: Android anuncia ARM (32 bits) además de x86 y carga Houdini para esas apps.
sed -e 's/^ro.product.cpu.abilist=.*/ro.product.cpu.abilist=x86_64,x86,armeabi-v7a,armeabi/' \
    -e 's/^ro.product.cpu.abilist32=.*/ro.product.cpu.abilist32=x86,armeabi-v7a,armeabi/' \
    /tmp/system.prop > "$S/build.prop"
cat >> "$S/build.prop" <<'FIN'

# ZYKRO: traducción de apps ARM con Houdini.
ro.dalvik.vm.isa.arm=x86
ro.enable.native.bridge.exec=1
FIN
sed -e 's/^ro.vendor.product.cpu.abilist=.*/ro.vendor.product.cpu.abilist=x86_64,x86,armeabi-v7a,armeabi/' \
    -e 's/^ro.vendor.product.cpu.abilist32=.*/ro.vendor.product.cpu.abilist32=x86,armeabi-v7a,armeabi/' \
    -e 's/^ro.dalvik.vm.native.bridge=.*/ro.dalvik.vm.native.bridge=libhoudini.so/' \
    /tmp/vendor.prop > "$V/build.prop"

# Permisos de Android: carpetas 755, archivos 644 y el traductor ejecutable.
find /salida -type d -exec chmod 755 {} +
find /salida -type f -exec chmod 644 {} +
chmod 755 "$S/lib/arm/houdini"
