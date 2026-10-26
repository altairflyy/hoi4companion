'use strict';
(() => {
  const root=document.createElement('section');root.className='live-panel';root.setAttribute('aria-label',HoiI18n.t("Current game state"));
  const heading=document.createElement('div');heading.className='live-heading';
  const label=document.createElement('label'),toggle=document.createElement('input');toggle.type='checkbox';toggle.checked=sessionStorage.getItem('hoi4-live-enabled')!=='off';
  label.append(toggle,document.createTextNode(HoiI18n.t(" Read current state · experimental")));
  const status=document.createElement('strong');status.textContent=HoiI18n.t("Connecting to the game…");heading.append(label,status);
  const detail=document.createElement('p'),list=document.createElement('ul'),follow=document.createElement('button');follow.type='button';follow.textContent=HoiI18n.t("Open the observed country's focus tree");follow.className='line-button';follow.hidden=true;
  root.append(heading,detail,list,follow);document.querySelector('.hero').after(root);
  let value=null,connection=null,signature='',lastArrival=0;const listeners=[];
  const api=window.hoiLive={enabled:toggle.checked,value:null,subscribe(callback){listeners.push(callback);if(value)callback(value);},
    sample(){return this.enabled&&this.value?.usable?this.value.sample:null;}};
  function changed(){for(const fn of listeners)fn(value);}
  function draw(){api.value=value;api.enabled=toggle.checked;document.body.classList.toggle('live-mode',toggle.checked);list.replaceChildren();follow.hidden=true;
    if(!toggle.checked){status.textContent=HoiI18n.t("Memory reading disabled");detail.textContent=HoiI18n.t("Views return to their snapshot or manual data source.");return;}
    if(!value?.usable){status.textContent=value?.sample?HoiI18n.t("Stale data · not applied to the graph"):HoiI18n.t("Game not connected");detail.textContent=(value?.error||HoiI18n.t("Waiting for a sample"))+(value?.sample?HoiI18n.t(" · last country {0}",value.sample.country):'');return;}
    const s=value.sample;status.textContent=`${s.country} · memoria del processo · sperimentale`;
    detail.textContent=HoiI18n.t("{0} completed or bypassed focuses · {1} technologies with researched levels · {2} slots. {3}. Browser events are sent when state changes; detection uses lightweight memory reads. Progress percentages and dynamic availability are not determined.",s.completed_focuses.length,s.researched_technologies.length,s.research_slots.length,s.current_focus?'Focus: '+(s.focus_name||s.current_focus):s.continuous_focus?'Focus continuo: '+(s.continuous_focus_name||s.continuous_focus):HoiI18n.t("No focus selected"));
    for(const slot of s.research_slots){const li=document.createElement('li');li.textContent=`Slot ${slot.slot}: ${slot.name||slot.technology_id||HoiI18n.t("free")}`;list.append(li);}
    follow.hidden=!s.focus_tree_id;follow.onclick=()=>window.dispatchEvent(new CustomEvent('hoi-live-follow',{detail:s}));
  }
  function publish(next){
    const stamp=JSON.stringify([next.usable,next.status,next.sample?{...next.sample,native_tick:undefined,metrics:undefined}:null,next.error,next.guide_revision,next.guide_checks,next.guide_states,next.guide_started,next.guide_baselines,next.guide_runs]);
    value=next;draw();if(stamp!==signature){signature=stamp;changed();}
  }
  function unavailable(message,statusName='disconnected'){
    publish({usable:false,status:statusName,sample:value?.sample||null,error:message});
  }
  function disconnect(){const old=connection;connection=null;if(old)old.close();lastArrival=0;}
  function connect(){
    if(connection||!toggle.checked||document.hidden)return;
    unavailable(HoiI18n.t("Connecting to the local stream; waiting for a verified sample"),'connecting');
    if(typeof EventSource==='undefined'){unavailable(HoiI18n.t("This browser does not support the local event stream"));return;}
    const stream=new EventSource('/api/state/events');connection=stream;
    stream.addEventListener('state',event=>{
      if(connection!==stream||!toggle.checked||document.hidden)return;
      try{
        const next=JSON.parse(event.data);
        if(typeof next.usable!=='boolean'||(next.usable&&!next.sample))throw Error(HoiI18n.t("Incomplete state"));
        lastArrival=Date.now();publish(next);
      }catch{unavailable(HoiI18n.t("Invalid state received; waiting for a new sample"),'stale');}
    });
    stream.addEventListener('heartbeat',event=>{
      if(connection!==stream||!toggle.checked||document.hidden)return;
      try{
        const beat=JSON.parse(event.data);lastArrival=Date.now();
        // A transport heartbeat cannot revive old game data after a disconnect.
        if(beat.usable!==true&&value?.usable)unavailable(HoiI18n.t("The reader no longer confirms fresh state"),'stale');
      }catch{unavailable(HoiI18n.t("Invalid reader confirmation"),'stale');}
    });
    stream.addEventListener('error',()=>{
      if(connection===stream)unavailable(HoiI18n.t("Local stream interrupted; reconnecting automatically"));
    });
  }
  toggle.addEventListener('change',()=>{
    sessionStorage.setItem('hoi4-live-enabled',toggle.checked?'on':'off');signature='';api.enabled=toggle.checked;
    disconnect();unavailable(toggle.checked?HoiI18n.t("Waiting for a new sample"):HoiI18n.t("Memory reading disabled"));
    if(toggle.checked)connect();
  });
  document.addEventListener('visibilitychange',()=>{
    if(document.hidden){disconnect();unavailable(HoiI18n.t("Reading suspended while this page is hidden"),'suspended');}
    else connect();
  });
  window.addEventListener('pagehide',disconnect);
  setInterval(()=>{
    if(api.enabled&&value?.usable&&Date.now()-lastArrival>12000)
      unavailable(HoiI18n.t("No recent reader confirmation; state not applied"),'stale');
  },2000);
  draw();connect();
})();
