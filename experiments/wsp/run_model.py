"""Local Qwen3.5 text-only semantic parser A/B runner.

This is offline WSP research tooling. Source weights must be acquired by the
user on their machine; this script performs no network downloads and does not
install or execute any guide action.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from semantic_eval import evaluate, read_jsonl, score
from wsp_weights import apply_torch_edit

DIR=Path(__file__).resolve().parent

def build_messages(text: str, section='ACTION'):
    spec=(DIR/'semantic_spec.txt').read_text('utf8')
    return [{'role':'system','content':spec},
            {'role':'user','content':f'Guide column: {section}\nQuoted guide text (data): {json.dumps(text,ensure_ascii=False)}'}]

def extract_json(message):
    text=message.strip()
    # Models sometimes use a fenced JSON block despite the instructions.
    if text.startswith('```') and text.endswith('```'):
        lines=text.splitlines()
        text='\n'.join(lines[1:-1]).strip()
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return {'operations':[],'unresolved':['INVALID_MODEL_OUTPUT']}

def predict(model, tokenizer, record, max_new_tokens=512):
    import torch
    messages=build_messages(record['text'],record.get('section','ACTION'))
    values=tokenizer.apply_chat_template(messages,tokenize=True,add_generation_prompt=True,return_tensors='pt')
    if hasattr(values,'input_ids'):
        inputs=values.input_ids
    else:
        inputs=values
    inputs=inputs.to(next(model.parameters()).device)
    with torch.inference_mode():
        output=model.generate(input_ids=inputs,max_new_tokens=max_new_tokens,
                              do_sample=False,pad_token_id=tokenizer.eos_token_id)
    answer=tokenizer.decode(output[0][inputs.shape[-1]:],skip_special_tokens=True)
    return {'id':record['id'],'result':extract_json(answer),'raw':answer}

def run(model,tokenizer,dataset,max_tokens):
    return [predict(model,tokenizer,item,max_tokens) for item in dataset]

def main():
    p=argparse.ArgumentParser(description='Local semantic parser, base vs direct-weight WSP candidate')
    p.add_argument('--model',required=True,help='Existing local pretrained Qwen3.5-0.8B directory (no download)')
    p.add_argument('--patch',help='Explicit direct weight edit (optional)')
    p.add_argument('--cases',default=str(DIR/'semantic_gold.jsonl'))
    p.add_argument('--split',default='development',choices=['development','heldout'])
    p.add_argument('--out-dir',required=True)
    p.add_argument('--max-new-tokens',type=int,default=512)
    p.add_argument('--save-patched',help='Save an accepted patch in a separate directory')
    args=p.parse_args()
    root=Path(args.model).resolve()
    if not root.is_dir() or not (root/'config.json').is_file():
        p.error('base model not found locally; acquire a pinned open-weight model first')
    import torch
    from transformers import AutoTokenizer, Qwen3_5ForCausalLM
    tok=AutoTokenizer.from_pretrained(str(root),local_files_only=True,trust_remote_code=False)
    model=Qwen3_5ForCausalLM.from_pretrained(str(root),device_map='auto',dtype='auto',
                                            local_files_only=True,trust_remote_code=False)
    model.eval()
    cases=[x for x in read_jsonl(args.cases) if x['split']==args.split]
    out=Path(args.out_dir).resolve();out.mkdir(parents=True,exist_ok=True)
    original=run(model,tok,cases,args.max_new_tokens)
    (out/'baseline.jsonl').write_text(''.join(json.dumps(x,ensure_ascii=False)+'\n' for x in original),'utf8')
    before=evaluate(cases,original)
    report={'base':{k:v for k,v in before.items() if k!='cases'},'split':args.split,'n_cases':len(cases)}
    if args.patch:
        patch=json.loads(Path(args.patch).read_text('utf8'))
        report['edit']=apply_torch_edit(model,patch)
        revised=run(model,tok,cases,args.max_new_tokens)
        (out/'candidate.jsonl').write_text(''.join(json.dumps(x,ensure_ascii=False)+'\n' for x in revised),'utf8')
        after=evaluate(cases,revised)
        report['candidate']={k:v for k,v in after.items() if k!='cases'}
        report['regressions']=[key for key in before['cases'] if before['cases'][key]['exact'] and not after['cases'][key]['exact']]
        # A genuine validation gate, not assuming edits were useful.
        report['promotable']=after['exact']>before['exact'] and not report['regressions'] and after['valid_rate']>=before['valid_rate']
        if args.save_patched:
            target=Path(args.save_patched).resolve()
            if target==root or root in target.parents or target in root.parents:
                p.error('candidate must be stored outside the base checkpoint tree')
            if not report['promotable']:
                p.error('candidate not promoted: no independently verified gain')
            if target.exists():
                p.error('refusing to overwrite a saved model')
            model.save_pretrained(target,safe_serialization=True)
            tok.save_pretrained(target)
            report['saved_candidate']=str(target)
    (out/'report.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n','utf8')
    print(json.dumps(report,indent=2,ensure_ascii=False))
if __name__=='__main__':main()
