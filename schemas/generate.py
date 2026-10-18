#!/usr/bin/env python3
"""Generate wire types. --check fails on drift without modifying files."""
import argparse, json, re
from pathlib import Path
from source import SCHEMA, COMMANDS, MODEL_BINDINGS, REGISTRY
ROOT=Path(__file__).resolve().parents[1]
def pascal(s): return ''.join(x[:1].upper()+(x[1:].lower() if x.isupper() else x[1:]) for x in re.split(r'[^a-zA-Z0-9]+',s) if x)
def ts(s):
 if '$ref' in s:return s['$ref'].split('/')[-1]
 if 'const' in s:return json.dumps(s['const'])
 if 'enum' in s:return ' | '.join(json.dumps(x) for x in s['enum'])
 if 'oneOf' in s or 'anyOf' in s:return '('+' | '.join(ts(x) for x in s.get('oneOf',s.get('anyOf')))+')'
 t=s.get('type')
 if t=='array':return 'Array<'+ts(s['items'])+'>'
 if t=='object':
  if 'properties' in s:return '{ '+ '; '.join(json.dumps(k)+('' if k in s.get('required',[]) else '?')+': '+ts(v) for k,v in s['properties'].items())+' }'
  return 'Record<string, '+ts(s.get('additionalProperties',{}))+'>'
 return {'string':'string','integer':'number','number':'number','boolean':'boolean','null':'null'}.get(t,'unknown')
extra={}
def rt(s,name):
 if '$ref' in s:return s['$ref'].split('/')[-1]
 if 'const' in s:
  v=s['const'];return 'bool' if isinstance(v,bool) else 'u64' if isinstance(v,int) else 'String'
 if 'enum' in s:
  if all(isinstance(v,str) for v in s['enum']):extra[name]=s;return name
  return 'u64'
 if 'anyOf' in s:
  non=[v for v in s['anyOf'] if v.get('type')!='null']
  if len(non)==1:return 'Option<'+rt(non[0],name+'Value')+'>'
  extra[name]=s;return name
 if 'oneOf' in s:extra[name]=s;return name
 t=s.get('type')
 if t=='array':return 'Vec<'+rt(s['items'],name+'Item')+'>'
 if t=='object':
  if 'properties' in s:extra[name]=s;return name
  return 'std::collections::BTreeMap<String, '+rt(s.get('additionalProperties',{}),name+'Value')+'>'
 return {'string':'String','integer':'u64' if s.get('minimum',-1)>=0 else 'i64','number':'f64','boolean':'bool','null':'()'}.get(t,'serde_json::Value')
def decl(n,s):
 if s.get('type')=='object' and 'properties' in s:
  out=['#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]','#[serde(deny_unknown_fields)]','pub struct '+n+' {']
  for k,v in s['properties'].items():
   typ=rt(v,n+pascal(k));opt=k not in s.get('required',[])
   if opt and not typ.startswith('Option<'):typ='Option<'+typ+'>'
   if opt:out+=['    #[serde(default, skip_serializing_if = "Option::is_none")]']
   out+=['    pub '+('r#'+k if k in ['type','match','ref','loop','mod','self','use'] else k)+': '+typ+',']
  return '\n'.join(out+['}'])
 if 'enum' in s and all(isinstance(x,str) for x in s['enum']):
  out=['#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]','pub enum '+n+' {']
  for x in s['enum']:
   variant=pascal(x)
   if not variant or variant[0].isdigit():variant='V'+variant
   out+=['    #[serde(rename = '+json.dumps(x)+')]','    '+variant+',']
  return '\n'.join(out+['}'])
 if 'oneOf' in s or 'anyOf' in s:
  opts=s.get('oneOf',s.get('anyOf'))
  return '\n'.join(['#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]','#[serde(untagged)]','pub enum '+n+' {']+['    V'+str(i)+'('+rt(v,n+'Variant'+str(i))+'),' for i,v in enumerate(opts)]+['}'])
 return 'pub type '+n+' = '+rt(s,n)+';'
def outputs():
 schema=json.dumps(SCHEMA,ensure_ascii=False,indent=2)+'\n'
 tsout='// Generated from schemas/source.py; do not edit.\n'+'\n'.join('export type '+n+' = '+ts(s)+';' for n,s in SCHEMA['$defs'].items())
 tsout+='\nexport interface CommandInputs {\n'+'\n'.join('  '+json.dumps(n)+': '+v['input']+';' for n,v in COMMANDS.items())+'\n}\n'
 tsout+='export interface CommandOutputs {\n'+'\n'.join('  '+json.dumps(n)+': '+v['output']+';' for n,v in COMMANDS.items())+'\n}\n'
 tsout+='export type CommandName = keyof CommandInputs;\n'
 rs=['// Generated from schemas/source.py; do not edit.']
 done=set()
 for n,s in SCHEMA['$defs'].items():rs.append(decl(n,s));done.add(n)
 while any(n not in done for n in extra):
  for n,s in list(extra.items()):
   if n not in done:rs.append(decl(n,s));done.add(n)
 return {'schemas/contracts.schema.json':schema,'schemas/registry.json':json.dumps(REGISTRY,indent=2)+'\n','packages/contracts/src/generated.ts':tsout,'crates/focaus-contracts/src/generated.rs':'\n\n'.join(rs)+'\n'}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');args=p.parse_args()
 bad=[]
 for rel,content in outputs().items():
  f=ROOT/rel
  if args.check:
   if not f.exists() or f.read_text()!=content:bad.append(rel)
  else:f.parent.mkdir(parents=True,exist_ok=True);f.write_text(content)
 if bad:raise SystemExit('Generated file drift: '+', '.join(bad))
 print('contract generation '+('check passed' if args.check else 'complete'))
