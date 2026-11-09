"""Rust revalidation; no game offsets, identity inference, or evidence cache.

Wire v1: repeated <u64 address, u32 read_size, u32 part_count>, followed
by <u32 offset, u32 length, u32 byte_mask> and expected bytes for each part.
Full-byte masks are 255; GUI visibility witnesses compare bit 3 only.
"""
import ctypes
from functools import lru_cache
import os
from pathlib import Path
import struct

MAX_PACKET = 8 * 1024 * 1024
_HEADER = struct.Struct('<QII')
_PART = struct.Struct('<III')


def _append(packet, address, size, parts):
    if (type(address) is not int or not 0x10000 <= address < 0x7FFFFFFFFFFF
            or type(size) is not int or not 0 < size <= 65536
            or address + size > 0x7FFFFFFFFFFF or not parts or len(parts) > 131072):
        raise ValueError('Invalid native observation range')
    packet.extend(_HEADER.pack(address, size, len(parts)))
    for offset, expected, mask in parts:
        if (type(offset) is not int or offset < 0 or not isinstance(expected, bytes)
                or not expected or offset + len(expected) > size):
            raise ValueError('Invalid native observation part')
        packet.extend(_PART.pack(offset, len(expected), mask))
        packet.extend(expected)
        if len(packet) > MAX_PACKET:
            raise ValueError('Native observation exceeds limit')


def encode_sample(witnesses, visibility=None):
    packet = bytearray()
    for (address, size), expected in witnesses.items():
        if not isinstance(expected, bytes) or len(expected) != size:
            raise ValueError('Invalid native witness length')
        _append(packet, address, size, [(0, expected, 255)])
    for address, expected in (visibility or {}).items():
        if type(expected) is not int or expected not in (0, 8):
            raise ValueError('Invalid native visibility witness')
        _append(packet, address, 1, [(0, bytes([expected]), 8)])
    return bytes(packet)


def encode_watch(reads):
    packet = bytearray()
    for address, size, parts in reads:
        _append(packet, address, size, [(offset, expected, 255) for offset, expected in parts])
    return bytes(packet)


def _library_path():
    folder = Path(__file__).parent / '_native'
    dll = folder / 'hoi4companion_observer.dll'
    if dll.is_file():
        return dll
    return next(folder.glob('hoi4companion_observer*.pyd'), None)


class Observer:
    def __init__(self, path=None):
        selected = Path(path) if path else _library_path()
        if selected is None:
            raise OSError('Native observer is not built; run scripts/build-observer.ps1')
        self.library = ctypes.CDLL(str(selected.resolve(strict=True)))
        abi = self.library.hoi4_observer_abi_version
        abi.argtypes = []
        abi.restype = ctypes.c_uint32
        if abi() != 1:
            raise ValueError('Unsupported native observer ABI')
        self.compare = self.library.hoi4_observer_compare
        self.compare.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint32]
        self.compare.restype = ctypes.c_int32

    def changed(self, handle, packet, *, passes=1):
        if type(passes) is not int or passes not in (1, 2):
            raise ValueError('Invalid native observation pass count')
        if not isinstance(packet, bytes) or len(packet) > MAX_PACKET:
            raise ValueError('Invalid native observation packet')
        result = self.compare(handle, packet, len(packet), passes)
        if result not in (0, 1):
            reason = {-1: 'invalid request', -2: 'read failed or partial', -3: 'panic'}.get(result, 'invalid response')
            raise RuntimeError('Native observer: ' + reason)
        return result == 1


@lru_cache(maxsize=1)
def _load(path):
    return Observer(path)


def optional_observer():
    if os.name != 'nt':
        return None
    path = _library_path()
    # Only absence uses the original implementation. ABI/read failures never
    # fall back to a weaker observation, nor turn unreadable bytes into success.
    return _load(str(path)) if path else None


def self_check():
    """Execute the real DLL against an owned buffer, never a game process."""
    if os.name != 'nt':
        raise OSError('Native observation requires Windows')
    from ctypes import wintypes
    kernel = ctypes.WinDLL('kernel32')
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    buffer = ctypes.create_string_buffer(b'observer-check')
    packet = encode_sample({(ctypes.addressof(buffer), 14): b'observer-check'})
    observer = Observer()
    handle = kernel.GetCurrentProcess()
    if observer.changed(handle, packet, passes=2):
        raise RuntimeError('Native observer self-check rejected matching bytes')
    buffer[0] = b'X'
    if not observer.changed(handle, packet, passes=2):
        raise RuntimeError('Native observer self-check missed changed bytes')
    return {'engine': 'rust', 'abi_version': 1, 'read_only': True,
            'owned_buffer_verified': True, 'game_live_verified': False}
