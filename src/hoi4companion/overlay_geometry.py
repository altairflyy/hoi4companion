"""Admit current runtime targets using witnessed, read-only game GUI identities."""
import math
from pathlib import Path
import time
from .overlay_diagnostics import explain_geometry


def _box(value):
    if (not isinstance(value,(list,tuple)) or len(value)!=4
            or any(type(n) not in (int,float) or not math.isfinite(n) for n in value)):
        return None
    x,y,w,h=value
    return (x,y,x+w,y+h) if w>0 and h>0 else None


def _rectangle(node,panel,view):
    boxes=[_box(node.get('rect')),_box(node.get('clip')),_box(panel.get('clip')),
           (0,0,view[4],view[5])]
    if any(b is None for b in boxes):return None
    left=max(b[0] for b in boxes);top=max(b[1] for b in boxes)
    right=min(b[2] for b in boxes);bottom=min(b[3] for b in boxes)
    return [left,top,right,bottom] if right-left>=8 and bottom-top>=8 else None


class NativeGeometry:
    def __init__(self,get,*,reader_factory=None):
        self.get=get;self.factory=reader_factory;self.reader=None
        self.retry_at=0;self.error='';self.supported=False

    def close(self):
        if self.reader is not None:self.reader.close()
        self.reader=None

    def _open(self):
        if self.reader is not None:return True
        if time.monotonic()<self.retry_at:return False
        try:
            if self.factory is None:
                from .focus_ui_native import UiReader
                self.factory=UiReader
            config=self.get('/api/config')
            directory=config.get('settings',config).get('game_dir')
            if not directory:raise ValueError('Percorso HOI4 non configurato')
            self.reader=self.factory(Path(directory)/'hoi4.exe')
            self.supported=True;self.error='';return True
        except (OSError,ValueError,RuntimeError) as exc:
            self.error=str(exc);self.retry_at=time.monotonic()+2;return False

    def read(self,view,proposals):
        result={'available':False,'matches':[],'handled_panels':[],
                'active_panel':None,'diagnostics':{}}
        process=getattr(self.reader,'process',None)
        if process is not None and process.pid!=view[1]:self.close()
        if not self._open():return {**result,'error':self.error,'suspend_visual':self.supported}
        result.update(available=True,handled_panels=['focus','research','event'])
        focus_ids={p['identity'] for p in proposals if p['panel']=='focus'}
        tech_ids={p['identity'] for p in proposals if p['panel']=='research'}
        event_ids={p.get('event_id') or p['identity'].split(':',1)[0] for p in proposals if p['panel']=='event'}
        instances={instance for p in proposals if p['panel']=='event' for instance in p.get('instances',[])}
        try:
            options={'action_ui':True} if any(p.get('panel') in ('politics','projects') for p in proposals) else {}
            raw=self.reader.snapshot(focus_ids=focus_ids,technology_ids=tech_ids,
                                     event_ids=event_ids,event_instances=instances,**options)
            result['diagnostics']=raw.get('diagnostics',{})
            if not raw.get('available'):return {**result,'suspend_visual':True,'error':raw.get('error','GUI non verificabile')}
            if raw.get('pid')!=view[1] or raw.get('resolution')!=view[4:]:
                self.close()
                return {**result,'suspend_visual':True,'error':'Processo o risoluzione GUI non corrispondenti'}
            panels=raw.get('panels',{})
            active=raw.get('active_panel')
            windows=[w for w in raw.get('event_windows',[]) if w.get('visible') is True]
            if windows:active='event'
            if active in ('focus','research') and panels.get(active,{}).get('visible') is True:
                result['active_panel']=active
            elif active=='event':result['active_panel']='event'
            country=raw.get('country')
            for proposal in proposals:
                panel=proposal['panel'];info=panels.get(panel,{})
                if active=='event' and panel!='event':continue
                if panel=='event':
                    if (country!=proposal.get('country') or raw.get('events',{}).get('status')!='observed'
                            or len(windows)!=1):continue
                    window=windows[0];event_id=proposal.get('event_id') or proposal['identity'].split(':',1)[0]
                    if window.get('event_id')!=event_id:continue
                    nodes=[n for n in raw.get('nodes',[]) if n.get('panel')=='event'
                           and n.get('identity')==proposal['identity'] and n.get('visible') is True
                           and n.get('event_id')==event_id and n.get('instance_id')==window.get('instance_id')]
                    if len(nodes)!=1:continue
                    rect=_rectangle(nodes[0],window,view)
                    if rect is not None:
                        result['matches'].append({**proposal,'rect':rect,'instance_id':window['instance_id'],
                                                  'basis':'native_gui_identity'})
                    continue
                if (panel not in ('focus','research') or info.get('visible') is not True
                        or not country or proposal.get('country')!=country):continue
                if panel=='focus' and proposal.get('scope') and proposal['scope']!=raw.get('scope'):continue
                if panel=='research' and (not info.get('folder') or proposal.get('folder')!=info['folder']):continue
                nodes=[n for n in raw.get('nodes',[]) if n.get('panel')==panel
                       and n.get('identity')==proposal['identity'] and n.get('visible') is True
                       and (panel!='focus' or n.get('scope')==raw.get('scope'))
                       and (panel!='research' or n.get('folder')==info.get('folder'))]
                if len(nodes)!=1:continue
                node=nodes[0]
                rect=_rectangle(node,info,view)
                if rect is None:continue
                result['matches'].append({**proposal,'rect':rect,
                                          'basis':'native_gui_identity'})
            info=panels.get('research',{})
            if active=='research' and info.get('visible') is True and info.get('folder'):
                folders={}
                for p in proposals:
                    if (p['panel']=='research' and p.get('country')==country and p.get('folder')
                            and p['folder']!=info['folder']):folders.setdefault(p['folder'],[]).append(p)
                for folder,steps in folders.items():
                    tabs=[t for t in raw.get('tabs',[]) if t.get('panel')=='research_tab'
                          and t.get('identity')==folder and t.get('visible') is True]
                    if len(tabs)!=1:continue
                    # Tabs have their own clipping region, above the scrollable
                    # tree. Native IDs supply identity; guide text supplies only
                    # the caption and never the screen coordinates.
                    tab=tabs[0];rect=_rectangle(tab,{'clip':tab.get('clip')},view)
                    if rect is None:continue
                    result['matches'].append({'key':'research-tab:'+folder,'panel':'research_tab',
                        'identity':folder,'folder':folder,'title':steps[0].get('folder_name') or folder,
                        'status':'active' if all(p.get('status')=='active' for p in steps) else 'ready',
                        'rect':rect,'step_keys':[p['key'] for p in steps],
                        'count':len(steps),'basis':'native_gui_identity'})
            from .action_navigation import navigation_matches
            actions=raw.get('action_ui',{})
            if actions.get('status')=='observed':
                result['handled_panels'] += [panel for panel,qualified in actions.get('coverage',{}).items() if qualified]
                # Resolve an observed advisor token only to its uniquely bound
                # catalog role; never turn a numeric native slot into a guess.
                for control in actions.get('controls',[]):
                    role={p.get('category_id') for p in proposals if p.get('panel')=='politics' and p.get('entity_kind')=='advisor' and p.get('identity')==control.get('entity_id')}
                    if control.get('entity_kind')=='advisor' and len(role)==1:control['category_id']=next(iter(role))
                result['matches']+=navigation_matches(raw,proposals,lambda node,info:_rectangle(node,info,view))
                if result['active_panel'] is None and actions.get('coverage',{}).get('politics') and any(actions.get('panels',{}).get(p) for p in ('countrypoliticsview','political_ideas_window')):
                    result['active_panel']='politics'
                if result['active_panel'] is None and actions.get('coverage',{}).get('projects') and any(actions.get('panels',{}).get(p) for p in ('program_window','facilities_view')):
                    result['active_panel']='projects'
            result['explanations']=explain_geometry(raw,proposals,result['matches'])
            return result
        except (OSError,ValueError,RuntimeError) as exc:
            # A torn/invalid observation never keeps the last coordinates and
            # never silently switches the same UI to text-based identification.
            return {**result,'matches':[],'suspend_visual':True,'error':str(exc)}
