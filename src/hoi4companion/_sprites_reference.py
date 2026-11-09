"""Sprite originali locali: .gfx -> texture -> PNG, senza copiare asset nel sorgente."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from io import BytesIO
from pathlib import Path, PurePosixPath
import struct
from threading import RLock

from .clausewitz import Node
from .jomini_parser import parse_file
from .installation import collect_files

try:
    from PIL import Image, UnidentifiedImageError
except ImportError:
    Image = None
    UnidentifiedImageError = OSError


@dataclass(frozen=True)
class Sprite:
    name: str
    texture: str
    frames: int = 1


def _fields(node: Node) -> dict:
    return {k.casefold(): v for k, v in node.entries}


def _definitions(node: Node):
    for key, value in node.entries:
        if not isinstance(value, Node):
            continue
        if key.casefold() in ('spritetype', 'frameanimatedspritetype', 'corneredtilespritetype'):
            fields = _fields(value)
            name, texture = fields.get('name'), fields.get('texturefile')
            if isinstance(name, str) and isinstance(texture, str):
                try:
                    frames = int(fields.get('noofframes', '1'))
                except (ValueError, TypeError):
                    continue
                if 1 <= frames <= 256:
                    yield Sprite(name, texture, frames)
        else:
            yield from _definitions(value)


class SpriteLibrary:
    def __init__(self, game: Path, extra_roots: list[Path], cache: Path):
        self.roots = [game.resolve(), *(p.resolve() for p in extra_roots)]
        self.cache = cache
        self.sprites: dict[str, Sprite] = {}
        self.assets: dict[str, tuple[Path, int, int]] = {}
        self.warnings: list[str] = []
        self.lock = RLock()
        self.failed: set[str] = set()
        self.replacements: dict[Path, list[str]] = {}
        for root in self.roots[1:]:
            descriptor = root / 'descriptor.mod'
            if descriptor.is_file():
                try:
                    self.replacements[root] = [str(PurePosixPath(x.replace('\\', '/'))).casefold()
                                               for x in parse_file(descriptor).all('replace_path')
                                               if isinstance(x, str)]
                except (OSError, ValueError, RecursionError):
                    self.warnings.append(f'Replace path delle texture non leggibile: {descriptor.name}')
        files, warnings = collect_files(game, extra_roots, 'interface', '.gfx')
        self.warnings += warnings
        for file in files:
            try:
                definitions = _definitions(parse_file(file))
                self.sprites.update((sprite.name, sprite) for sprite in definitions)
            except (OSError, ValueError, RecursionError) as exc:
                self.warnings.append(f'Sprite non letti ({file.name}): {exc}')
        self.resolved: dict[tuple[str, int], dict | None] = {}

    def _texture(self, relative: str) -> Path | None:
        relative = relative.replace('\\', '/')
        path = PurePosixPath(relative)
        if path.is_absolute() or '..' in path.parts or ':' in relative:
            return None
        if path.suffix.casefold() not in ('.dds', '.png', '.tga', '.bmp'):
            return None
        # I file correnti includono .gfx con estensione .png ma texture .dds.
        alternatives = [path]
        if path.suffix.casefold() != '.dds':
            alternatives.append(path.with_suffix('.dds'))
        for root in reversed(self.roots):
            for candidate in alternatives:
                file = (root / str(candidate)).resolve()
                if file.is_relative_to(root) and file.is_file():
                    return file
            if any(str(path).casefold() == prefix or str(path).casefold().startswith(prefix + '/')
                   for prefix in self.replacements.get(root, [])):
                return None
        return None

    def resolve(self, name: str, frame: int = 1) -> dict | None:
        with self.lock:
            key = (name, frame)
            if key in self.resolved:
                return self.resolved[key]
            sprite = self.sprites.get(name)
            if not sprite or Image is None or not 1 <= frame <= sprite.frames:
                self.resolved[key] = None
                return None
            file = self._texture(sprite.texture)
            if not file:
                self.resolved[key] = None
                return None
            stat = file.stat()
            identity = f'{file}|{stat.st_size}|{stat.st_mtime_ns}|{sprite.frames}|{frame}|png-v1'
            token = hashlib.sha256(identity.encode('utf-8')).hexdigest()
            self.assets[token] = (file, sprite.frames, frame)
            value = {'name': name, 'url': f'/api/sprite/{token}.png', 'frame': frame,
                     'frames': sprite.frames, 'source': sprite.texture,
                     'override': any(file.is_relative_to(root) for root in self.roots[1:])}
            self.resolved[key] = value
            return value

    def png(self, token: str) -> bytes | None:
        # Nessun percorso HTTP viene trasformato in un percorso del filesystem.
        with self.lock:
            if token not in self.assets or token in self.failed or Image is None:
                return None
            file, frames, frame = self.assets[token]
            cached = self.cache / (token + '.png')
            if cached.is_file():
                return cached.read_bytes()
            try:
                if file.stat().st_size > 32_000_000:
                    raise ValueError('Texture troppo grande')
                raw = file.read_bytes()
                image = self._decode(raw)
                with image:
                    width, height = image.size
                    if width * height > 8_000_000 or width % frames:
                        raise ValueError('Dimensioni o atlas non supportati')
                    width //= frames
                    with image.crop(((frame - 1) * width, 0, frame * width, height)) as crop:
                        output = BytesIO()
                        crop.save(output, format='PNG')
                        data = output.getvalue()
                self.cache.mkdir(parents=True, exist_ok=True)
                cached.write_bytes(data)
                return data
            except (OSError, ValueError, NotImplementedError, struct.error, Image.DecompressionBombError):
                self.failed.add(token)
                return None

    @staticmethod
    def _decode(raw: bytes):
        # DDS DX10 BGRA sRGB (DXGI 91) compare in tre icone CZE installate.
        if len(raw) >= 148 and raw[:4] == b'DDS ' and raw[84:88] == b'DX10':
            height, width, pitch = struct.unpack_from('<III', raw, 12)
            fmt, dimension, misc, count = struct.unpack_from('<IIII', raw, 128)
            if fmt in (87, 91) and dimension == 3 and count == 1 and not misc & 4:
                if not 0 < width * height <= 8_000_000:
                    raise ValueError('Dimensioni DDS non supportate')
                stride = pitch if pitch >= width * 4 else width * 4
                if stride * height > len(raw) - 148:
                    raise ValueError('DDS troncato')
                return Image.frombytes('RGBA', (width, height), raw[148:], 'raw', 'BGRA', stride, 1)
        image = Image.open(BytesIO(raw))
        if image.width * image.height > 8_000_000:
            image.close()
            raise ValueError('Texture troppo grande')
        try:
            return image.convert('RGBA')
        finally:
            image.close()

    def inline(self, texts: list[str]) -> dict:
        import re
        result = {}
        for text in texts:
            for match in re.finditer(r'£([A-Za-z0-9_]+)(?:\|(\d+))?£?', text):
                token = match.group(0)
                name = match.group(1)
                frame = int(match.group(2) or 1)
                # Il frame 0 nei testi non seleziona un frame noto: resta testo.
                if frame < 1:
                    continue
                name = name if name.startswith('GFX_') else 'GFX_' + name
                if sprite := self.resolve(name, frame):
                    result[token] = sprite
        return result

    def theme(self) -> dict:
        names = {'locked': 'GFX_focus_unavailable', 'unknown': 'GFX_focus_unavailable',
                 'verify': 'GFX_focus_unavailable', 'excluded': 'GFX_focus_unavailable',
                 'ready': 'GFX_focus_can_start', 'active': 'GFX_focus_can_start',
                 'completed': 'GFX_focus_completed', 'exclusive': 'GFX_focus_link_exclusive'}
        return {key: value for key, name in names.items() if (value := self.resolve(name))}
