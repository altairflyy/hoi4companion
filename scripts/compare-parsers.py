"""Read-only corpus gate for the production Jomini parser.

The Python implementation remains a reference, never a fallback for native errors.
The installed native library is discovered automatically, as in the application.
"""
import argparse
import json
from pathlib import Path
import time

from hoi4companion.clausewitz import Node
import importlib.util
_reference_spec = importlib.util.spec_from_file_location(
    'legacy_clausewitz', Path(__file__).with_name('legacy_clausewitz.py'))
_reference = importlib.util.module_from_spec(_reference_spec)
_reference_spec.loader.exec_module(_reference)
parse_legacy = _reference.parse_legacy
from hoi4companion.jomini_parser import parse_bytes as parse_native
from hoi4companion.installation import collect_files, active_mods


def difference(a, b, location='root'):
    if isinstance(a, Node) and isinstance(b, Node):
        if len(a.entries) != len(b.entries):
            return f'{location}: entry count {len(a.entries)} -> {len(b.entries)}'
        for i, ((ka, va), (kb, vb)) in enumerate(zip(a.entries, b.entries)):
            if ka != kb:
                return f'{location}[{i}]: key {ka!r} -> {kb!r}'
            found = difference(va, vb, location+'.'+ka)
            if found:
                return found
        return None
    return None if a == b else f'{location}: {str(a)[:100]!r} -> {str(b)[:100]!r}'


def syntax_upgrades(a, b, location='root'):
    """Audit only two observed lexical corrections; never modify either AST.

    Require alignment of the entire trees. An unrelated missing/changed entry
    remains unexplained and blocks the gate, regardless of its source file.
    """
    if not isinstance(a, Node) or not isinstance(b, Node):
        if a == b:
            return []
        if isinstance(a,str) and a.endswith(';') and a[:-1] == b:
            return [{'path':location,'kind':'unquoted_semicolon_separator','old':a,'new':b}]
        return None
    result=[];i=j=0
    while i<len(a.entries):
        ka,va=a.entries[i]
        if ka=='' and va==';':
            result.append({'path':location,'kind':'semicolon_separator'});i+=1;continue
        if j>=len(b.entries):return None
        kb,vb=b.entries[j]
        if ka!=kb:return None
        if isinstance(va,str) and isinstance(vb,str) and i+2<len(a.entries) and a.entries[i+1]==('', '?'):
            suffix_key,suffix=a.entries[i+2]
            if suffix_key=='' and isinstance(suffix,str) and va+'?'+suffix==vb:
                result.append({'path':location+'.'+ka,'kind':'opaque_question_expression','old':[va,'?',suffix],'new':vb})
                i+=3;j+=1;continue
        changes=syntax_upgrades(va,vb,location+'.'+ka)
        if changes is None:return None
        result.extend(changes);i+=1;j+=1
    return result if j==len(b.entries) else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-dir', type=Path, required=True)
    parser.add_argument('--docs-dir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    mods, warnings = active_mods(args.docs_dir)
    files = []
    for directory, extension in [('common','.txt'),('history','.txt'),('events','.txt'),('interface','.gui'),('interface','.gfx')]:
        paths, issues = collect_files(args.game_dir, mods, directory, extension)
        files.extend(paths)
        warnings.extend(issues)
    files = list(dict.fromkeys(files))
    report = {'schema_version':1, 'jomini_version':'0.37.2', 'files':len(files),
              'bytes':0, 'identical':0, 'both_rejected':0, 'newly_accepted':[],
              'regressions':[], 'different':[], 'reviewed_syntax_upgrades':[], 'unexplained':[], 'legacy_seconds':0, 'native_bridge_seconds':0,
              'warnings':warnings, 'results':[]}
    for path in files:
        raw = path.read_bytes()
        source = raw.decode('utf-8-sig', errors='replace')
        report['bytes'] += len(raw)
        values, errors, times = [], [], []
        for parse in (parse_legacy, parse_native):
            start = time.perf_counter()
            try:
                values.append(parse(source if parse is parse_legacy else raw)); errors.append(None)
            except (ValueError, RecursionError) as exc:
                values.append(None); errors.append(str(exc))
            times.append(time.perf_counter()-start)
        report['legacy_seconds'] += times[0]
        report['native_bridge_seconds'] += times[1]
        row = {'file':str(path), 'bytes':len(raw), 'legacy_error':errors[0], 'native_error':errors[1]}
        if errors[0] and errors[1]:
            report['both_rejected'] += 1; row['status']='both_rejected'
        elif errors[1]:
            report['regressions'].append(row); row['status']='regression'
        elif errors[0]:
            report['newly_accepted'].append(row); row['status']='newly_accepted'
        else:
            reason = difference(*values)
            if reason:
                row.update(status='different', difference=reason); report['different'].append(row)
                changes=syntax_upgrades(*values)
                if changes is None:
                    report['unexplained'].append(row)
                else:
                    report['reviewed_syntax_upgrades'].append({'file':str(path),'changes':changes})
            else:
                row['status']='identical'; report['identical'] += 1
        report['results'].append(row)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    lists=('newly_accepted','regressions','different','reviewed_syntax_upgrades','unexplained')
    print(json.dumps({k:v for k,v in report.items() if k not in ('results',*lists)}
        | {k:len(report[k]) for k in lists},ensure_ascii=False))
    raise SystemExit(1 if report['regressions'] or report['unexplained'] else 0)


if __name__ == '__main__':
    main()
