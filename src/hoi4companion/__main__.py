from __future__ import annotations
import argparse
from pathlib import Path
from .api import Store, serve
from .installation import detect_game, default_documents


def main():
    parser = argparse.ArgumentParser(description='HOI4 Companion — Focus Tree MVP')
    parser.add_argument('--game-dir', help='Percorso della cartella di Hearts of Iron IV')
    parser.add_argument('--docs-dir', help='Cartella documenti di Hearts of Iron IV')
    parser.add_argument('--snapshot', help='Snapshot JSON o salvataggio testuale .hoi4')
    parser.add_argument('--country', default=None, help='TAG nazionale, es. AUS')
    parser.add_argument('--port', default=8766, type=int)
    parser.add_argument('--no-browser', action='store_true')
    parser.add_argument('--test', action='store_true', help='Esegue i test unitari')
    args = parser.parse_args()
    if args.test:
        import unittest
        root = Path(__file__).resolve().parents[2]
        suite = unittest.defaultTestLoader.discover(str(root / 'tests'))
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        raise SystemExit(0 if result.wasSuccessful() else 1)
    detected = detect_game()
    docs = default_documents()
    overrides = {}
    if args.game_dir or detected:
        overrides['game_dir'] = str(Path(args.game_dir or detected))
    if args.docs_dir or docs.is_dir():
        overrides['docs_dir'] = str(Path(args.docs_dir or docs))
    if args.snapshot:
        overrides['snapshot_path'] = args.snapshot
    if args.country:
        overrides['country'] = args.country.upper()
    serve(Store(overrides), port=args.port, open_browser=not args.no_browser)


if __name__ == '__main__':
    main()
