(function(root){
  'use strict';
  const HoiI18n=typeof module!=='undefined'&&module.exports?require('./i18n.js'):root.HoiI18n;
  function filterDiagnostics(report,category,search){
    const query=String(search||'').trim().toLocaleLowerCase();
    return (report.diagnostics||[]).filter(row=>(!category||row.category===category)&&
      (!query||[row.file,row.path,row.message,row.value].join(' ').toLocaleLowerCase().includes(query)));
  }
  function summaryText(report){
    const coverage=report.coverage||{};
    if(coverage.metrics_available===false){
      if(coverage.ignored_parser_errors!==undefined){
        const ignored=coverage.ignored_parser_errors;
        const remaining=coverage.parser_errors-ignored;
        const log=report.source_error_log;
        const saved=log?.status==='saved'?HoiI18n.t(" Log: {0}.",log.path):log?.status==='error'?HoiI18n.t(" Log not saved: see warnings."):'';
        const recovered=coverage.recovered_sources||0;
        const recovery=recovered?HoiI18n.t(" {0} files recovered in memory using content-verified corrections; properties without values remain omitted.",recovered):'';
        return HoiI18n.t("CWTools engine: {0} documents analyzed; {1} parsing {2} to investigate. {3} files rejected by both parsers were set aside; analysis continues on the remaining files.{4}{5} Instruction coverage remains unknown and excluded files have not been verified. Static diagnostics do not verify the current game.",coverage.documents,remaining,remaining===1?HoiI18n.t('error'):HoiI18n.t('errors'),ignored,recovery,saved);
      }
      const both=(report.parser_error_details||[]).filter(row=>row.classification==='rejected_by_both').length;
      const detail=both?HoiI18n.t(" {0} sources were rejected by both parsers.",both):'';
      return HoiI18n.t("CWTools engine: {0} documents analyzed; {1} parsing errors.{2} The engine does not report a count of unverifiable instructions: this coverage remains unknown. Static diagnostics do not verify the current game.",coverage.documents,coverage.parser_errors,detail);
    }
    return HoiI18n.t("Partial coverage: {0} recognized instructions; {1} unverifiable checks; {2} unknown scopes. Counts overlap: do not add them to calculate coverage.",coverage.recognized||0,coverage.unsupported||0,coverage.unknown_scope||0);
  }
  function mount(document,fetcher){
    const el=id=>document.getElementById(id);
    let report=null,request=0,ruleRequest=0;
    function add(parent,text){const row=document.createElement('li');row.textContent=text;parent.appendChild(row);}
    function applyFilters(){
      el('diagnostics').replaceChildren();
      if(!report){el('count').textContent='';return;}
      const rows=filterDiagnostics(report,el('category').value,el('search').value);
      el('count').textContent=`${rows.length} segnalazioni mostrate${report.truncated?HoiI18n.t(" · limited list; totals include omitted entries"):''}`;
      for(const row of rows){
        const item=document.createElement('li');
        const title=document.createElement('strong');title.textContent=row.message;
        const detail=document.createElement('p');detail.className='hint';
        detail.textContent=`${row.severity==='error'?HoiI18n.t("Error"):HoiI18n.t("Warning")} · ${row.file} · ${row.path} · ${row.value}`;
        item.appendChild(title);item.appendChild(detail);el('diagnostics').appendChild(item);
      }
    }
    async function load(refresh=false){
      const current=++request;report=null;applyFilters();
      el('summary').textContent='';el('coverage').textContent='';el('limitations').replaceChildren();
      el('error').hidden=true;el('analyze').disabled=true;el('status').textContent=HoiI18n.t("Analysis in progress; game views remain available…");
      try{
        const response=await fetcher('/api/script-analysis'+(refresh?'?refresh=1':''));
        const data=await response.json();if(!response.ok)throw new Error(data.error||HoiI18n.t("Analysis unavailable"));
        if(current!==request)return;
        report=data;
        const count=data.summary||{};
        el('summary').textContent=HoiI18n.t("Reference warnings: {0} · Scopes: {1} · Parameters: {2} · Localization: {3}",count.references||0,count.scopes||0,count.parameters||0,count.localisation||0);
        el('coverage').textContent=summaryText(data);
        el('provenance').textContent=HoiI18n.t("{0}CWTools rules {1} · {2} files · {3} entities · no evaluation of the current game",data.rules.engine?'Motore CWTools '+data.rules.engine_revision.slice(0,12)+' · ':'',data.rules.revision.slice(0,12),data.coverage.documents,data.coverage.entities);
        for(const warning of data.file_warnings||[])add(el('limitations'),warning);
        for(const file of data.non_script_files||[])add(el('limitations'),HoiI18n.t("Non-script document: {0}. Indexed without interpreting it as code.",file));
        for(const item of data.source_recoveries||[])add(el('limitations'),HoiI18n.t("Recovered in memory: {0}. {1}",item.file,item.evidence.omitted_properties?HoiI18n.t("Property without a value omitted; no value was invented."):HoiI18n.t("Structure recovered using a correction verified against the exact hash.")));
        if(data.input_adjustments?.length)add(el('limitations'),HoiI18n.t("Jomini → CWTools lexical compatibility: {0} files adapted in memory; game files remain unchanged.",data.input_adjustments.length));
        for(const warning of data.rules.load_errors||[])add(el('limitations'),warning.file+': '+warning.error);
        for(const [key,count] of Object.entries(data.coverage.unhandled||{}))add(el('limitations'),`${key}: ${count}`);
        el('status').textContent=data.demo?HoiI18n.t("Demo results: no real installation loaded."):HoiI18n.t("Analysis available. Results remain cached until you select Reload or Run analysis again.");
        applyFilters();
      }catch(error){if(current===request){el('error').textContent=error.message;el('error').hidden=false;el('status').textContent=HoiI18n.t("Analysis unavailable.");}}
      finally{if(current===request)el('analyze').disabled=false;}
    }
    async function lookup(){
      const current=++ruleRequest;el('ruleDefinition').textContent='';el('ruleInfo').textContent='Ricerca…';
      try{
        const response=await fetcher('/api/script-rules?kind='+encodeURIComponent(el('ruleKind').value)+'&name='+encodeURIComponent(el('ruleName').value.trim()));
        const data=await response.json();if(!response.ok)throw new Error(data.error||HoiI18n.t("Definition unavailable"));
        if(current!==ruleRequest)return;
        el('ruleInfo').textContent=data.variants.length?HoiI18n.t("Declared scopes: {0}. The definition does not prove that the condition is met.",data.scopes.join(', ')||HoiI18n.t("not specified")):HoiI18n.t("Name not found in the bundled rules; it may be a custom mod definition.");
        el('ruleDefinition').textContent=data.variants.map(v=>(v.description||'')+'\n'+JSON.stringify(v.schema,null,2)).join('\n\nAlternativa:\n');
      }catch(error){if(current===ruleRequest)el('ruleInfo').textContent=error.message;}
    }
    el('analyze').addEventListener('click',()=>load(true));
    el('category').addEventListener('change',applyFilters);el('search').addEventListener('input',applyFilters);
    el('lookup').addEventListener('click',lookup);
    return {load,lookup,applyFilters};
  }
  const api={filterDiagnostics,summaryText,mount};
  if(typeof module!=='undefined'&&module.exports)module.exports=api;
  else {root.ScriptAnalysis=api;mount(root.document,root.fetch.bind(root)).load();}
})(typeof window!=='undefined'?window:globalThis);
