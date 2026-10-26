"""Explanations of qualified GUI observations, never target admission rules."""
import json
from pathlib import Path
import time


def explain_geometry(raw,proposals,matches):
    groups={};matched={m['key'] for m in matches};tabs={k for m in matches if m.get('panel')=='research_tab' for k in m.get('step_keys',[])}
    for p in proposals:
        panel=p.get('panel')
        if panel not in ('focus','research','event','politics','projects'):continue
        info=raw.get('panels',{}).get(panel,{})
        folder=p.get('folder');name=p.get('folder_name') or folder or 'ricerche'
        message='';code=''
        if raw.get('country')!=p.get('country'):code='identity_mismatch';message='Identità della nazione diversa: indicazione sospesa.'
        elif any(w.get('visible') is True for w in raw.get('event_windows',[])) and panel!='event':
            code='event_foreground';message='Una finestra evento è in primo piano: le indicazioni degli alberi sono sospese.'
        elif p['key'] in matched:
            code='highlighted';message='Pulsante individuato ed evidenziato.'
            stage=next((m.get('navigation_stage') for m in matches if m['key']==p['key']),None)
            if stage in ('access','slot','facility'):message='Evidenziato il punto di accesso; l’entità verrà indicata solo nel pannello e contesto qualificati.'
            if p.get('action_readiness',{}):
                if p['action_readiness'].get('unknown'):message+=' Restano requisiti non verificabili; la cornice non certifica disponibilità o esecuzione.'
        elif panel in ('politics','projects'):
            code='action_gui_unqualified'
            message='Slot o ruolo non individuato in modo univoco nella GUI qualificata.' if panel=='politics' else 'Impianto e specializzazione non ancora qualificati: nessun progetto scelto arbitrariamente.'
            if raw.get('action_ui',{}).get('status') in ('partial','unavailable'):
                message='Contesto GUI delle azioni non qualificato: '+raw.get('action_ui',{}).get('reason','copertura incompleta')
        elif info.get('visible') is not True:code='panel_closed';message='Apri l’albero tecnologico nel gioco.' if panel=='research' else 'Apri l’albero dei focus nel gioco.' if panel=='focus' else 'L’evento previsto non è visibile.'
        elif panel=='focus' and p.get('scope') and p['scope']!=raw.get('scope'):code='identity_mismatch';message='Albero nazionale diverso: indicazione sospesa.'
        elif panel=='research' and folder!=info.get('folder'):
            code='different_folder';message='Ricerca prevista in '+name+'. '+('La scheda è evidenziata nel gioco.' if p['key'] in tabs else 'Apri questa scheda nel gioco.')
        else:
            nodes=[n for n in raw.get('nodes',[]) if n.get('panel')==panel and n.get('identity')==p.get('identity') and (panel!='research' or n.get('folder')==folder)]
            if len(nodes)>1:code='widget_ambiguous';message='Più pulsanti corrispondenti: nessuna posizione scelta arbitrariamente.'
            elif not nodes:code='widget_unresolved';message='Pulsante non individuato nella GUI corrente; la causa non è ancora verificata.'
            else:
                clip=nodes[0].get('clip')
                if isinstance(clip,list) and len(clip)==4 and (clip[2]<=0 or clip[3]<=0):code='offscreen';message='Pulsante fuori dall’area visibile: scorri l’albero.'
                elif nodes[0].get('visible') is not True:code='widget_hidden';message='Il widget corrispondente non è visibile; non ne deduco la disponibilità.'
                else:code='geometry_unqualified';message='Pulsante individuato, ma il rettangolo visibile non è qualificato.'
        group=groups.setdefault((code,panel,folder),{'code':code,'message':message,'panel':panel,'folder':folder,'step_keys':[]})
        group['step_keys'].append(p['key'])
    return list(groups.values())


class HudStatus:
    """Bounded local status reader, reused by the existing SSE producer.

    No extra live reader, thread or browser polling. Only semantic explanation
    fields are forwarded; frame timings never cause a state-event feedback loop.
    """
    def __init__(self,path):self.path=Path(path);self.key=None;self.cached=None

    def read(self,guide_id,server_id):
        unavailable={'schema_version':1,'status':'unavailable','code':'not_observed',
                     'message':'Stato HUD non disponibile: avvialo per verificarne le indicazioni.','items':[]}
        try:
            stat=self.path.stat();age=time.time()-stat.st_mtime
            if not 0<=age<=4:return {**unavailable,'code':'stale','message':'Stato HUD non aggiornato; non mostro indicazioni precedenti.'}
            if stat.st_size>262144:return unavailable
            key=(stat.st_mtime_ns,stat.st_size)
            if key!=self.key:
                value=json.loads(self.path.read_text(encoding='utf-8'))
                self.cached=value if isinstance(value,dict) else None;self.key=key
            value=self.cached
            if not value or value.get('guide_id')!=guide_id or value.get('guide_server_id')!=server_id:
                return {**unavailable,'code':'context_mismatch','message':'Diagnostica HUD riferita a un’altra guida o sessione del programma.'}
            items=[]
            for item in (value.get('explanations') or [])[:8]:
                if not isinstance(item,dict) or not isinstance(item.get('code'),str) or not isinstance(item.get('message'),str):continue
                items.append({'code':item['code'][:80],'message':item['message'][:500],
                    **{k:item[k][:160] for k in ('panel','folder') if isinstance(item.get(k),str)},
                    'step_keys':[key[:160] for key in item.get('step_keys',[])[:64] if isinstance(key,str)]
                    if isinstance(item.get('step_keys'),list) else []})
            message=value.get('status_message')
            if not isinstance(message,str) or not message:return unavailable
            return {'schema_version':1,'status':'observed','code':value.get('status_code','unknown'),
                    'message':message[:500],'items':items,'guide_id':guide_id}
        except (OSError,ValueError,TypeError):return unavailable
