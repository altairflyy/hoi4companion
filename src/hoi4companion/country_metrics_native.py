"""External read-only effective war support for one exact executable.

No native calls. Context-dependent modifier paths fail closed.
Static provenance and runtime qualification limits: docs/country-metrics-live.md.
The layout requires explicit profile registration; importing it enables nothing.
"""
import hashlib
import struct

from .live import LiveReadError, MILITARY_HASH


COUNTRY_METRICS_LAYOUT = {'version': 1, 'path': 'effective-war-support'}
_FUNCTIONS = (
    (0x40BC70, 0x4F, '7c9967e9d99bf4cbe524ab9a4adb184b03910047726718d8f258e7389ab53727'),
    (0x741BE0, 0x15F8, '2ac5844aecbdb116f9f7c6990715b59bc1a51b1703d2846a2ff5840f394380fe'),
    (0x1F1FB0, 0x25, 'b0c01a4d42eefba0e06f60207c4f35c3d8521a769ff87bd67959f323268033e8'),
    (0x1DD900, 0x78, '2cb254c625879dd131bd760cea304f96f212b521285383928e47828667eb5c44'),
    (0x15B31D0, 0xBE, '85d9163d4c0835930cb8c5b0551eeeb23555c0bf769d14799ef003d49444c5b2'),
    (0x15B1A30, 0x31, 'f848324bd5bc0e134fa4d92ac9adfb836cbd74bf0d3e41e5761d051e0bab28ea'),
    (0x15B1A70, 0xC4, 'eb82c8a8242930ee3a18a12465cc2829a53877a44369f5a2573fd5c2c276fb36'),
    (0x15B2E50, 0x290, '236d80ad491aa5dd7ddd51dd624fa33170614c57f5e94d239a4cd4b3508e04c8'),
    (0x2572AE0, 0x57, 'cd50a39087b2738618b3ace767cd15a0b17adc2dbcaa3954d52a0fa626c8b1de'),
    (0x2572C70, 0x138, '60a7379faa0c45a2a46f40c8a69d0796e277e95803c9f27bc5cc8cf845ec41ca'),
    (0x5A51A0, 0x10B, '4de3048ac47a29e219da059ab711a07edf19e5cd81cdcc1eb93c6c53effdc37c'),
    (0xF81FC0, 0xEE, '5f668cf1d31e1daed2db7f973c89c86562730a7182567d589fdb6c50559fca62'),
    (0xDA4470, 0x87, '8a15912f99e6d9df42aa5b58a06000284f41b9d1e8a2a1758a71ae26d29a15cb'),
    (0xDA3190, 0x88, '0582baf2d9aae7cd7105d8aae2608c4476f6748d69c682acd202387b2acb98a1'),
    (0xDA2F50, 0x88, '0ab91fad6f1b8670df4e00e5c0bd5c969d2f61911da2c6d2c4e6c7b07fe78b75'),
    (0xD84980, 0x14, '60bc10f2876de35c5cf51b3d446e8a1540cdd88e633b0a5796416ae04855bfcb'),
    (0xD839D0, 0x14, '00e906037dc4abaddc92f4158e9e14b84f1f6eb182a4de339266100aa22689ab'),
    (0xC08580, 0x141, '24f9af80318ebe381cf2dc1a0c16b228c1ea2b1bb84204451ff9673ca0778903'),
)


def _trunc_div(value, divisor):
    return (1 if value >= 0 else -1) * (abs(value) // divisor)


def _to_q15(value):
    # 2572AE0: integer and fractional quotients both truncate toward zero.
    return _trunc_div(value, 100000) * 32768 + _trunc_div(
        (value - _trunc_div(value, 100000) * 100000) * 32768, 100000)


def _from_q15(value):
    # 2572C70: arithmetic shift plus rounded nonnegative remainder. This is
    # deliberately not float division or round(), especially for negatives.
    integer = value >> 15
    return integer * 100000 + ((value - integer * 32768) * 100000 + 16384) // 32768


def read_metrics(reader):
    from .country_native import read_section
    return read_section(reader,'metrics',function_signatures=_FUNCTIONS)
