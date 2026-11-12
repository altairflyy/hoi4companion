'use strict';
((root)=>{
 const country=typeof module==='object'&&module.exports?require('./country-model.js'):root.HoiCountryModel;
 const routes={overview:null,library:null,walkthrough:'/walkthrough',focus:'/',technologies:'/technologies',country:'/country',templates:'/importer#collectionTemplatesHeading',importer:'/importer',analysis:'/analysis',settings:'/'};
 function title(value){return String(value||'').replace(/\.docx$/i,'').replace(/_/g,' ').replace(/\s+/g,' ').trim();}
 function metrics(packet){const model=country.evaluate(packet),s=model.sections;
  return {current:model.current,country:model.country,divisions:country.number(s.divisions.total,{integer:true}),politicalPower:s.politics.power?.status==='observed'?country.number(s.politics.power.available,{signed:true}):null,factories:country.number(s.production.effective_factories?.military,{integer:true})};
 }
 function library(guides,tag,all,query=''){const needle=query.trim().toLocaleLowerCase();return (Array.isArray(guides)?guides:[]).filter(g=>g&&typeof g.id==='string'&&(!tag||all||!g.applies_to?.country||g.applies_to.country===tag)&&(!needle||(title(g.title)+' '+(g.applies_to?.country||'')).toLocaleLowerCase().includes(needle))).slice().sort((a,b)=>Number(b.applies_to?.country===tag)-Number(a.applies_to?.country===tag)||title(a.title).localeCompare(title(b.title)));}
 function selectable(guide,tag){return guide?.compatibility?.blocked!==true&&(!tag||!guide?.applies_to?.country||guide.applies_to.country===tag);}
 function currentProjection(packet,projection){const id=packet?.active_guide_id,run=packet?.guide_runs?.[id],campaign=packet?.sample?.campaign_identity;
  return !!(packet?.usable===true&&id&&projection?.engine==='rust-guide-core'&&projection.guide_id===id&&projection.state?.usable===true&&projection.state.session_id===packet.sample.session_id&&run?.run_id&&projection.progress?.run_id===run.run_id&&run.country===packet.sample.country&&campaign?.status==='observed'&&projection.progress.campaign_id===campaign.id&&run.campaign_id===campaign.id);
 }
 function projectionKey(packet){const sample={...packet?.sample};for(const key of ['native_tick','metrics','observed_at','age_seconds','transport'])delete sample[key];
  return JSON.stringify([packet?.usable,packet?.guide_server_id,packet?.active_guide_id,packet?.active_guide_revision,packet?.guide_revision,sample,packet?.guide_runs,packet?.guide_states,packet?.guide_started,packet?.guide_baselines]);
 }
 const api={routes,title,metrics,library,selectable,currentProjection,projectionKey};if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiSuiteModel=api;
})(typeof window==='object'?window:globalThis);
