"""Shared source-side validation for fixtures and engine packages. Never performs I/O on user assets."""
import hashlib, json, math, rfc8785
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker
from source import SCHEMA, REGISTRY, COMMANDS
class ContractError(ValueError):
 def __init__(self,code,reason):self.code=code;super().__init__(reason)
def canonical_hash(value):
 return hashlib.sha256(rfc8785.dumps(value)).hexdigest()
def search_text_hash(text):
 return hashlib.sha256(text.encode('utf-8')).hexdigest()
def fail(reason,code='INVALID_RESULT'):raise ContractError(code,reason)
def finite(v):
 if isinstance(v,float) and not math.isfinite(v):fail('non-finite number')
 if isinstance(v,dict):
  for x in v.values():finite(x)
 elif isinstance(v,list):
  for x in v:finite(x)
def validate(kind,value):
 finite(value)
 validator=Draft202012Validator({'$ref':'#/$defs/'+kind,'$defs':SCHEMA['$defs']},format_checker=FormatChecker())
 errors=list(validator.iter_errors(value))
 if errors:fail('schema mismatch at '+ '/'.join(map(str,errors[0].absolute_path)))
 if kind=='Command':validate(COMMANDS[value['name']]['input'],value['payload'])
 if kind in ['ScalarRange','FrameRange','TimelineRange']:
  a,b={'ScalarRange':('start','end'),'FrameRange':('start_frame','end_frame'),'TimelineRange':('start_ms','end_ms')}[kind]
  if value[b]<value[a]:fail('range reversed')
 if kind=='EventBatch':
  seqs=[e['global_seq'] for e in value['events']]
  if value['after_seq']>value['through_seq'] or seqs!=sorted(set(seqs)) or any(s<=value['after_seq'] or s>value['through_seq'] for s in seqs):fail('event batch watermarks')
 if kind=='AudioSpan':
  validate('TimelineRange',value['timeline_range'])
  if value['coverage_status'] in ['captured','silence_observed'] and any(value[k] is None for k in ['asset_id','track_id','frame_range','mapping_revision_id']):fail('captured span requires asset and mapping')
 if kind=='SourceEdge' and value['to']['selector']['kind']=='material_region' and value['role'] in ['teacher_said','teacher_path']:fail('material cannot prove teacher speech')
 if kind=='Conv2dParameters':
  inp=value['input'];kernel=value['kernel']
  if not inp or not kernel or len(inp)!=len(kernel):fail('channel mismatch')
  def shape(t):
   if not t or not t[0] or not t[0][0]:fail('empty tensor')
   h,w=len(t[0]),len(t[0][0])
   if any(len(c)!=h or any(len(r)!=w for r in c) for c in t):fail('ragged tensor')
   return len(t),h,w
  c,h,w=shape(inp);_,kh,kw=shape(kernel)
  if value['dilation']!=[1,1]:fail('dilation unsupported','UNSUPPORTED_CAPABILITY')
  p=value['padding'];s=value['stride'];oh=(h+p[0]+p[1]-kh)//s[0]+1;ow=(w+p[2]+p[3]-kw)//s[1]+1
  if oh<=0 or ow<=0:fail('non-positive output')
  if c*(h*w+kh*kw)+oh*ow>65536 or oh*ow*c*kh*kw>1000000:fail('computation budget exceeded','UNSUPPORTED_CAPABILITY')
 if kind=='CurveParameters' and value['x_max']<=value['x_min']:fail('empty curve domain')
 if kind=='VisualizationContent':
  spec=REGISTRY['templates'].get(value['template_id'])
  if not spec:
   if value['generation_status']!='unsupported':fail('unknown template must be unsupported','UNSUPPORTED_CAPABILITY')
  else:
   if value['template_version']!=spec['version']:fail('unsupported template version','UNSUPPORTED_CAPABILITY')
   validate(spec['parameter_type'],value['parameters'])
  if value['generation_status']=='ready' and (value['validation']['parameters_hash']!=canonical_hash(value['parameters']) or value['validation']['validated_revision']!=value['revision_id']):fail('validation is bound to another revision/parameters')
  for bind in value['value_bindings']:
   if bind['value_origin']=='classroom' and not bind['source_refs']:fail('classroom value lacks evidence')
   if bind['value_origin']=='derived' and not bind['dependency_ids']:fail('derived value lacks dependencies')
   if bind['value'] is None and not bind['unknown_reason']:fail('null value requires reason')
 if kind=='MaterialManifest':
  ids=[x['object_id'] for x in value['objects']]+[x['page_id'] for x in value['pages']]+[x['region_id'] for x in value['regions']]+[x['resource_id'] for x in value['resources']]
  if len(ids)!=len(set(ids)):fail('duplicate identities')
  object_ids={x['object_id'] for x in value['objects']};region_ids={x['region_id'] for x in value['regions']};page_ids={x['page_id'] for x in value['pages']};resources={x['resource_id'] for x in value['resources']}
  for page in value['pages']:
   if not set(page['region_ids'])<=region_ids or not set(page['reading_order'])<=(object_ids|region_ids):fail('page closure')
   if page['preview_resource_id'] is not None and page['preview_resource_id'] not in resources:fail('preview closure')
  for region in value['regions']:
   if region['page_id'] not in page_ids or not set(region['object_ids'])<=object_ids:fail('region closure')
  for o in value['objects']:
   if not set(o['children'])<=object_ids or not set(o['region_ids'])<=region_ids:fail('object closure')
   if o['type']=='interactive_visualization':validate('VisualizationContent',o['content'])
   if o['type']=='image' and o['content']['resource_id'] not in resources:fail('image resource missing')
  if value['completion']=='complete' and (value['search_coverage']['state']!='complete' or (value['parse_profile']=='deep' and any(o['status']!='complete' for o in value['objects']))):fail('incomplete package cannot claim complete')
 if kind=='SearchPage':
  if value['coverage']['state']!='complete' and value['count_status']=='exact':fail('partial index count is provisional')
 if kind=='SearchScope' and value['version_mode']=='historical' and not value['revision_ids']:fail('historical scope needs revisions')
 if kind=='CatchupResult':
  ids=[n['node_id'] for n in value['path_nodes']];blocks={b['block_id'] for b in value['body']}
  if len(ids)!=len(set(ids)):fail('duplicate path node')
  for node in value['path_nodes']:
   if not set(node['body_block_ids'])<=blocks:fail('path body closure')
   if node['evidence_role']=='teacher_path' and (not node['source_refs'] or any(r['selector']['kind']=='material_region' for r in node['source_refs'])):fail('teacher path lacks speech evidence')
  if any(e['from_node_id'] not in ids or e['to_node_id'] not in ids for e in value['path_edges']):fail('path edge closure')
  if not value['prerequisites'] and value['prerequisites_reason'] is None:fail('empty prerequisites need reason')
  if value['completion_status']=='complete' and (value['prerequisites_reason']=='insufficient_evidence' or not any(n['role']=='current' for n in value['path_nodes'])):fail('incomplete understanding path')
 return value

def negotiate(capabilities):
 validate('Capabilities',capabilities)
 if 2 not in capabilities['supported_contract_majors'] or 2 not in capabilities['supported_material_majors']:fail('unsupported major','UNSUPPORTED_VERSION')
 unsupported=[]
 if 1 not in capabilities['visualization_protocol_majors']:unsupported.append('visualization')
 for k,v in REGISTRY['templates'].items():
  if v['version'] not in capabilities['templates'].get(k,[]):unsupported.append(k)
 if 1 not in capabilities['search_projection_versions']:unsupported.append('search')
 if 'focaus_context_v2' not in capabilities['focaus_result_semantics']:unsupported.append('focaus')
 return {'contract_version':'2.0.0','material_version':'2.0.0','visualization_version':'1.0.0','write_allowed':not unsupported,'unsupported':unsupported}

def adapt_legacy_material(original,new_parse_version_id):
 import copy
 if original.get('format')!='focaus.material' or not str(original.get('format_version','')).startswith('1.'):fail('explicit legacy adapter only accepts 1.x','UNSUPPORTED_VERSION')
 out=copy.deepcopy(original);out['format_version']='2.0.0';out['parent_parse_version_id']=original['parse_version_id'];out['parse_version_id']=new_parse_version_id
 out['search_units']=[];unprocessed=[];kept=[]
 for o in original.get('objects',[]):
  if o.get('type')=='text':
   kept.append(copy.deepcopy(o));text=''.join(r['text'] for r in o['content']['runs'])
   anchor={'owner_entity_id':o['object_id'],'revision_id':o['revision_id'],'selector':{'kind':'text','block_id':o['content']['block_id'],'anchor':{'left_element_id':None,'right_element_id':None,'element_ids':[r['element_id'] for r in o['content']['runs']],'scalar_range':{'start':0,'end':len(text)}}},'field_path':'content.runs','scalar_range':{'start':0,'end':len(text)},'audio_spans':[],'fallback':None}
   out['search_units'].append({'unit_key':o['object_id'],'owner_entity_id':o['object_id'],'revision_id':o['revision_id'],'content_hash':search_text_hash(text),'display_text':text,'field_path':'content.runs','anchor':anchor,'equivalence_group_id':None,'alternate_anchors':[],'source_role':'material_supplement','extraction_version':'1.0.0'})
  else:
   # Do not invent an editable object for unknown semantics. Preserve it in the original package.
   unprocessed.append({'page_id':None,'region_id':None,'reason':'legacy_object_requires_reparse:'+o.get('type','unknown')})
 out['objects']=kept;known={o['object_id'] for o in kept}
 for o in kept:o['children']=[x for x in o['children'] if x in known]
 for r in out.get('regions',[]):r['object_ids']=[x for x in r['object_ids'] if x in known]
 for page in out.get('pages',[]):
  page['reading_order']=[x for x in page['reading_order'] if x in known or x in {r['region_id'] for r in out.get('regions',[])}]
  unprocessed.append({'page_id':page['page_id'],'region_id':None,'reason':'legacy_OCR_coverage_unverified'})
 out['search_coverage']={'requested_pages':[p['page_id'] for p in out.get('pages',[])],'processed_pages':[],'unprocessed':unprocessed,'failed':[],'state':'partial'}
 out['completion']='partial';out['provenance']={**out.get('provenance',{}),'legacy_format_version':original['format_version'],'legacy_original_hash':canonical_hash(original),'adapter_version':'1.0.0'}
 validate('MaterialManifest',out)
 return {'manifest':out,'original':copy.deepcopy(original),'unsupported_count':sum(1 for x in original.get('objects',[]) if x.get('type')!='text')}
def adapt_legacy_catchup(original):return {'result_semantics':'legacy_summary','original':original,'completion_status':'partial','new_acceptance_eligible':False}
