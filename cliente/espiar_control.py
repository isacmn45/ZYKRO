#!/usr/bin/python3
"""Anota los mensajes que SwagPlayer manda a Android por control.sock (toques, teclas, scroll).

Se pone en medio: aparta el socket real del servidor y escucha en su lugar, reenviando todo.
Uso: córrelo ANTES de abrir SwagPlayer; deja de escuchar al cerrar SwagPlayer o con Ctrl+C.

  espiar_control.py [archivo.log]
"""
import os
import socket
import struct
import sys
import threading
import time

DIR = '/tmp/swag_sockets'
REAL, FALSO = f'{DIR}/control.sock', f'{DIR}/control.real.sock'
ACCIONES = {0: 'bajar', 1: 'soltar', 2: 'mover'}


def esperar_socket():
    """Espera a que el servidor de Android cree control.sock y lo aparta."""
    viejo = os.stat(REAL).st_ino if os.path.exists(REAL) else None
    print('Esperando a que Android cree control.sock… (abre SwagPlayer ahora)')
    while True:
        try:
            st = os.stat(REAL)
            if st.st_ino != viejo and not os.path.exists(FALSO):
                os.rename(REAL, FALSO)
                return
        except FileNotFoundError:
            pass
        time.sleep(0.01)


def decodificar(buf, log, t0):
    """Saca mensajes completos del búfer y los anota. Devuelve lo que sobra."""
    while buf:
        tipo = buf[0]
        largos = {0x00: 14, 0x02: 32, 0x03: 21, 0x14: 6}
        if tipo == 0x12:
            if len(buf) < 2 or len(buf) < 2 + buf[1]:
                return buf
            n = 2 + buf[1]
            texto = f'layout {buf[2:n].decode(errors="replace")}'
        elif tipo in largos:
            n = largos[tipo]
            if len(buf) < n:
                return buf
            m = buf[:n]
            if tipo == 0x02:
                accion, dedo, x, y, w, h, presion, _, _ = struct.unpack('>BQiiHHHII', m[1:])
                texto = f'toque {ACCIONES.get(accion, accion):6} dedo={dedo:<20} x={x:5} y={y:5} pantalla={w}x{h} presión={presion}'
            elif tipo == 0x00:
                accion, codigo, rep, meta = struct.unpack('>BIII', m[1:])
                texto = f'tecla {ACCIONES.get(accion, accion):6} código={codigo} meta={meta:#x}'
            elif tipo == 0x03:
                x, y, w, h, hs, vs, b = struct.unpack('>iiHHhhI', m[1:])
                texto = f'scroll x={x} y={y} h={hs} v={vs}'
            else:
                activo, valor = struct.unpack('>Bf', m[1:])
                texto = f'0x14 activo={activo} valor={valor:.2f}'
        else:
            texto = f'DESCONOCIDO tipo={tipo:#04x} bytes={buf[:64].hex()}'
            n = len(buf)
        log.write(f'{time.time() - t0:9.3f}  {texto}\n')
        buf = buf[n:]
    return buf


def main():
    ruta_log = sys.argv[1] if len(sys.argv) > 1 else 'control.log'
    esperar_socket()
    oyente = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        oyente.bind(REAL)
        os.chmod(REAL, 0o666)
        oyente.listen(1)
        print('Listo: esperando a SwagPlayer…')
        cliente, _ = oyente.accept()
        servidor = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        servidor.connect(FALSO)
        print(f'SwagPlayer conectado. Anotando en {ruta_log}. Juega y luego cierra SwagPlayer.')

        def de_vuelta():  # el servidor casi nunca responde, pero por si acaso
            try:
                while (d := servidor.recv(65536)):
                    cliente.sendall(d)
            except OSError:
                pass

        threading.Thread(target=de_vuelta, daemon=True).start()
        t0 = time.time()
        buf = b''
        with open(ruta_log, 'w', buffering=1) as log:
            while (d := cliente.recv(65536)):
                servidor.sendall(d)
                buf = decodificar(buf + d, log, t0)
    except KeyboardInterrupt:
        pass
    finally:
        oyente.close()
        if os.path.exists(FALSO):
            if os.path.exists(REAL):
                os.unlink(REAL)
            os.rename(FALSO, REAL)
        print('Terminado; control.sock restaurado.')


if __name__ == '__main__':
    main()
