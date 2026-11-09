#!/bin/sh
# Funciones compartidas del instalador. Importar este archivo no cambia el sistema.

zykro_error() { printf 'ZYKRO: %s\n' "$*" >&2; return 1; }

zykro_entorno() {
    [ "$(uname -s)" = Linux ] || { zykro_error 'El instalador necesita Linux.'; return 1; }
    [ "$(uname -m)" = x86_64 ] || { zykro_error 'La imagen de Android necesita una laptop x86_64 (Intel o AMD).'; return 1; }
    [ "$(id -u)" != 0 ] || { zykro_error 'Ejecuta ./instalar.sh con tu usuario, sin sudo; se pedirá la contraseña cuando haga falta.'; return 1; }
}

zykro_distribucion() {
    # El argumento permite probar la detección con archivos de ejemplo.
    [ -r "${1:-/etc/os-release}" ] || { zykro_error 'No se pudo identificar la distribución.'; return 1; }
    ID= ID_LIKE= PRETTY_NAME=
    . "${1:-/etc/os-release}"
    case " $ID $ID_LIKE " in
        *' arch '*) ZYKRO_FAMILIA=arch ;;
        *' debian '*|*' ubuntu '*) ZYKRO_FAMILIA=debian ;;
        *' fedora '*) ZYKRO_FAMILIA=fedora ;;
        *) zykro_error "Instalación automática no disponible para ${PRETTY_NAME:-$ID}. Puedes preparar las dependencias y usar --sin-dependencias."; return 1 ;;
    esac
    printf 'Distribución: %s\n' "${PRETTY_NAME:-$ID}"
}

zykro_instalado() {
    case "$ZYKRO_FAMILIA" in
        arch) pacman -Q "$1" >/dev/null 2>&1 ;;
        debian) [ "$(dpkg-query -W -f='${Status}' "$1" 2>/dev/null)" = 'install ok installed' ] ;;
        fedora) rpm -q "$1" >/dev/null 2>&1 ;;
    esac
}

zykro_raiz() {
    command -v sudo >/dev/null 2>&1 || { zykro_error 'Hace falta sudo para instalar los paquetes y preparar los dispositivos.'; return 1; }
    sudo "$@"
}

zykro_paquetes() {
    # Los APK y el servidor Java ya vienen compilados: no hacen falta Java ni el SDK de Android.
    case "$ZYKRO_FAMILIA" in
        arch)
            ZYKRO_PAQUETES='gcc make binutils pkgconf glibc git curl ca-certificates kmod util-linux shadow
                wayland libxkbcommon libxkbcommon-x11 libx11 libxcursor libxi libxrandr
                mesa libglvnd vulkan-icd-loader vulkan-intel vulkan-radeon libpulse
                xdg-utils xdg-user-dirs desktop-file-utils libnotify pciutils polkit
                konsole imagemagick android-tools'
            [ "$1" = 0 ] || ZYKRO_PAQUETES="$ZYKRO_PAQUETES docker docker-buildx"
            ZYKRO_PULSE=pipewire-pulse
            ZYKRO_AUDIO='pipewire pipewire-pulse wireplumber'
            ;;
        debian)
            ZYKRO_PAQUETES='build-essential pkg-config libc6-dev git curl ca-certificates kmod util-linux passwd
                libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev
                libegl-dev libegl-mesa0 libgl1-mesa-dri mesa-vulkan-drivers libpulse-dev pulseaudio-utils
                xdg-utils xdg-user-dirs desktop-file-utils libnotify-bin pciutils pkexec
                konsole imagemagick adb'
            # No mezclar docker.io con una instalación existente de Docker CE.
            if [ "$1" != 0 ] && ! command -v docker >/dev/null 2>&1; then
                ZYKRO_PAQUETES="$ZYKRO_PAQUETES docker.io"
            fi
            ZYKRO_PULSE=pipewire-pulse
            ZYKRO_AUDIO='pipewire pipewire-pulse wireplumber'
            ;;
        fedora)
            ZYKRO_PAQUETES='gcc gcc-c++ make binutils pkgconf-pkg-config glibc-static git curl ca-certificates kmod util-linux shadow-utils
                wayland-devel libxkbcommon-devel libxkbcommon-x11-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel
                mesa-libEGL-devel mesa-dri-drivers mesa-vulkan-drivers pulseaudio-libs-devel pulseaudio-utils
                xdg-utils xdg-user-dirs desktop-file-utils libnotify pciutils polkit
                konsole ImageMagick android-tools'
            if [ "$1" != 0 ] && ! command -v docker >/dev/null 2>&1; then
                ZYKRO_PAQUETES="$ZYKRO_PAQUETES moby-engine docker-cli docker-buildx"
            fi
            ZYKRO_PULSE=pipewire-pulseaudio
            ZYKRO_AUDIO='pipewire pipewire-pulseaudio wireplumber'
            ;;
    esac
    # El selector de APK prefiere Zenity en GNOME y KDialog en el resto, como la app.
    case ":${XDG_CURRENT_DESKTOP:-}:" in
        *:[Gg][Nn][Oo][Mm][Ee]:*) ZYKRO_PAQUETES="$ZYKRO_PAQUETES zenity" ;;
        *) ZYKRO_PAQUETES="$ZYKRO_PAQUETES kdialog" ;;
    esac
    # Conservar el servidor de sonido que ya tenga el escritorio.
    ZYKRO_AUDIO_NUEVO=0
    if ! zykro_instalado "$ZYKRO_PULSE" && ! zykro_instalado pulseaudio; then
        if ! command -v pactl >/dev/null 2>&1 || ! pactl info >/dev/null 2>&1; then
            ZYKRO_PAQUETES="$ZYKRO_PAQUETES $ZYKRO_AUDIO"
            ZYKRO_AUDIO_NUEVO=1
        fi
    fi
    if ! command -v powerprofilesctl >/dev/null 2>&1; then
        ZYKRO_PAQUETES="$ZYKRO_PAQUETES power-profiles-daemon"
    fi
}

zykro_instalar_paquetes() {
    # Acumular argumentos, sin eval: los nombres nunca se interpretan como órdenes.
    set --
    for zykro_pkg in $ZYKRO_PAQUETES; do
        # Fedora puede traer curl-minimal, que ya proporciona la herramienta que necesitamos.
        if [ "$zykro_pkg" = curl ] && command -v curl >/dev/null 2>&1; then
            continue
        fi
        zykro_instalado "$zykro_pkg" || set -- "$@" "$zykro_pkg"
    done
    if [ "$#" = 0 ]; then
        echo 'Dependencias del sistema: listas.'
        return
    fi
    printf 'Paquetes que faltan: %s\n' "$*"
    [ "${ZYKRO_SIMULAR:-0}" = 0 ] || return 0
    case "$ZYKRO_FAMILIA" in
        # No refrescar solo la base de datos (-Sy): Arch no admite actualizaciones parciales.
        arch) zykro_raiz pacman -S --needed --noconfirm "$@" ;;
        debian)
            zykro_raiz apt-get update || return 1
            zykro_raiz apt-get install -y --no-install-recommends "$@"
            ;;
        fedora) zykro_raiz dnf install -y "$@" ;;
    esac
}

zykro_version_rust() {
    # Versiones major.minor.patch; 1.95 y 1.95.0 son equivalentes.
    awk -v actual="$1" -v minima="$2" 'BEGIN {
        split(actual,a,"."); split(minima,b,".");
        for(i=1;i<=3;i++) { if(a[i]+0>b[i]+0) exit 0; if(a[i]+0<b[i]+0) exit 1 }
    }'
}

zykro_rust() {
    zykro_rust_minima=$(sed -n 's/^rust-version = "\([^"]*\)"/\1/p' zykro/Cargo.toml)
    [ -n "$zykro_rust_minima" ] || { zykro_error 'Falta la versión mínima de Rust en Cargo.toml.'; return 1; }
    if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        zykro_rust_actual=$(rustc --version 2>/dev/null | awk '{print $2}')
        if [ -n "$zykro_rust_actual" ] && zykro_version_rust "$zykro_rust_actual" "$zykro_rust_minima"; then
            printf 'Rust: %s (listo).\n' "$zykro_rust_actual"
            return
        fi
    fi
    printf 'Se preparará Rust estable, versión %s o superior, para tu usuario.\n' "$zykro_rust_minima"
    [ "${ZYKRO_SIMULAR:-0}" = 0 ] || return 0
    PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    export PATH
    if ! command -v rustup >/dev/null 2>&1; then
        zykro_rust_instalador=$(mktemp) || return 1
        if ! curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$zykro_rust_instalador"; then
            rm -f "$zykro_rust_instalador"
            return 1
        fi
        if ! sh "$zykro_rust_instalador" -y --profile minimal --no-modify-path --default-toolchain none; then
            rm -f "$zykro_rust_instalador"
            return 1
        fi
        rm -f "$zykro_rust_instalador"
    fi
    rustup toolchain install stable --profile minimal || return 1
    # Solo esta instalación usa stable; no cambiar el toolchain predeterminado del usuario.
    RUSTUP_TOOLCHAIN=stable
    export RUSTUP_TOOLCHAIN
    zykro_rust_actual=$(rustc --version | awk '{print $2}')
    zykro_version_rust "$zykro_rust_actual" "$zykro_rust_minima" || {
        zykro_error "Rust estable ($zykro_rust_actual) no alcanza la versión $zykro_rust_minima."; return 1;
    }
}

zykro_archivo_sistema() {
    # Lee el contenido por stdin y solo reemplaza el archivo si cambió.
    zykro_archivo_tmp=$(mktemp) || return 1
    if ! cat > "$zykro_archivo_tmp"; then
        rm -f "$zykro_archivo_tmp"; return 1
    fi
    if ! cmp -s "$zykro_archivo_tmp" "$1"; then
        if ! zykro_raiz install -Dm644 "$zykro_archivo_tmp" "$1"; then
            rm -f "$zykro_archivo_tmp"; return 1
        fi
    fi
    rm -f "$zykro_archivo_tmp"
}

zykro_permisos() {
    zykro_usuario=$(id -un)
    zykro_grupos=input
    [ "$1" = 0 ] || zykro_grupos='input docker'
    for zykro_grupo in $zykro_grupos; do
        getent group "$zykro_grupo" >/dev/null || zykro_raiz groupadd "$zykro_grupo" || return 1
        if ! id -nG "$zykro_usuario" | tr ' ' '\n' | grep -qx "$zykro_grupo"; then
            zykro_raiz usermod -aG "$zykro_grupo" "$zykro_usuario" || return 1
        fi
        if ! id -nG | tr ' ' '\n' | grep -qx "$zykro_grupo"; then
            ZYKRO_REINICIAR_SESION=1
        fi
    done
    zykro_archivo_sistema /etc/udev/rules.d/70-zykro-input.rules <<'FIN' || return 1
# Ratón y mandos para el grupo input; uinput para el mando virtual de ZYKRO.
SUBSYSTEM=="input", KERNEL=="event*", GROUP="input", MODE="0660"
SUBSYSTEM=="misc", KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"
FIN
    zykro_raiz udevadm control --reload-rules || return 1
    zykro_raiz udevadm trigger --subsystem-match=input || return 1
    zykro_raiz udevadm trigger --subsystem-match=misc --sysname-match=uinput
}

zykro_dependencias() {
    zykro_entorno || return 1
    zykro_distribucion || return 1
    zykro_paquetes "$1"
    zykro_instalar_paquetes || return 1
    zykro_rust || return 1
    if [ "${ZYKRO_SIMULAR:-0}" = 1 ]; then
        echo 'Se prepararán los permisos del ratón y mando; Android también necesita Docker, binder y uinput.'
        return
    fi
    zykro_permisos "$1" || return 1
    if [ "$ZYKRO_AUDIO_NUEVO" = 1 ]; then
        systemctl --user enable --now pipewire.socket pipewire-pulse.socket wireplumber.service || {
            echo 'El sonido de PipeWire quedará disponible al volver a iniciar sesión.'
        }
    fi
}

zykro_docker() {
    if [ "${ZYKRO_DOCKER_SUDO:-0}" = 1 ]; then
        zykro_raiz -H docker "$@"
    else
        command docker "$@"
    fi
}

zykro_binder_extra() {
    # En Ubuntu, algunos kernels distribuyen binder en este paquete separado.
    modinfo binder_linux >/dev/null 2>&1 && return 0
    zykro_distribucion || return 1
    if [ "$ID" = ubuntu ] || { [ "$ZYKRO_FAMILIA" = debian ] && echo " $ID_LIKE " | grep -qw ubuntu; }; then
        zykro_extra="linux-modules-extra-$(uname -r)"
        if apt-cache show "$zykro_extra" >/dev/null 2>&1; then
            ZYKRO_PAQUETES=$zykro_extra
            zykro_instalar_paquetes || return 1
        fi
    fi
    modinfo binder_linux >/dev/null 2>&1
}
