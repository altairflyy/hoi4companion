from __future__ import annotations
import argparse
from pathlib import Path
from .api import Store, serve
from .jomini_parser import ParserUnavailableError


def main():
    parser = argparse.ArgumentParser(description='HOI4 Companion — Focus Tree MVP')
    parser.add_argument('--game-dir', help='Path to the Hearts of Iron IV installation')
    parser.add_argument('--docs-dir', help='Hearts of Iron IV documents directory')
    parser.add_argument('--snapshot', help='JSON snapshot or text .hoi4 save')
    parser.add_argument('--country', default=None, help='Country tag, e.g. AUS')
    parser.add_argument('--ui-language',choices=('english','italian'),help='Application interface language (default: English)')
    parser.add_argument('--language',choices=('english','italian'),help='Game content language, separate from the interface')
    parser.add_argument('--port', default=8766, type=int)
    parser.add_argument('--no-browser', action='store_true')
    parser.add_argument('--test', action='store_true', help='Run unit tests')
    parser.add_argument('--export-script', type=Path, help='Export a local script using Jomini JSON without starting the server')
    parser.add_argument('--script-encoding',choices=('auto','utf-8','windows-1252'),default='auto')
    parser.add_argument('--json-duplicates',choices=('key_value_pairs','group','preserve'),default='key_value_pairs')
    parser.add_argument('--output',type=Path,help='New JSON file; omit to write UTF-8 to stdout')
    args = parser.parse_args()
    if args.export_script:
        from .jomini_parser import export_json
        import sys
        try:
            with args.export_script.open('rb') as stream:
                data=stream.read(80_000_001)
            result=export_json(data,encoding=args.script_encoding,duplicate_keys=args.json_duplicates)
            output=(result['json']+'\n').encode('utf-8')
            if args.output:
                if args.output.suffix.lower()!='.json':
                    raise ValueError('Export requires a new .json file')
                # Exclusive creation also prevents accidental source replacement.
                with args.output.open('xb') as stream:stream.write(output)
            else:sys.stdout.buffer.write(output)
        except (OSError,ValueError,ParserUnavailableError) as exc:
            parser.exit(1,f'{exc}\n')
        return
    if args.output:
        parser.error('--output requires --export-script')
    if args.test:
        import unittest
        root = Path(__file__).resolve().parents[2]
        suite = unittest.defaultTestLoader.discover(str(root / 'tests'))
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        raise SystemExit(0 if result.wasSuccessful() else 1)
    overrides = {}
    if args.ui_language:overrides['ui_language']=args.ui_language
    if args.language:overrides['language']=args.language
    if args.game_dir:
        overrides['game_dir'] = str(Path(args.game_dir))
    if args.docs_dir:
        overrides['docs_dir'] = str(Path(args.docs_dir))
    if args.snapshot:
        overrides['snapshot_path'] = args.snapshot
    if args.country:
        overrides['country'] = args.country.upper()
    try:
        store = Store(overrides)
    except ParserUnavailableError as exc:
        parser.exit(1, f'{exc}\n')
    serve(store, port=args.port, open_browser=not args.no_browser)


if __name__ == '__main__':
    main()
