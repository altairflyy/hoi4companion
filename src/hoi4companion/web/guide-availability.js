'use strict';
((root)=>{
 function notice(value,t=(s,...args)=>s.replace(/\{(\d+)\}/g,(_,i)=>args[i])){
  if(!value?.blocked)return '';
  const alternatives=(value.requirements||[]).map(group=>[
   group.enable?.length?t('Requires DLC: {0}',group.enable.join(', ')):'',
   group.disable?.length?t('Requires disabling DLC: {0}',group.disable.join(', ')):''
  ].filter(Boolean).join(' · '));
  return alternatives.join(' / ');
 }
 function decorate(option,value,t){
  option.disabled=value?.blocked===true;
  option.classList?.toggle('guide-dlc-unavailable',option.disabled);
  if(option.disabled){const reason=notice(value,t);option.title=reason;option.textContent+=' — '+reason;}
  return option;
 }
 const api={notice,decorate};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiGuideAvailability=api;
})(typeof window!=='undefined'?window:globalThis);
