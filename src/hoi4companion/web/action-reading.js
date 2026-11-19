'use strict';
// Presentation only: shared vocabulary, no ID resolution or runtime decisions.
((root)=>{
 const i18n=typeof module!=='undefined'&&module.exports?require('./i18n.js'):root.HoiI18n;
 const t=(s,...args)=>i18n.t(s,...args),key=s=>s.toLocaleLowerCase('en'),escape=s=>s.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 function create(language={}){
  const domains=language.abbreviations||{},index=new Map();
  function addCodes(domain,codes,uncertain=false){for(const [code,meaning] of Object.entries(codes)){
   if(typeof meaning!=='string'||!code||code.length>80)continue;
   const k=key(code);if(!index.has(k))index.set(k,[]);index.get(k).push({code,meaning,domain,uncertain});
  }}
  for(const [domain,codes] of Object.entries(domains))addCodes(domain,codes);
  for(const [domain,codes] of Object.entries(language.conventions?.possible_abbreviations||{})){
   // A current confirmed entry takes precedence over obsolete possible metadata.
   addCodes(domain,Object.fromEntries(Object.entries(codes).filter(([code])=>!domains[domain]?.[code])),true);
  }
  const notation=language.conventions?.design_notation||{},ground=notation.ground_codes||{};
  const designBases=[...new Set([...Object.keys(ground),'F','C','NB','MF','SB','CNB','HNB'])].sort((a,b)=>b.length-a.length);
  const designMatcher=new RegExp('(?<![\\p{L}\\p{N}_])('+designBases.map(escape).join('|')+')(-'+escape(notation.infantry_suffix||'I')+')?[- ]?(\\d+)('+escape(notation.cannon_suffix||'ca')+')?(?![\\p{L}\\p{N}_])','gu');
  const reconVariants=notation.recon_variants||{},reconCodes=Object.keys(reconVariants).sort((a,b)=>b.length-a.length);
  const reconMatcher=reconCodes.length?new RegExp('(?<![\\p{L}\\p{N}_])('+reconCodes.map(escape).join('|')+')(?![\\p{L}\\p{N}_])','gu'):null;
  // One compiled matcher per guide, reused while live state changes.
  const codes=[...index.keys()].filter(c=>c.length>1).sort((a,b)=>b.length-a.length).map(escape);
  const matcher=codes.length?new RegExp('(?<![\\p{L}\\p{N}_])('+codes.join('|')+')(?![\\p{L}\\p{N}_])','giu'):null;
  const divis=Object.keys(domains.divisions||{}).sort((a,b)=>b.length-a.length).map(escape);
  const variants=divis.length?new RegExp('\\b('+divis.join('|')+')([ast]{1,3})(?![\\p{L}\\p{N}_])','gu'):null;
  const recruiting=divis.length?new RegExp('\\brecruit\\s+(\\d+)\\s+(?=(?:'+divis.join('|')+')(?:[ast]{1,3})?\\b)','gi'):null;
  function read(text,row={}){
   if(typeof text!=='string')return {text:'',terms:[],notes:[]};
   const p=row.action?.params||{},terms=new Map(),notes=new Set(),protectedText=[],sourceText=[];
   // Every rewritten span is held; its source spelling is kept for lines that
   // are not understood in full and are therefore shown unchanged.
   const hold=(s,source=s)=>{sourceText.push(source);return '\uE000'+(protectedText.push(s)-1)+'\uE001';};
   const record=(code,meaning)=>{terms.set(code,{code,meaning});return t(meaning);};
   const designLabel=(base,infantry,cannons)=>{
    const meaning=ground[base]||domains.designs?.[base];if(!meaning)return null;
    let label=t(meaning);
    if(infantry)label=t('{0}, infantry variant',label);
    if(cannons)label=t('{0}, with cannons',label);
    return label;
   };
   const designMeaning=(code,base,infantry,number,cannons)=>{
    const label=designLabel(base,infantry,cannons);if(!label)return code;
    record(code,label);return t('{0} ({1} design)',code,label);
   };
   const restore=s=>s.replace(/\uE000(\d+)\uE001/g,(_,n)=>protectedText[Number(n)]);
   const original=s=>s.replace(/\uE000(\d+)\uE001/g,(_,n)=>sourceText[Number(n)]);
   // Understood in full: no source letters remain outside rewritten spans.
   const understood=s=>!/\p{L}/u.test(s.replace(/\uE000\d+\uE001/g,''));
   // Preserve bound spelling and quotations. Only documented coded designs get
   // appended explanations; references retain their complete original names.
   const names=[p.variant_name,p.person_name,p.law_name,p.project_name,p.template_name,
    p.design_reference?.name,...(Array.isArray(p.reference_images)?p.reference_images:[]).map(r=>r?.name),
    ...(Array.isArray(p.production_targets)?p.production_targets:[]).flatMap(r=>[r?.variant_name,r?.design_reference?.name])]
    .filter(s=>typeof s==='string'&&s.length>1).sort((a,b)=>b.length-a.length);
   let input=text.replace(/"[^"\n]+"|“[^”\n]+”/g,quote=>hold(quote));
   if(names.length)input=input.replace(new RegExp('(?<![\\p{L}\\p{N}_])(?:'+names.map(escape).join('|')+')(?![\\p{L}\\p{N}_])','gu'),name=>{
    // Explain a confirmed naval code even when it is also a design-reference
    // name; preserve that exact code and never change the reference identity.
    if((p.category==='production'||/\b(?:NAVY|NAV)\s*:/i.test(text))&&domains.naval?.[name])return hold(t('{0} ({1})',record(name,domains.naval[name]),name),name);
    if(p.category==='production'||/\b(?:design|produce|ARMY|AIR FORCE)\b/i.test(text)){
     designMatcher.lastIndex=0;const match=designMatcher.exec(name);designMatcher.lastIndex=0;
     const tail=match?name.slice(match[0].length):'';
     if(match?.index===0&&(!tail||/^\s*\([\dA-Z /-]+\)$/.test(tail))){
      let label=designLabel(match[1],match[2],match[4]);
      if(label){
       for(const code of new Set(tail.match(/\b(?:MG|SL)\b/g)||[]))if(domains.designs?.[code])label+=' · '+t(domains.designs[code]);
       return hold(t('{0} ({1} design)',name,record(name,label)),name);
      }
     }
    }
    return hold(name);
   });
   const lines=input.split(/\n|\s+[-•]\s+(?=(?:ARMY|NAVY|LAW|BUILD|MIL|NAV|ASAP)\b)/i).filter(s=>s.trim());
   const output=lines.map(raw=>{
    let line=raw.replace(/^\s*[-•]\s*/,'').trim(),domain=p.category||row.category||'';
    const source=line,rewrite=text=>all=>hold(t(text),all);
    line=line.replace(/\bBuild\s+INFRAs?\s*\+\s*MILs?\b/gi,rewrite('Build infrastructure, then military factories, in each indicated region before moving to the next'))
     .replace(/\bBuild\s+INFRAs?\s*,?\s+then\s+MILs?\b/gi,rewrite('Build infrastructure in all indicated regions first, then military factories'))
     .replace(/\bOn\s+free\s+CIV\b/gi,rewrite('When civilian factories become available'));
    const scopedLine=line.replace(/^(?:ASAP|PRIO)\s*:\s*/i,'');
    if(/^(?:MIL|NAV|PRODUCTION)\s*:|\b(?:produce|production)\b/i.test(scopedLine))domain='production';
    else if(/^(?:BUILD|CONSTRUCT)\b/i.test(scopedLine))domain='construction';
    else if(/^LAW\s*:|\boccupation\b/i.test(scopedLine))domain='occupation';
    else if(/^(?:ARMY|NAVY|AIR FORCE)\s*:|\b(?:recruit|divisions?|battalions?|armies|garrison|template|train|exercise|bring|convert|duplicate|replace|swap|send|change|VOL)\b|\b\d+w\b/i.test(scopedLine))domain='divisions';
    if(/^NAVY\s*:/i.test(scopedLine))domain='naval';
    if(/^AIR FORCE\s*:/i.test(scopedLine))domain='designs';
    // A doctrine qualifier is essential: SD and BS have different meanings.
    const doctrine=line.match(/\b(land|air|naval)\s+doc(?:trines?)?\b/i);
    if(doctrine)domain=key(doctrine[1])+'_doctrines';
    // A leading "Land Doc:" is a section label, like ARMY:, not prose.
    line=line.replace(/^(?:land|air|naval)\s+doc(?:trines?)?\s*:/i,label=>hold(label));
    if(domain==='army')domain='divisions';
    const navalProduction=/^NAV\s*:/i.test(scopedLine)||domain==='naval'&&/:\s*(?:\d+|ALL)\b/.test(scopedLine);
    if(domain==='construction')line=line.replace(/\b(INFRA|MIL|CIV)s\b/gi,(code,base)=>{
     const meaning=language.conventions?.construction_plurals?.[base.toUpperCase()]||domains.construction?.[base.toUpperCase()];
     return meaning?hold(t('{0} ({1})',record(code,meaning),code),code):code;
    });
    const heading=line.match(/^(ARMY|NAVY|AIR FORCE|MIL|NAV|BUILD|PRODUCTION)\s*:\s*/i);
    if(heading)line=line.slice(heading[0].length);
    line=line.replace(/\b(\d+(?:\.\d+)?)\s*(PP|AE|NE|CP)\b/gi,(all,n,code)=>domains.resources?.[code.toUpperCase()]?hold(t('{0} {1}',n,record(code.toUpperCase(),domains.resources[code.toUpperCase()])),all):all);
    if(domain==='divisions'||domain==='occupation'){
     if(recruiting)line=line.replace(recruiting,(all,n)=>hold(t('Recruit')+' '+n+' '+t('Divisions').toLocaleLowerCase()+' · ',all));
     line=line.replace(/^(Bring\s+)(.+?)(\s+to\s+)(?=\d+w\b)/i,(_,bring,name,to)=>hold(t('Template')+': ',bring)+name+hold(' · ',to));
     line=line.replace(/\b(\d+)\s*w\b/gi,(all,n)=>hold(t('combat width {0}',n),all));
     if(variants)line=line.replace(variants,(all,base,suffix)=>{
      const extra=[...new Set(suffix)].map(c=>language.conventions?.division_suffixes?.[c]);
      if(extra.some(s=>!s))return all;
      const meaning=t('{0} with {1}',t(domains.divisions[base]),extra.map(s=>t(s)).join(' + '));
      record(all,meaning);return hold(t('{0} template ({1})',meaning,all),all);
     });
     line=line.replace(/\bchange\s+ALL\s+to\b/gi,rewrite('Change all divisions to'))
      .replace(/\btrain\s+ALL\b/gi,rewrite('Exercise all divisions'));
     line=line.replace(/\b(recruit|duplicate|train)\b/gi,word=>hold(t(({recruit:'Recruit',duplicate:'Duplicate',train:'Exercise'})[key(word)]),word));
    }
    if(domain==='naval'||domain==='designs')line=line.replace(/\btrain\s+ALL\b/gi,rewrite(domain==='naval'?'Exercise all ships':'Exercise all air wings'));
    if(['production','designs','divisions'].includes(domain)||/\b(?:design|produce)\b/i.test(raw)){
     line=line.replace(designMatcher,(...args)=>hold(designMeaning(...args.slice(0,5)),args[0]));
     if(reconMatcher)line=line.replace(reconMatcher,code=>hold(t('{0} ({1})',record(code,reconVariants[code]),code),code));
    }
    if(matcher)line=line.replace(matcher,(code,_,offset,whole)=>{
     let options=index.get(key(code)),scoped=options.filter(o=>o.domain===domain);
     // Company names are common in division compositions. Other domain-specific
     // meanings are never picked merely because they happen to be unique.
     if(!scoped.length&&['divisions','occupation'].includes(domain))scoped=options.filter(o=>o.domain==='support_companies'||o.domain==='regimental_companies'||domain==='occupation'&&o.domain==='divisions');
     if(!scoped.length&&domain==='divisions')scoped=options.filter(o=>o.domain==='designs');
     if(!scoped.length&&domain==='production'&&/\b(?:design|produce)\b/i.test(raw))scoped=options.filter(o=>o.domain==='designs');
     if(!scoped.length&&navalProduction)scoped=options.filter(o=>o.domain==='naval');
     if(!scoped.length)scoped=options.filter(o=>o.domain==='general'||o.domain==='resources'||o.domain==='diplomacy'&&(/^(?:REL|PRE|Puppet\+|Balkanize\+)$/i.test(code)||/^\s+[A-Z]{3}\b/.test(whole.slice(offset+code.length))));
     // MIL means factories in both contexts, not two different targets.
     if(!scoped.length&&/^mil$/i.test(code))scoped=options.filter(o=>o.domain==='production');
     if(!scoped.length&&options.every(o=>o.domain==='production')&&/equip/i.test(code))scoped=options;
     // A distinctive exact author code can be explained even in an unclassified
     // instruction. This is display text, never an entity or completion binding.
     if(!scoped.length&&/^[A-Z]{2,}[a-z]{0,2}$/.test(code)&&!['OFF','DEF','ARM'].includes(code)){
      const exact=options.filter(o=>o.code===code&&o.domain!=='diplomacy'&&!o.domain.endsWith('_doctrines'));
      if(new Set(exact.map(o=>o.meaning)).size===1)scoped=exact;
     }
     if(!scoped.length){
      const known=[...new Set(options.map(o=>o.meaning))];
      if(known.length>1)notes.add(t('Ambiguous abbreviation {0}: {1}. Check the original reference.',code,known.map(m=>t(m)).join(' / ')));
      // One known meaning in another domain: shown as a possible reading in the
      // glossary notes, never substituted into the instruction.
      else if(known.length===1)notes.add(t('Possible meaning of {0}: {1}. Check the original reference.',code,t(known[0])));
      return code;
     }
     const meanings=[...new Set(scoped.map(o=>o.meaning))];if(meanings.length!==1)return code;
     if(scoped[0].code==='UP'&&code!=='UP')return code;
     if(scoped.some(o=>o.uncertain)){
      notes.add(t('Possible meaning of {0}: {1}. Check the original reference.',code,t(meanings[0])));
      return code;
     }
     if(scoped[0].domain==='divisions'&&['DEF','OFF','ARM'].includes(scoped[0].code)&&code!==scoped[0].code)return code;
     // Don't expand Art as a substring, or a template code in an ID/name.
     const meaning=record(code,meanings[0]);
     if(scoped[0].domain==='divisions')return hold(t('{0} template ({1})',meaning,code),code);
     return hold(t('{0} ({1})',meaning,code),code);
    });
    if(domain==='production'||navalProduction){
     const capacity=t(navalProduction?'naval dockyards':'military factories');
     line=line.replace(/\b0\s*(?:->|→)\s*ALL\b/g,all=>hold(t('Remove this line’s assignments, then allocate all remaining and future {0} (0→ALL)',capacity),all));
     line=line.replace(/(?:->|→)\s*(\d+)\b/g,(all,n)=>hold(t('→ bring to {0} {1}',n,capacity),all));
     line=line.replace(/\bALL\b/g,rewrite(navalProduction?'all remaining and future naval dockyards':'all remaining and future military factories'));
     if(/\bALL\b/.test(raw))notes.add(t(navalProduction?(language.conventions?.naval_production_remaining||'ALL assigns all remaining and future naval dockyards to this line; keep it last in priority.'):(language.conventions?.production_remaining||'Keep the remaining-capacity production line last.')));
     if(/->|→|\bfrom\b/i.test(raw))notes.add(t(language.conventions?.production_order||'Listed production lines indicate priority; from indicates reassignment.'));
    }
    // The LAW convention names occupation law + garrison template, not two laws.
    if(domain==='occupation'&&/^LAW\s*:/i.test(raw)){
     const parts=line.replace(/^LAW\s*:\s*/i,'').split(/\s*\+\s*/);
     line=t('Occupation law: {0}',parts.shift());
     if(parts.length)line+='; '+t('garrison template: {0}',parts.join(' + '));
     if(/(?:…|\.{3})/.test(raw))notes.add(t('The template name is shortened in the source. Check the template reference; no exact identity is inferred.'));
    }else{
     // A partly understood line is never rewritten mid-sentence: the source
     // stays as written and its explanations go to the glossary beside it.
     if(!understood(line))return restore(original(source));
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
