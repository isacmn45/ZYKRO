# Imagen de Android de ZYKRO (la arma construir.sh): redroid 13, de código abierto, con:
# - traducción de apps ARM y ARM64 (Houdini 14, con el parche de Free Fire),
# - Google Play (MindTheGapps; Play y sus servicios se actualizan solos),
# - el servidor de ZYKRO (servidor/): pantalla, toques y sonido,
# - las superposiciones de la interfaz (superposiciones/),
# - y propiedades de un teléfono normal: compilación «user», sin depuración ni root.
# Las descargas las verifica construir.sh; aquí solo se acomodan.
FROM alpine:3.20 AS piezas
RUN apk add --no-cache libarchive-tools busybox-static
COPY --from=redroid/redroid:13.0.0-latest /system/build.prop /tmp/props/system.prop
COPY --from=redroid/redroid:13.0.0-latest /system/system_ext/etc/build.prop /tmp/props/system_ext.prop
COPY --from=redroid/redroid:13.0.0-latest /system/product/etc/build.prop /tmp/props/product.prop
COPY --from=redroid/redroid:13.0.0-latest /vendor/build.prop /tmp/props/vendor.prop
COPY --from=redroid/redroid:13.0.0-latest /vendor/odm/etc/build.prop /tmp/props/odm.prop
COPY descargas/houdini13.zip descargas/gapps13.zip armar.sh /tmp/
RUN sh /tmp/armar.sh

FROM redroid/redroid:13.0.0-latest
# Sin «su»: los juegos con antitrampas lo toman por un teléfono rooteado. Se borra con un busybox
# estático, que se borra a sí mismo (Android no tiene shell fuera de Android).
COPY --from=piezas /bin/busybox.static /busybox
RUN ["/busybox", "rm", "-f", "/system/xbin/su", "/busybox"]
COPY --from=piezas /salida/system/ /system/
COPY --from=piezas /salida/vendor/ /vendor/
COPY zykro-servidor.jar /system/framework/zykro-servidor.jar
COPY zykro-servidor.rc /system/etc/init/zykro-servidor.rc
COPY zykro-houdini.rc /system/etc/init/zykro-houdini.rc
COPY zykro-houdini.sh /system/etc/zykro-houdini.sh
COPY configuracion.apk /system/product/overlay/ZykroConfiguracion.apk
COPY sistema.apk /system/product/overlay/ZykroSistema.apk
COPY marco.apk /system/product/overlay/ZykroMarco.apk
