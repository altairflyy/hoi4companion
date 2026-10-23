(function(root){
  'use strict';
  function filterDiagnostics(report,category,search){
    const query=String(search||'').trim().toLocaleLowerCase();
    return (report.diagnostics||[]).filter(row=>(!category||row.category===category)&&
      (!query||[row.file,row.path,row.message,row.value].join(' ').toLocaleLowerCase().includes(query)));
  }
  function summaryText(report){
    const coverage=report.coverage||{};
    return `Copertura parziale: ${coverage.recognized||0} istruzioni riconosciute; ${coverage.unsupported||0} controlli non verificabili; ${coverage.unknown_scope||0} contesti sconosciuti. I contatori si sovrappongono: non sommarli per ottenere una percentuale di copertura.`;
  }
  function mount(document,fetcher){
    const el=id=>document.getElementById(id);
    let report=null,request=0,ruleRequest=0;
    function add(parent,text){const row=document.createElement('li');row.textContent=text;parent.appendChild(row);}
    function applyFilters(){
      el('diagnostics').replaceChildren();
      if(!report){el('count').textContent='';return;}
      const rows=filterDiagnostics(report,el('category').value,el('search').value);
      el('count').textContent=`${rows.length} segnalazioni mostrate${report.truncated?' · elenco limitato; i totali includono anche quelle omesse':''}`;
      for(const row of rows){
        const item=document.createElement('li');
        const title=document.createElement('strong');title.textContent=row.message;
        const detail=document.createElement('p');detail.className='hint';
        detail.textContent=`${row.severity==='error'?'Errore':'Avviso'} · ${row.file} · ${row.path} · ${row.value}`;
        item.appendChild(title);item.appendChild(detail);el('diagnostics').appendChild(item);
      }
    }
    async function load(refresh=false){
      const current=++request;report=null;applyFilters();
      el('summary').textContent='';el('coverage').textContent='';el('limitations').replaceChildren();
      el('error').hidden=true;el('analyze').disabled=true;el('status').textContent='Analisi in corso; le viste della partita restano disponibili…';
      try{
        const response=await fetcher('/api/script-analysis'+(refresh?'?refresh=1':''));
        const data=await response.json();if(!response.ok)throw new Error(data.error||'Analisi non disponibile');
        if(current!==request)return;
        report=data;
        const count=data.summary||{};
        el('summary').textContent=`Avvisi sui riferimenti: ${count.references||0} · Contesti: ${count.scopes||0} · Parametri: ${count.parameters||0} · Localizzazioni: ${count.localisation||0}`;
        el('coverage').textContent=summaryText(data);
        el('provenance').textContent=`Regole CWTools ${data.rules.revision.slice(0,12)} · ${data.coverage.documents} file · ${data.coverage.entities} entità · nessuna valutazione della partita`;
        for(const warning of data.file_warnings||[])add(el('limitations'),warning);
        for(const warning of data.rules.load_errors||[])add(el('limitations'),warning.file+': '+warning.error);
        for(const [key,count] of Object.entries(data.coverage.unhandled||{}))add(el('limitations'),`${key}: ${count}`);
        el('status').textContent=data.demo?'Risultati dimostrativi: nessuna installazione reale caricata.':'Analisi disponibile. I dati restano in cache fino a «Ricarica» o «Ripeti analisi».';
        applyFilters();
      }catch(error){if(current===request){el('error').textContent=error.message;el('error').hidden=false;el('status').textContent='Analisi non disponibile.';}}
      finally{if(current===request)el('analyze').disabled=false;}
    }
    async function lookup(){
      const current=++ruleRequest;el('ruleDefinition').textContent='';el('ruleInfo').textContent='Ricerca…';
      try{
        const response=await fetcher('/api/script-rules?kind='+encodeURIComponent(el('ruleKind').value)+'&name='+encodeURIComponent(el('ruleName').value.trim()));
        const data=await response.json();if(!response.ok)throw new Error(data.error||'Definizione non disponibile');
        if(current!==ruleRequest)return;
        el('ruleInfo').textContent=data.variants.length?`Contesti dichiarati: ${data.scopes.join(', ')||'non specificati'}. La definizione non dimostra che la condizione sia soddisfatta.`:'Nome non presente nelle regole incluse; può essere una definizione personalizzata della mod.';
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
