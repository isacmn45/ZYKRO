# Imagen de Android de ZYKRO (la arma construir.sh): redroid 9, de código abierto, con:
# - traducción de apps ARM (Houdini de Android-x86, para las de 32 bits como Free Fire),
# - Google Play (OpenGApps pico; Play y sus servicios se actualizan solos),
# - el servidor de ZYKRO (servidor/): pantalla, toques y sonido,
# - la superposición que quita las barras de Android (superposiciones/marco).
# Las descargas las verifica construir.sh; aquí solo se acomodan.
FROM alpine:3.20 AS piezas
RUN apk add --no-cache squashfs-tools libarchive-tools
COPY --from=redroid/redroid:9.0.0-latest /system/build.prop /tmp/system.prop
COPY --from=redroid/redroid:9.0.0-latest /vendor/build.prop /tmp/vendor.prop
COPY --from=redroid/redroid:9.0.0-latest /system/etc/ld.config.28.txt /tmp/ld.config.28.txt
COPY --from=redroid/redroid:9.0.0-latest /system/lib64/libbinder.so /tmp/libbinder64.so
COPY --from=redroid/redroid:9.0.0-latest /system/lib/libbinder.so /tmp/libbinder32.so
COPY descargas/houdini.sfs descargas/gapps.zip armar.sh /tmp/
RUN sh /tmp/armar.sh

FROM redroid/redroid:9.0.0-latest
COPY --from=piezas /salida/system/ /system/
COPY --from=piezas /salida/vendor/ /vendor/
COPY zykro-servidor.jar /system/framework/zykro-servidor.jar
COPY zykro-servidor.rc /system/etc/init/zykro-servidor.rc
COPY marco.apk /vendor/overlay/ZykroMarco.apk
# El entorno de Android también para `docker exec`: sin él, las órdenes hechas en Java (input, wm…)
# no arrancan.
ENV PATH=/sbin:/system/sbin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin \
    ANDROID_ROOT=/system ANDROID_DATA=/data ANDROID_STORAGE=/storage ANDROID_ASSETS=/system/app \
    EXTERNAL_STORAGE=/sdcard ASEC_MOUNTPOINT=/mnt/asec DOWNLOAD_CACHE=/data/cache \
    BOOTCLASSPATH=/system/framework/core-oj.jar:/system/framework/core-libart.jar:/system/framework/conscrypt.jar:/system/framework/okhttp.jar:/system/framework/bouncycastle.jar:/system/framework/apache-xml.jar:/system/framework/ext.jar:/system/framework/framework.jar:/system/framework/telephony-common.jar:/system/framework/voip-common.jar:/system/framework/ims-common.jar:/system/framework/android.hidl.base-V1.0-java.jar:/system/framework/android.hidl.manager-V1.0-java.jar:/system/framework/framework-oahl-backward-compatibility.jar:/system/framework/android.test.base.jar \
    SYSTEMSERVERCLASSPATH=/system/framework/services.jar:/system/framework/ethernet-service.jar:/system/framework/wifi-service.jar:/system/framework/com.android.location.provider.jar
