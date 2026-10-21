#!/usr/bin/python3
"""Aplica nuestro parche a libhoudini.so (traductor ARM -> x86 de Android).

Reemplaza un salto condicional (`jae`, 6 bytes) por NOPs. No subimos la
librería al repositorio porque es de Intel: se saca de la imagen base y se
parchea aquí.
"""
import sys

OFFSET = 942213
ORIGINAL = bytes.fromhex('0f83680d0000')  # jae rel32
PARCHE = b'\x90' * len(ORIGINAL)          # nop x6

ruta = sys.argv[1] if len(sys.argv) > 1 else 'libhoudini.so'
datos = bytearray(open(ruta, 'rb').read())
actual = bytes(datos[OFFSET:OFFSET + len(ORIGINAL)])
if actual == PARCHE:
    print('libhoudini.so ya estaba parcheado')
elif actual == ORIGINAL:
    datos[OFFSET:OFFSET + len(ORIGINAL)] = PARCHE
    open(ruta, 'wb').write(datos)
    print('libhoudini.so parcheado')
else:
    sys.exit(f'bytes inesperados en {OFFSET}: {actual.hex()} (¿otra versión de libhoudini?)')
