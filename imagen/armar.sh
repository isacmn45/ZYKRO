#!/bin/sh
# Corre dentro de la imagen (etapa «piezas» del Dockerfile): acomoda Houdini, Google Play y las
# propiedades en /salida, que luego se copia sobre el Android de redroid.
set -e
S=/salida/system
V=/salida/vendor
mkdir -p "$S" "$V/odm/etc" "$S/system_ext/etc" "$S/product/etc"

parchear() { # archivo desplazamiento bytes-originales bytes-nuevos (en hexadecimal)
    n=$((${#3} / 2))
    actual=$(od -An -tx1 -j "$2" -N "$n" "$1" | tr -d ' \n')
    [ "$actual" = "$3" ] || { echo "$1: bytes inesperados en $2 ($actual)"; exit 1; }
    printf "$(echo "$4" | sed 's/../\\x&/g')" | dd of="$1" bs=1 seek="$2" conv=notrunc 2>/dev/null
}

# Houdini 14 (el mismo que usan Waydroid y otros emuladores): el traductor y las librerías de Android
# para ARM en /system/lib/arm y /system/lib64/arm64, y el puente que carga Android en /system/lib{,64}.
mkdir /tmp/h
bsdtar xf /tmp/houdini13.zip -C /tmp/h --strip-components 2 '*/prebuilts/lib' '*/prebuilts/lib64' '*/prebuilts/bin' '*/prebuilts/etc'
cp -r /tmp/h/lib /tmp/h/lib64 /tmp/h/bin "$S/"
# Y su registro en binfmt_misc, para que Android también pueda ejecutar programas ARM (no solo cargar
# librerías). Es del kernel de la laptop, no del contenedor: es lo mismo que hacía la imagen de SwagPlayer.
mkdir -p "$V/etc/init"
cp -r /tmp/h/etc/binfmt_misc "$V/etc/"
cp /tmp/h/etc/init/houdini.rc "$V/etc/init/houdini.rc"
# El parche de Free Fire: con dos o más hilos en cierto estado, Houdini daba un error fatal y se quedaba
# en un bucle infinito (pantalla negra). El salto a ese error pasa a ser seis NOP.
parchear "$S/lib64/libhoudini.so" $((0xe61d5)) 0f83680d0000 909090909090

# Google Play: solo lo necesario para iniciar sesión, instalar y actualizar apps. Sin el asistente de
# configuración (frenaría el primer arranque), el buscador de Google ni sus otras apps.
mkdir /tmp/g
cd /tmp/g
bsdtar xf /tmp/gapps13.zip
for d in product/priv-app/GmsCore product/priv-app/Phonesky product/priv-app/GooglePartnerSetup \
    system_ext/priv-app/GoogleServicesFramework product/etc product/overlay/GmsOverlay.apk \
    product/overlay/GmsSettingsProviderOverlay.apk system_ext/etc/permissions; do
    mkdir -p "$S/$(dirname "$d")"
    cp -r "system/$d" "$S/$d"
done

# Propiedades: como un teléfono de verdad (compilación «user», sin depuración, ADB con llave) y Houdini
# para las apps ARM y ARM64 (también sus programas: native.bridge.exec y exec64).
de_usuario() {
    sed -e 's/build\.type=userdebug/build.type=user/' -e 's/build\.tags=test-keys/build.tags=release-keys/' \
        -e 's/^ro\.debuggable=1/ro.debuggable=0/' -e 's/^ro\.adb\.secure=0/ro.adb.secure=1/' \
        -e 's/^persist\.sys\.usb\.config=adb/persist.sys.usb.config=none/' "$1"
}
de_usuario /tmp/props/system.prop > "$S/build.prop"
de_usuario /tmp/props/system_ext.prop > "$S/system_ext/etc/build.prop"
de_usuario /tmp/props/product.prop > "$S/product/etc/build.prop"
de_usuario /tmp/props/odm.prop > "$V/odm/etc/build.prop"
de_usuario /tmp/props/vendor.prop | sed -e 's/^ro\.dalvik\.vm\.native\.bridge=.*/ro.dalvik.vm.native.bridge=libhoudini.so/' > "$V/build.prop"
echo 'ro.enable.native.bridge.exec64=1' >> "$V/build.prop"
# Lo demás que no hay en el contenedor (Bluetooth, NFC, el asistente del primer arranque) apagado, y
# SurfaceFlinger sin desenfoques entre ventanas ni estadísticas: lo mismo que tenía la imagen de SwagPlayer.
# Y sin la predicción de toques: con los del mouse de ZYKRO hacía temblar la mira (ver SIN_PREDICCION en
# zykro/src/android.rs).
cat >> "$S/build.prop" <<'FIN'

# ZYKRO
ro.input.resampling=0
persist.bluetooth.disabled=true
ro.nfc.port=disabled
ro.setupwizard.mode=DISABLED
persist.sys.sf.disable_blurs=1
debug.sf.disable_timestats=1
ro.config.alarm_vol_steps=7
ro.config.alarm_vol_default=7
FIN
mkdir -p "$S/etc/init"
# El traductor de redroid (ndk_translation) queda sin usar: su registro en binfmt_misc chocaría con
# el de Houdini.
echo "# Vacío: ZYKRO usa Houdini (ver houdini.rc en /vendor/etc/init)." > "$S/etc/init/ndk_translation.rc"
grep -q '^ro.debuggable=0' "$S/build.prop"
grep -q '^ro.dalvik.vm.native.bridge=libhoudini.so' "$V/build.prop"

# Permisos de Android: carpetas 755, archivos 644 y los traductores ejecutables.
find /salida -type d -exec chmod 755 {} +
find /salida -type f -exec chmod 644 {} +
chmod 755 "$S/bin/houdini" "$S/bin/houdini64"
