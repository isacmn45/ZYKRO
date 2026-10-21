#!/usr/bin/python3
"""Prueba del protocolo de swag-rust-server sin SwagPlayer (ver docs/investigacion.md).

  prueba_protocolo.py captura salida.png   -> un cuadro por video.sock (DMA-BUF sin comprimir)
  prueba_protocolo.py toque X Y            -> un toque por control.sock (mensaje 0x02 de scrcpy)

SwagPlayer tiene que estar cerrado: el servidor solo acepta un cliente por socket.
"""
import array
import fcntl
import mmap
import os
import socket
import struct
import sys
import zlib

SOCKETS = '/tmp/swag_sockets'
DMA_BUF_IOCTL_SYNC = 0x40086200
SYNC_READ, SYNC_START, SYNC_END = 1, 0, 4


def recibir(sock, tam, max_fds=4):
    fds = array.array('i')
    datos, anc, _, _ = sock.recvmsg(tam, socket.CMSG_SPACE(max_fds * fds.itemsize))
    for nivel, tipo, d in anc:
        if nivel == socket.SOL_SOCKET and tipo == socket.SCM_RIGHTS:
            fds.frombytes(d[:len(d) - len(d) % fds.itemsize])
    return datos, list(fds)


def guardar_png(ruta, ancho, alto, filas):
    crudo = b''.join(b'\x00' + f for f in filas)

    def bloque(tipo, datos):
        return struct.pack('>I', len(datos)) + tipo + datos + struct.pack('>I', zlib.crc32(tipo + datos))

    with open(ruta, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(bloque(b'IHDR', struct.pack('>IIBBBBB', ancho, alto, 8, 6, 0, 0, 0)))
        f.write(bloque(b'IDAT', zlib.compress(crudo, 6)))
        f.write(bloque(b'IEND', b''))


def captura(ruta):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(f'{SOCKETS}/video.sock')
    primero = s.recv(1, socket.MSG_PEEK)
    if primero != b'\xfd':
        sys.exit(f'el servidor no está en modo DMA-BUF (primer byte {primero.hex()})')
    cab = b''
    while len(cab) < 21:
        cab += s.recv(21 - len(cab))
    magic, ancho, alto, formato, extra = struct.unpack('>4sIIII', cab[1:])
    print(f'cabecera: {magic.decode()} {ancho}x{alto} formato={formato} extra={extra}')

    buffers = {}
    for _ in range(200):
        datos, fds = recibir(s, 64)
        if not datos:
            sys.exit('el servidor cerró la conexión')
        magic, bid = struct.unpack_from('>4sI', datos)
        if magic == b'DMBR':
            w, h, stride, tam, fmt, aw, ah = struct.unpack_from('>IIIIIII', datos, 8)
            mod = struct.unpack_from('>Q', datos, 36)[0] if len(datos) >= 44 else 0
            print(f'DMBR id={bid} {w}x{h} stride={stride} tam={tam} formato={fmt} '
                  f'asig={aw}x{ah} modificador={mod:#x} fds={len(fds)} bytes={len(datos)}')
            buffers[bid] = (fds[0], w, h, stride, tam, fmt, mod)
            for fd in fds[1:]:
                os.close(fd)
        elif magic == b'DMBX':
            for fd in fds:  # fence: no la esperamos, solo la cerramos
                os.close(fd)
            if bid not in buffers:
                continue
            fd, w, h, stride, tam, fmt, mod = buffers[bid]
            if mod not in (0, 0xffffffffffffff):
                print(f'aviso: modificador {mod:#x} no lineal, la imagen puede salir revuelta')
            m = mmap.mmap(fd, tam, mmap.MAP_SHARED, mmap.PROT_READ)
            fcntl.ioctl(fd, DMA_BUF_IOCTL_SYNC, struct.pack('Q', SYNC_START | SYNC_READ))
            filas = [m[y * stride:y * stride + w * 4] for y in range(h)]
            fcntl.ioctl(fd, DMA_BUF_IOCTL_SYNC, struct.pack('Q', SYNC_END | SYNC_READ))
            if fmt == 5:  # BGRA -> RGBA
                filas = [bytes(b for i in range(0, len(f), 4) for b in (f[i + 2], f[i + 1], f[i], f[i + 3]))
                         for f in filas]
            guardar_png(ruta, w, h, filas)
            print(f'DMBX id={bid}: cuadro guardado en {ruta}')
            return
        else:
            print(f'paquete desconocido {magic!r} ({len(datos)} bytes)')
    sys.exit('no llegó ningún cuadro')


def toque(x, y, ancho=1280, alto=720):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(f'{SOCKETS}/control.sock')
    layout = b'keyboard_layout_english_us'
    s.send(bytes([0x12, len(layout)]) + layout, socket.MSG_NOSIGNAL)
    s.send(struct.pack('>BBf', 0x14, 1, 0.5), socket.MSG_NOSIGNAL)
    for accion, presion in ((0, 0xFFFF), (1, 0)):  # bajar, soltar
        s.send(struct.pack('>BBQiiHHHII', 0x02, accion, 0, x, y, ancho, alto, presion, 0, 0),
               socket.MSG_NOSIGNAL)
    s.close()
    print(f'toque enviado en {x},{y}')


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == 'captura':
        captura(sys.argv[2])
    elif len(sys.argv) == 4 and sys.argv[1] == 'toque':
        toque(int(sys.argv[2]), int(sys.argv[3]))
    else:
        sys.exit(__doc__)
