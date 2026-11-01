#!/system/bin/sh
# Lo corre init (zykro-houdini.rc): suma las rutas de Houdini a la lista de rutas permitidas.
f=/linkerconfig/ld.config.txt
grep -q 'permitted.paths += /system/${LIB}/arm64$' $f && exit 0
sed -i '/^namespace\.default\.permitted\.paths = \/system\/${LIB}\/drm$/a\
namespace.default.permitted.paths += /system/${LIB}/arm64\
namespace.default.permitted.paths += /system/${LIB}/arm64/nb\
namespace.default.permitted.paths += /system/${LIB}/arm\
namespace.default.permitted.paths += /system/${LIB}/arm/nb' $f
