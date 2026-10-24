"""Persistent audit of unreadable and reviewed recovered sources.

Each explicit analysis retries the sources. Historical versions stay available
for investigation; a registry failure is visible and never blocks live reads.
"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
from tempfile import NamedTemporaryFile


def record_source_errors(report, path):
    if 'ignored_source_errors' not in report:
        return report
    path=Path(path)
    result={**report, 'file_warnings':list(report.get('file_warnings',[]))}
    result['source_error_log']={'status':'saved','path':str(path.resolve()),
                              'count':len(report['ignored_source_errors']),
                              'recovered':len(report.get('source_recoveries',[]))}
    temporary=None
    try:
        registry=json.loads(path.read_text(encoding='utf8')) if path.exists() else {'schema_version':1,'entries':[]}
        if (not isinstance(registry,dict) or registry.get('schema_version')!=1
                or not isinstance(registry.get('entries'),list)
                or any(not isinstance(e,dict) or not isinstance(e.get('id'),str)
                       for e in registry['entries'])):
            raise ValueError('Formato del registro non supportato')
        entries={e['id']:dict(e) for e in registry['entries']}
        sources=report.get('sources',{})
        scanned=set(sources.values())
        now=datetime.now(timezone.utc).isoformat()
        for entry in entries.values():
            if entry.get('source_path') in scanned:
                entry['active']=False
        outcomes=[(error,'skipped_unreadable_source') for error in report['ignored_source_errors']]
        for item in report.get('source_recoveries',[]):
            evidence=item['evidence']
            outcomes.append(({'file':item['file'],'source_sha256':item['source_sha256'],
                'classification':'recovered_by_both','jomini_error':evidence['original_error'],
                'recovery':evidence},'recovered_in_memory'))
        for error,disposition in outcomes:
            source=sources.get(error['file'],'')
            identity=json.dumps([source or error['file'],error['source_sha256']],ensure_ascii=False)
            key=hashlib.sha256(identity.encode()).hexdigest()
            previous=entries.get(key,{})
            entries[key]={**previous,**error,'id':key,'source_path':source,
                          'first_seen':previous.get('first_seen',now),'last_seen':now,
                          'active':True,'disposition':disposition,
                          'engine_revision':report['rules']['engine_revision'],
                          'rules_revision':report['rules']['revision']}
        registry={'schema_version':1,'updated_at':now,
                  'meaning':'Parser rejection is not proof of a gameplay defect. Sources are retried on each analysis.',
                  'entries':sorted(entries.values(),key=lambda e:e['id'])}
        path.parent.mkdir(parents=True,exist_ok=True)
        with NamedTemporaryFile(mode='w',encoding='utf8',dir=path.parent,
                                prefix=path.name+'.',suffix='.tmp',delete=False) as handle:
            temporary=Path(handle.name)
            json.dump(registry,handle,ensure_ascii=False,indent=2)
            handle.write('\n')
        os.replace(temporary,path)
    except (OSError,ValueError,KeyError,TypeError) as exc:
        result['source_error_log']['status']='error'
        result['source_error_log']['error']=str(exc)
        result['status']='partial'
        result['file_warnings'].append('Registro dei sorgenti accantonati non salvato: '+str(exc))
    finally:
        if temporary:
            try:temporary.unlink(missing_ok=True)
            except OSError:pass
    return result
