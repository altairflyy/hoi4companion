"""Exact-build, external read-only doctrine selection membership.

Mirrors CHasDoctrineTrigger's selected grand/subdoctrine pointers, not unlocked
or available definitions. No engine calls; see docs/doctrines-live.md.
"""
import hashlib
import struct

from .live import LiveReadError, MILITARY_HASH


_FUNCTIONS = (
    (0x572A30, 290, 'b3492d37117969070e59e594bf1ab16ee74661ff9d5eb9853d108d8af49b8066'),
    (0x1460C71, 27, '04b5b9b6d46a89a23923f7fe5650a5fec917b6a31b2e43d1729122c9e07fc632'),
    (0xDE4EB0, 55, 'f38fdc1da64da292cbc1bc4f8568c2e6464ab7cc19a53c7c9f9cf4bf170c2ee8'),
    (0xDE2099, 41, '95d18c3ed69e70881c97e20f519b3b0b149f946d6c715d22d6ddcd6b4bb4d8da'),
    (0x253EB99, 23, '318cb0f871a1f97ad8c157034a0e3c07dc650a9f9a1d5469080b5a6377be3c77'),
    (0x253EC1F, 11, '14b60f1d321877cd236aaf39e72fc8c4fe7365aa4a05d1e69596df088e279547'),
)


def read_doctrines(reader):
    from .country_native import read_section
    return read_section(reader,'doctrines',function_signatures=_FUNCTIONS)
