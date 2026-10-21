'use strict';
(() => {
  const root=document.createElement('section');root.className='live-panel';root.setAttribute('aria-label','Stato corrente del gioco');
  const heading=document.createElement('div');heading.className='live-heading';
  const label=document.createElement('label'),toggle=document.createElement('input');toggle.type='checkbox';toggle.checked=sessionStorage.getItem('hoi4-live-enabled')!=='off';
  label.append(toggle,document.createTextNode(' Leggi lo stato corrente · sperimentale'));
  const status=document.createElement('strong');status.textContent='Connessione al gioco…';heading.append(label,status);
  const detail=document.createElement('p'),list=document.createElement('ul'),follow=document.createElement('button');follow.type='button';follow.textContent='Apri l’albero della nazione rilevata';follow.className='line-button';follow.hidden=true;
  root.append(heading,detail,list,follow);document.querySelector('.hero').after(root);
  let value=null,connection=null,signature='',lastArrival=0;const listeners=[];
  const api=window.hoiLive={enabled:toggle.checked,value:null,subscribe(callback){listeners.push(callback);if(value)callback(value);},
    sample(){return this.enabled&&this.value?.usable?this.value.sample:null;}};
  function changed(){for(const fn of listeners)fn(value);}
  function draw(){api.value=value;api.enabled=toggle.checked;document.body.classList.toggle('live-mode',toggle.checked);list.replaceChildren();follow.hidden=true;
    if(!toggle.checked){status.textContent='Lettura memoria disattivata';detail.textContent='Le viste tornano alla propria sorgente snapshot o manuale.';return;}
    if(!value?.usable){status.textContent=value?.sample?'Dati obsoleti · non applicati al grafo':'Gioco non collegato';detail.textContent=(value?.error||'In attesa di un campione')+(value?.sample?` · ultimo paese ${value.sample.country}`:'');return;}
    const s=value.sample;status.textContent=`${s.country} · memoria del processo · sperimentale`;
    detail.textContent=`${s.completed_focuses.length} focus completati o saltati · ${s.researched_technologies.length} tecnologie con livelli ricercati · ${s.research_slots.length} slot. ${s.current_focus?'Focus: '+(s.focus_name||s.current_focus):s.continuous_focus?'Focus continuo: '+(s.continuous_focus_name||s.continuous_focus):'Nessun focus selezionato'}. Eventi al browser quando cambia lo stato; rilevamento con letture leggere della memoria. Percentuali e disponibilità dinamica non determinate.`;
    for(const slot of s.research_slots){const li=document.createElement('li');li.textContent=`Slot ${slot.slot}: ${slot.name||slot.technology_id||'libero'}`;list.append(li);}
    follow.hidden=!s.focus_tree_id;follow.onclick=()=>window.dispatchEvent(new CustomEvent('hoi-live-follow',{detail:s}));
  }
  function publish(next){
    const stamp=JSON.stringify([next.usable,next.status,next.sample?{...next.sample,native_tick:undefined,metrics:undefined}:null,next.error,next.guide_revision,next.guide_checks]);
    value=next;draw();if(stamp!==signature){signature=stamp;changed();}
  }
  function unavailable(message,statusName='disconnected'){
    publish({usable:false,status:statusName,sample:value?.sample||null,error:message});
  }
  function disconnect(){const old=connection;connection=null;if(old)old.close();lastArrival=0;}
  function connect(){
    if(connection||!toggle.checked||document.hidden)return;
    unavailable('Connessione al flusso locale; attendo un campione verificato','connecting');
    if(typeof EventSource==='undefined'){unavailable('Questo browser non supporta il flusso di eventi locale');return;}
    const stream=new EventSource('/api/state/events');connection=stream;
    stream.addEventListener('state',event=>{
      if(connection!==stream||!toggle.checked||document.hidden)return;
      try{
        const next=JSON.parse(event.data);
        if(typeof next.usable!=='boolean'||(next.usable&&!next.sample))throw Error('Stato incompleto');
        lastArrival=Date.now();publish(next);
      }catch{unavailable('Stato ricevuto non valido; attendo un nuovo campione','stale');}
    });
    stream.addEventListener('heartbeat',event=>{
      if(connection!==stream||!toggle.checked||document.hidden)return;
      try{
        const beat=JSON.parse(event.data);lastArrival=Date.now();
        // A transport heartbeat cannot revive old game data after a disconnect.
        if(beat.usable!==true&&value?.usable)unavailable('Il lettore non conferma più uno stato fresco','stale');
      }catch{unavailable('Conferma del lettore non valida','stale');}
    });
    stream.addEventListener('error',()=>{
      if(connection===stream)unavailable('Flusso locale interrotto; riconnessione automatica in corso');
    });
  }
  toggle.addEventListener('change',()=>{
    sessionStorage.setItem('hoi4-live-enabled',toggle.checked?'on':'off');signature='';api.enabled=toggle.checked;
    disconnect();unavailable(toggle.checked?'In attesa di un nuovo campione':'Lettura memoria disattivata');
    if(toggle.checked)connect();
  });
  document.addEventListener('visibilitychange',()=>{
    if(document.hidden){disconnect();unavailable('Lettura sospesa mentre la pagina è nascosta','suspended');}
    else connect();
  });
  window.addEventListener('pagehide',disconnect);
  setInterval(()=>{
    if(api.enabled&&value?.usable&&Date.now()-lastArrival>12000)
      unavailable('Nessuna conferma recente dal lettore; stato non applicato','stale');
  },2000);
  draw();connect();
})();
