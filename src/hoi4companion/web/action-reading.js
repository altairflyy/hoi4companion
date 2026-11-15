'use strict';
// Presentation only: shared vocabulary, no ID resolution or runtime decisions.
((root)=>{
 const i18n=typeof module!=='undefined'&&module.exports?require('./i18n.js'):root.HoiI18n;
 const t=(s,...args)=>i18n.t(s,...args),key=s=>s.toLocaleLowerCase('en'),escape=s=>s.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 function create(language={}){
  const domains=language.abbreviations||{},index=new Map();
  for(const [domain,codes] of Object.entries(domains))for(const [code,meaning] of Object.entries(codes)){
   if(typeof meaning!=='string'||!code||code.length>80)continue;
   const k=key(code);if(!index.has(k))index.set(k,[]);index.get(k).push({code,meaning,domain});
  }
  // One compiled matcher per guide, reused while live state changes.
  const codes=[...index.keys()].filter(c=>c.length>1).sort((a,b)=>b.length-a.length).map(escape);
  const matcher=codes.length?new RegExp('(?<![\\p{L}\\p{N}_])('+codes.join('|')+')(?![\\p{L}\\p{N}_])','giu'):null;
  const divis=Object.keys(domains.divisions||{}).sort((a,b)=>b.length-a.length).map(escape);
  const variants=divis.length?new RegExp('\\b('+divis.join('|')+')([ast]{1,3})(?![\\p{L}\\p{N}_])','gu'):null;
  const recruiting=divis.length?new RegExp('\\brecruit\\s+(\\d+)\\s+(?=(?:'+divis.join('|')+')(?:[ast]{1,3})?\\b)','gi'):null;
  function read(text,row={}){
   if(typeof text!=='string')return {text:'',terms:[],notes:[]};
   const p=row.action?.params||{},terms=new Map(),notes=new Set(),protectedText=[];
   const hold=s=>'\uE000'+(protectedText.push(s)-1)+'\uE001';
   const record=(code,meaning)=>{terms.set(code,{code,meaning});return t(meaning);};
   const restore=s=>s.replace(/\uE000(\d+)\uE001/g,(_,n)=>protectedText[Number(n)]);
   // Preserve exact bound names and quotations: abbreviations inside names are
   // not instructions. References remain available through the original code.
   const names=[p.variant_name,p.person_name,p.law_name,p.project_name,p.template_name,
    p.design_reference?.name,...(Array.isArray(p.reference_images)?p.reference_images:[]).map(r=>r?.name),
    ...(Array.isArray(p.production_targets)?p.production_targets:[]).flatMap(r=>[r?.variant_name,r?.design_reference?.name])]
    .filter(s=>typeof s==='string'&&s.length>1).sort((a,b)=>b.length-a.length);
   let input=text.replace(/"[^"\n]+"|“[^”\n]+”/g,hold);
   if(names.length)input=input.replace(new RegExp('(?<![\\p{L}\\p{N}_])(?:'+names.map(escape).join('|')+')(?![\\p{L}\\p{N}_])','gu'),hold);
   const lines=input.split(/\n|\s+[-•]\s+(?=(?:ARMY|NAVY|LAW|BUILD|MIL|NAV|ASAP)\b)/i).filter(s=>s.trim());
   const output=lines.map(raw=>{
    let line=raw.replace(/^\s*[-•]\s*/,'').trim(),domain=p.category||row.category||'';
    line=line.replace(/\bBuild\s+INFRAs?\s*\+\s*MILs?\b/gi,t('Build infrastructure, then military factories, in each indicated region before moving to the next'))
     .replace(/\bBuild\s+INFRAs?\s*,?\s+then\s+MILs?\b/gi,t('Build infrastructure in all indicated regions first, then military factories'))
     .replace(/\bOn\s+free\s+CIV\b/gi,t('When civilian factories become available'));
    if(/^(?:MIL|NAV|PRODUCTION)\s*:|\b(?:produce|production)\b/i.test(line))domain='production';
    else if(/^(?:BUILD|CONSTRUCT)\b/i.test(line))domain='construction';
    else if(/^LAW\s*:/i.test(line))domain='occupation';
    else if(/^(?:ARMY|NAVY|AIR FORCE)\s*:|\b(?:recruit|divisions?|battalions?|armies|garrison|template|train|exercise|bring|convert|duplicate|replace|swap|send|change|VOL)\b|\b\d+w\b/i.test(line))domain='divisions';
    if(/^NAVY\s*:/i.test(line))domain='naval';
    if(/^AIR FORCE\s*:/i.test(line))domain='designs';
    // A doctrine qualifier is essential: SD and BS have different meanings.
    const doctrine=line.match(/\b(land|air|naval)\s+doc(?:trines?)?\b/i);
    if(doctrine)domain=key(doctrine[1])+'_doctrines';
    if(domain==='army')domain='divisions';
    const heading=line.match(/^(ARMY|NAVY|AIR FORCE|MIL|NAV|BUILD|PRODUCTION)\s*:\s*/i);
    if(heading)line=line.slice(heading[0].length);
    line=line.replace(/\b(\d+(?:\.\d+)?)\s*(PP|AE|NE|CP)\b/gi,(all,n,code)=>domains.resources?.[code.toUpperCase()]?hold(t('{0} {1}',n,record(code.toUpperCase(),domains.resources[code.toUpperCase()]))):all);
    if(domain==='divisions'||domain==='occupation'){
     if(recruiting)line=line.replace(recruiting,(_,n)=>t('Recruit')+' '+n+' '+t('Divisions').toLocaleLowerCase()+' · ');
     line=line.replace(/^Bring\s+(.+?)\s+to\s+(?=\d+w\b)/i,(_,name)=>t('Template')+': '+name+' · ');
     line=line.replace(/\b(\d+)\s*w\b/gi,(_,n)=>hold(t('combat width {0}',n)));
     if(variants)line=line.replace(variants,(all,base,suffix)=>{
      const extra=[...new Set(suffix)].map(c=>language.conventions?.division_suffixes?.[c]);
      if(extra.some(s=>!s))return all;
      const meaning=t('{0} with {1}',t(domains.divisions[base]),extra.map(s=>t(s)).join(' + '));
      record(all,meaning);return hold(t('{0} template ({1})',meaning,all));
     });
     line=line.replace(/\bchange\s+ALL\s+to\b/gi,t('Change all divisions to'))
      .replace(/\btrain\s+ALL\b/gi,t('Exercise all divisions'));
     line=line.replace(/\b(recruit|duplicate|train)\b/gi,word=>t(({recruit:'Recruit',duplicate:'Duplicate',train:'Exercise'})[key(word)]));
    }
    if(domain==='naval'||domain==='designs')line=line.replace(/\btrain\s+ALL\b/gi,t(domain==='naval'?'Exercise all ships':'Exercise all air wings'));
    if(domain==='production'||/\b(?:design|produce)\b/i.test(raw))line=line.replace(/\b(F|C|NB|MF|SB|CNB|HNB|TD|FL|AR)(?:-I)?[- ]?\d+\b/g,(code,base)=>{
     if(!domains.designs?.[base])return code;
     const meaning=record(code,domains.designs[base]);return hold(t('{0} ({1} design)',code,meaning));
    });
    if(matcher)line=line.replace(matcher,(code,_,offset,whole)=>{
     let options=index.get(key(code)),scoped=options.filter(o=>o.domain===domain);
     // Company names are common in division compositions. Other domain-specific
     // meanings are never picked merely because they happen to be unique.
     if(!scoped.length&&['divisions','occupation'].includes(domain))scoped=options.filter(o=>o.domain==='support_companies'||domain==='occupation'&&o.domain==='divisions');
     if(!scoped.length&&domain==='divisions')scoped=options.filter(o=>o.domain==='designs');
     if(!scoped.length)scoped=options.filter(o=>o.domain==='general'||o.domain==='resources'||o.domain==='diplomacy'&&(/^(?:REL|PRE|Puppet\+|Balkanize\+)$/i.test(code)||/^\s+[A-Z]{3}\b/.test(whole.slice(offset+code.length))));
     // MIL means factories in both contexts, not two different targets.
     if(!scoped.length&&/^mil$/i.test(code))scoped=options.filter(o=>o.domain==='production');
     if(!scoped.length&&options.every(o=>o.domain==='production')&&/equip/i.test(code))scoped=options;
     if(!scoped.length){
      if(new Set(options.map(o=>o.meaning)).size>1)notes.add(t('Ambiguous abbreviation {0}: {1}. Check the original reference.',code,[...new Set(options.map(o=>t(o.meaning)))].join(' / ')));
      return code;
     }
     const meanings=[...new Set(scoped.map(o=>o.meaning))];if(meanings.length!==1)return code;
     if(scoped[0].domain==='divisions'&&['DEF','OFF','ARM'].includes(scoped[0].code)&&code!==scoped[0].code)return code;
     // Don't expand Art as a substring, or a template code in an ID/name.
     const meaning=record(code,meanings[0]);
     if(scoped[0].domain==='divisions')return t('{0} template ({1})',meaning,code);
     return t('{0} ({1})',meaning,code);
    });
    if(domain==='production'){
     line=line.replace(/\bALL\b/g,t('all remaining and future military factories'));
     if(/\bALL\b/.test(raw))notes.add(t(language.conventions?.production_remaining||'Keep the remaining-capacity production line last.'));
     if(/->|→|\bfrom\b/i.test(raw))notes.add(t(language.conventions?.production_order||'Listed production lines indicate priority; from indicates reassignment.'));
    }
    if(/\bUP\b/.test(raw))notes.add(t(language.conventions?.up||'The next useful level of the technology branch, even on its first mention.'));
    // The LAW convention names occupation law + garrison template, not two laws.
    if(domain==='occupation'&&/^LAW\s*:/i.test(raw)){
     const parts=line.replace(/^LAW\s*:\s*/i,'').split(/\s*\+\s*/);
     line=t('Occupation law: {0}',parts.shift());
     if(parts.length)line+='; '+t('garrison template: {0}',parts.join(' + '));
     if(/(?:…|\.{3})/.test(raw))notes.add(t('The template name is shortened in the source. Check the template reference; no exact identity is inferred.'));
    }else{
     if(heading)line=t(({ARMY:'Army',NAVY:'Navy','AIR FORCE':'Air force',MIL:'Military production',NAV:'Naval production',BUILD:'Construction',PRODUCTION:'Production'})[heading[1].toUpperCase()])+': '+line;
    }
    return restore(line.replace(/\s*->\s*/g,' → '));
   });
   return {text:output.join('\n'),terms:[...terms.values()],notes:[...notes]};
  }
  return {read};
 }
 const api={create};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiActionReading=api;
})(typeof window!=='undefined'?window:globalThis);
