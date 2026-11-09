#!/usr/bin/env python3
"""Pruebas sin red ni privilegios: paquetes, sudo, Rust y Docker son dobles locales."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

RAIZ = Path(__file__).resolve().parents[1]
DOBLE = r'''#!/usr/bin/python3
import json, os, pathlib, subprocess, sys
name=pathlib.Path(sys.argv[0]).name; args=sys.argv[1:]
def log(**more):
    with open(os.environ['MOCK_LOG'],'a') as f: f.write(json.dumps(dict(command=name,args=args,**more))+'\n')
installed=os.environ.get('MOCK_INSTALLED','').split()
if name=='uname':
    print({'-s':'Linux','-m':os.environ.get('MOCK_ARCH','x86_64'),'-r':'6.8.0-test'}.get(args[0] if args else '-s','Linux'))
elif name=='id':
    if args==['-u']: print('1000')
    elif args==['-un']: print('prueba')
    elif args and args[0]=='-nG': print(os.environ.get('MOCK_DATABASE_GROUPS' if len(args)>1 else 'MOCK_SESSION_GROUPS','users input docker'))
    else: sys.exit(1)
elif name=='getent':
    sys.exit(0 if args[-1] in os.environ.get('MOCK_GROUPS','input docker').split() else 2)
elif name in ('pacman','rpm','dpkg-query'):
    ok=args[-1] in installed
    if name=='dpkg-query' and ok: print('install ok installed',end='')
    sys.exit(0 if ok else 1)
elif name=='pactl': sys.exit(0 if os.environ.get('MOCK_AUDIO','0')=='1' else 1)
elif name=='powerprofilesctl': pass
elif name=='rustc':
    upgraded=pathlib.Path(os.environ['MOCK_RUST_STATE']).exists()
    print('rustc '+os.environ.get('MOCK_RUST_AFTER' if upgraded else 'MOCK_RUST_VERSION','1.95.0')+' (prueba)')
elif name=='rustup':
    log(); pathlib.Path(os.environ['MOCK_RUST_STATE']).touch()
elif name=='curl':
    log()
    if os.environ.get('MOCK_CURL_FAIL')=='1': sys.exit(22)
    # El supuesto instalador solo agrega un doble de rustup dentro de este directorio temporal.
    target=pathlib.Path(args[args.index('-o')+1])
    target.write_text('#!/bin/sh\nln -s "$MOCK_DISPATCH" "$MOCK_BIN/rustup"\n')
elif name=='sudo':
    more={}
    if args and args[0]=='install': more['content']=pathlib.Path(args[-2]).read_text()
    log(**more)
    if os.environ.get('MOCK_FAIL_WHEN') and os.environ['MOCK_FAIL_WHEN'] in ' '.join(args): sys.exit(1)
    if args[:2]==['apt-get','install'] and any(a.startswith('linux-modules-extra-') for a in args):
        pathlib.Path(os.environ['MOCK_MODULE_STATE']).touch()
    # Nunca ejecutar la orden privilegiada, ni siquiera install o modprobe.
elif name=='docker':
    log()
    if args==['info'] and os.environ.get('MOCK_DOCKER_DENIED')=='1': sys.exit(1)
elif name=='systemctl': log()
elif name=='modinfo':
    sys.exit(1 if os.environ.get('MOCK_MODULE_MISSING')=='1' and not pathlib.Path(os.environ['MOCK_MODULE_STATE']).exists() else 0)
elif name=='apt-cache': sys.exit(1 if os.environ.get('MOCK_EXTRA_UNAVAILABLE')=='1' else 0)
elif name=='grep':
    if '/proc/filesystems' in args: sys.exit(0)
    sys.exit(subprocess.run(['/usr/bin/grep',*args]).returncode)
elif name=='gcc':
    log(); pathlib.Path(args[args.index('-o')+1]).write_text('teclado de prueba')
elif name=='cargo':
    log(); out=pathlib.Path('target/release/zykro'); out.parent.mkdir(parents=True,exist_ok=True); out.write_text('binario de prueba')
else: raise SystemExit('Doble no implementado: '+name)
'''


class Instalador(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='zykro instalador ')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        self.dispatch = self.base / 'doble'
        self.dispatch.write_text(DOBLE)
        self.dispatch.chmod(0o755)
        for name in ['uname', 'id', 'getent', 'pacman', 'rpm', 'dpkg-query', 'pactl',
                     'powerprofilesctl', 'rustc', 'rustup', 'curl', 'sudo', 'docker',
                     'systemctl', 'modinfo', 'apt-cache', 'grep', 'gcc', 'cargo']:
            (self.bin / name).symlink_to(self.dispatch)
        self.home = self.base / 'usuario'
        self.home.mkdir()
        self.log = self.base / 'ordenes.jsonl'
        self.env = dict(os.environ, PATH=f'{self.bin}:/usr/bin:/bin', HOME=str(self.home),
                        CARGO_HOME=str(self.home / '.cargo'), MOCK_LOG=str(self.log),
                        MOCK_BIN=str(self.bin), MOCK_DISPATCH=str(self.dispatch),
                        MOCK_MODULE_STATE=str(self.base / 'modulo-instalado'),
                        MOCK_RUST_STATE=str(self.base / 'rust-estable'))
        self.env.pop('RUSTUP_TOOLCHAIN', None)

    def ejecutar(self, code, *, ok=True, cwd=RAIZ, **env):
        proc = subprocess.run(['/bin/sh', '-c', f'set -e; . "{RAIZ}/instalacion/sistema.sh"; {code}'],
                              cwd=cwd, env=dict(self.env, **env), text=True, capture_output=True)
        if ok:
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        else:
            self.assertNotEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        return proc

    def ordenes(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_detecta_familias_y_rechaza_una_desconocida(self):
        for ident, like, family in [('cachyos', 'arch', 'arch'), ('arch', '', 'arch'),
                                    ('ubuntu', 'debian', 'debian'), ('debian', '', 'debian'),
                                    ('fedora', '', 'fedora'), ('nobara', 'fedora', 'fedora')]:
            with self.subTest(ident=ident):
                release = self.base / 'os-release'
                release.write_text(f'ID={ident}\nID_LIKE="{like}"\n')
                p = self.ejecutar(f'zykro_distribucion "{release}"; echo "$ZYKRO_FAMILIA"')
                self.assertEqual(p.stdout.splitlines()[-1], family)
        release.write_text('ID=nixos\n')
        self.ejecutar(f'zykro_distribucion "{release}"', ok=False)
        self.assertFalse(self.ordenes())

    def test_arch_incluye_compilacion_graficos_docker_y_sonido(self):
        p = self.ejecutar('ZYKRO_FAMILIA=arch; zykro_paquetes 1; echo "$ZYKRO_PAQUETES"')
        packages = p.stdout.split()
        for package in ['gcc', 'pkgconf', 'glibc', 'libpulse', 'wayland', 'vulkan-intel',
                        'vulkan-radeon', 'docker', 'docker-buildx', 'pipewire-pulse', 'wireplumber']:
            self.assertIn(package, packages)

    def test_sin_android_no_instala_docker(self):
        p = self.ejecutar('ZYKRO_FAMILIA=arch; zykro_paquetes 0; echo "$ZYKRO_PAQUETES"')
        self.assertNotIn('docker', p.stdout.split())

    def test_simulacion_elige_el_selector_de_apk_del_escritorio(self):
        for familia in ('arch', 'debian', 'fedora'):
            for escritorio, selector, alternativo in [('ubuntu:GNOME', 'zenity', 'kdialog'),
                                                      ('gnome', 'zenity', 'kdialog'),
                                                      ('KDE', 'kdialog', 'zenity'),
                                                      ('', 'kdialog', 'zenity')]:
                with self.subTest(familia=familia, escritorio=escritorio):
                    p = self.ejecutar(f'ZYKRO_FAMILIA={familia}; zykro_paquetes 0; zykro_instalar_paquetes',
                                      XDG_CURRENT_DESKTOP=escritorio, ZYKRO_SIMULAR='1', PATH=str(self.bin))
                    paquetes = p.stdout.removeprefix('Paquetes que faltan: ').split()
                    self.assertIn(selector, paquetes)
                    self.assertNotIn(alternativo, paquetes)
                    self.assertIn('konsole', paquetes)
        self.assertFalse(self.ordenes())

    def test_conserva_pulseaudio_y_servidores_existentes(self):
        for env in [dict(MOCK_INSTALLED='pulseaudio'), dict(MOCK_AUDIO='1')]:
            p = self.ejecutar('ZYKRO_FAMILIA=arch; zykro_paquetes 1; echo "$ZYKRO_PAQUETES"', **env)
            self.assertNotIn('pipewire-pulse', p.stdout.split())

    def test_arch_solo_instala_lo_que_falta_sin_refresco_parcial(self):
        self.ejecutar('ZYKRO_FAMILIA=arch; ZYKRO_PAQUETES="gcc libpulse"; zykro_instalar_paquetes', MOCK_INSTALLED='gcc')
        self.assertEqual(self.ordenes(), [dict(command='sudo', args=['pacman', '-S', '--needed', '--noconfirm', 'libpulse'])])

    def test_no_reinstala_paquetes_presentes(self):
        self.ejecutar('ZYKRO_FAMILIA=arch; ZYKRO_PAQUETES="gcc libpulse"; zykro_instalar_paquetes', MOCK_INSTALLED='gcc libpulse')
        self.assertFalse(self.ordenes())

    def test_debian_actualiza_indices_y_luego_instala(self):
        self.ejecutar('ZYKRO_FAMILIA=debian; ZYKRO_PAQUETES="libpulse-dev"; zykro_instalar_paquetes')
        self.assertEqual([o['args'] for o in self.ordenes()], [
            ['apt-get', 'update'], ['apt-get', 'install', '-y', '--no-install-recommends', 'libpulse-dev']])

    def test_fallo_del_gestor_detiene_la_instalacion(self):
        self.ejecutar('ZYKRO_FAMILIA=debian; ZYKRO_PAQUETES=gcc; zykro_instalar_paquetes',
                      MOCK_FAIL_WHEN='apt-get update', ok=False)
        self.assertEqual(len(self.ordenes()), 1)

    def test_fedora_instala_glibc_estatica_y_moby(self):
        (self.bin / 'docker').unlink()
        p = self.ejecutar('ZYKRO_FAMILIA=fedora; zykro_paquetes 1; echo "$ZYKRO_PAQUETES"', PATH=str(self.bin))
        for package in ['glibc-static', 'moby-engine', 'docker-cli']:
            self.assertIn(package, p.stdout.split())
        self.ejecutar('ZYKRO_FAMILIA=fedora; ZYKRO_PAQUETES=glibc-static; zykro_instalar_paquetes')
        self.assertEqual(self.ordenes()[-1]['args'], ['dnf', 'install', '-y', 'glibc-static'])

    def test_compara_versiones_de_rust_numericamente(self):
        for actual, succeeds in [('1.95', True), ('1.95.0', True), ('1.96.1', True), ('1.100.0', True),
                                 ('2.0.0', True), ('1.94.9', False), ('1.9.0', False)]:
            self.ejecutar(f'zykro_version_rust {actual} 1.95', ok=succeeds)

    def test_rust_reciente_no_cambia_toolchain(self):
        p = self.ejecutar('zykro_rust; echo "${RUSTUP_TOOLCHAIN:-sin-cambio}"')
        self.assertIn('sin-cambio', p.stdout)
        self.assertFalse(self.ordenes())

    def test_rust_antiguo_usa_estable_sin_cambiar_el_predeterminado(self):
        p = self.ejecutar('zykro_rust; echo "$RUSTUP_TOOLCHAIN"', MOCK_RUST_VERSION='1.85.0', MOCK_RUST_AFTER='1.96.0')
        self.assertEqual(p.stdout.splitlines()[-1], 'stable')
        self.assertEqual(self.ordenes(), [dict(command='rustup', args=['toolchain', 'install', 'stable', '--profile', 'minimal'])])

    def test_bootstrap_de_rustup_sin_red_ni_escribir_fuera_del_usuario(self):
        (self.bin / 'rustup').unlink()
        self.ejecutar('zykro_rust', MOCK_RUST_VERSION='1.85.0', MOCK_RUST_AFTER='1.96.0')
        orders = self.ordenes()
        self.assertEqual([o['command'] for o in orders], ['curl', 'rustup'])
        self.assertIn('https://sh.rustup.rs', orders[0]['args'])
        self.assertFalse(any(o['command'] == 'sudo' for o in orders))

    def test_un_fallo_de_descarga_no_deja_instalador_de_rustup(self):
        (self.bin / 'rustup').unlink()
        self.ejecutar('zykro_rust', MOCK_RUST_VERSION='1.85.0', MOCK_CURL_FAIL='1', ok=False)
        curl = self.ordenes()[0]['args']
        self.assertFalse(Path(curl[curl.index('-o') + 1]).exists())

    def test_rechaza_rust_estable_si_todavia_es_demasiado_antiguo(self):
        self.ejecutar('zykro_rust', MOCK_RUST_VERSION='1.85.0', MOCK_RUST_AFTER='1.94.0', ok=False)

    def test_rechaza_arquitecturas_sin_imagen_android(self):
        p = self.ejecutar('zykro_entorno', MOCK_ARCH='aarch64', ok=False)
        self.assertIn('x86_64', p.stderr)
        self.assertFalse(self.ordenes())

    def test_permisos_agrega_grupos_sin_borrar_los_otros(self):
        p = self.ejecutar('zykro_permisos 1; echo "$ZYKRO_REINICIAR_SESION"',
                          MOCK_DATABASE_GROUPS='users', MOCK_SESSION_GROUPS='users')
        args = [o['args'] for o in self.ordenes()]
        self.assertIn(['usermod', '-aG', 'input', 'prueba'], args)
        self.assertIn(['usermod', '-aG', 'docker', 'prueba'], args)
        self.assertEqual(p.stdout.strip(), '1')
        rules = next(o['content'] for o in self.ordenes() if 'content' in o)
        self.assertIn('MODE="0660"', rules)
        self.assertNotIn('0666', rules)

    def test_permisos_no_repite_usermod_si_el_usuario_ya_pertenece(self):
        self.ejecutar('zykro_permisos 1')
        self.assertFalse(any(o['args'][0] in ['usermod', 'groupadd'] for o in self.ordenes()))

    def test_un_fallo_al_escribir_reglas_no_continua_con_udev(self):
        self.ejecutar('zykro_permisos 1', MOCK_FAIL_WHEN='install -Dm644', ok=False)
        self.assertFalse(any('udevadm' in o['args'] for o in self.ordenes()))

    def test_docker_con_permisos_pendientes_solo_eleva_docker(self):
        self.ejecutar('ZYKRO_DOCKER_SUDO=1; zykro_docker build -q .')
        self.assertEqual(self.ordenes(), [dict(command='sudo', args=['-H', 'docker', 'build', '-q', '.'])])

    def fixture(self):
        root = self.base / 'proyecto con espacios'
        root.mkdir()
        for name in ['instalar.sh', 'preparar-android.sh']:
            shutil.copy2(RAIZ / name, root / name)
        (root / 'instalacion').mkdir()
        release = root / 'os-release'
        release.write_text('ID=cachyos\nID_LIKE=arch\n')
        helper = (RAIZ / 'instalacion/sistema.sh').read_text().replace('/etc/os-release', str(release))
        (root / 'instalacion/sistema.sh').write_text(helper)
        for folder in ['zykro', 'src', 'bin', 'launcher', 'desktop', 'imagen']:
            (root / folder).mkdir()
        (root / 'zykro/Cargo.toml').write_text('rust-version = "1.95"\n')
        for name in ['src/teclado-fisico.c', 'bin/teclado-android', 'launcher/zykro-inicio.apk', 'desktop/zykro.svg']:
            (root / name).write_text('contenido de prueba')
        (root / 'desktop/zykro.desktop').write_text('Exec=@HOME@/.local/bin/zykro\n')
        (root / 'imagen/construir.sh').write_text('cd "$(dirname "$0")"\n. ../instalacion/sistema.sh\nzykro_docker build -q .\n')
        return root

    def test_simulacion_no_instala_no_descarga_ni_crea_archivos_del_usuario(self):
        root = self.fixture()
        p = subprocess.run(['sh', 'instalar.sh', '--simular'], cwd=root, env=self.env, text=True, capture_output=True)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn('Simulación', p.stdout)
        self.assertFalse(self.ordenes())
        self.assertFalse(list(self.home.iterdir()))

    def test_instalacion_completa_en_laptop_nueva_sin_tocar_el_sistema_real(self):
        root = self.fixture()
        # Simular que el grupo docker aún no se aplica a la sesión actual.
        env = dict(self.env, MOCK_DOCKER_DENIED='1', MOCK_DATABASE_GROUPS='users', MOCK_SESSION_GROUPS='users')
        p = subprocess.run(['sh', 'instalar.sh'], cwd=root, env=env, text=True, capture_output=True)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertEqual((self.home / '.local/bin/zykro').read_text(), 'binario de prueba')
        self.assertIn('cierra tu sesión', p.stdout)
        args = [o['args'] for o in self.ordenes() if o['command'] == 'sudo']
        self.assertTrue(any(a[0] == 'pacman' for a in args))
        self.assertIn(['-H', 'docker', 'build', '-q', '.'], args)
        self.assertIn(['build', '--release', '--locked'], [o['args'] for o in self.ordenes() if o['command'] == 'cargo'])

    def test_ubuntu_instala_el_modulo_binder_del_kernel_actual(self):
        root = self.fixture()
        (root / 'os-release').write_text('ID=ubuntu\nID_LIKE=debian\n')
        self.ejecutar(f'. "{root}/instalacion/sistema.sh"; zykro_binder_extra', MOCK_MODULE_MISSING='1')
        args = [o['args'] for o in self.ordenes()]
        self.assertIn(['apt-get', 'install', '-y', '--no-install-recommends', 'linux-modules-extra-6.8.0-test'], args)

    def test_kernel_sin_modulo_disponible_no_instala_otro_kernel(self):
        root = self.fixture()
        (root / 'os-release').write_text('ID=ubuntu\nID_LIKE=debian\n')
        self.ejecutar(f'. "{root}/instalacion/sistema.sh"; zykro_binder_extra',
                      MOCK_MODULE_MISSING='1', MOCK_EXTRA_UNAVAILABLE='1', ok=False)
        self.assertFalse(self.ordenes())

    def test_instalacion_sin_android_no_ejecuta_docker(self):
        root = self.fixture()
        p = subprocess.run(['sh', 'instalar.sh', '--sin-android'], cwd=root, env=self.env, text=True, capture_output=True)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertFalse(any(o['command'] == 'docker' or 'docker' in o['args'] for o in self.ordenes()))


if __name__ == '__main__':
    unittest.main(verbosity=2)
