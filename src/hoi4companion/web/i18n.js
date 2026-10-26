/* UI copy only. Game localization and imported guide content are not rewritten. */
'use strict';
(function(root,factory){
 const data=typeof module==='object'&&module.exports?require('../translations.json'):root.HOI_TRANSLATIONS;
 const api=factory(data||{messages:{},legacy:{}});
 if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiI18n=api;
})(typeof globalThis==='object'?globalThis:this,function(catalog){
 let locale=typeof document==='object'&&document.documentElement.lang==='it'?'italian':'english';
 function t(message,...values){
  const english=catalog.legacy[message]||message;
  const text=locale==='italian'?(catalog.messages[english]||message):english;
  return text.replace(/\{(\d+)\}/g,(match,index)=>index<values.length?String(values[index]):match);
 }
 function setLanguage(value){if(!['english','italian'].includes(value))throw Error('Unsupported interface language');locale=value;}
 function mount(){
  const nav=document.querySelector('nav[aria-label]');if(!nav)return;
  const label=document.createElement('label');label.className='ui-language';label.textContent=t('Interface language')+' ';
  const select=document.createElement('select');select.setAttribute('aria-label',t('Interface language'));
  for(const [value,name] of [['english','English'],['italian','Italiano']]){const option=document.createElement('option');option.value=value;option.textContent=name;select.append(option);}
  select.value=locale;label.append(select);nav.append(label);
  select.addEventListener('change',async()=>{
   select.disabled=true;
   try{
    const response=await fetch('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({ui_language:select.value})});
    if(!response.ok){const result=await response.json();throw Error(result.error||t('Could not change interface language'));}
    location.reload();
   }catch(error){select.disabled=false;select.value=locale;label.title=error.message;}
  });
 }
 if(typeof document==='object')document.addEventListener('DOMContentLoaded',mount,{once:true});
 return {t,setLanguage,get language(){return locale;}};
});
