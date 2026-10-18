"""Reproducible synthetic fixtures. No real classroom content, audio or credentials."""
import copy,json,sys,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'schemas'))
from validate import canonical_hash
from source import REGISTRY
U=lambda s:str(uuid.uuid5(uuid.NAMESPACE_URL,'focaus-fixture:'+s))
H='a'*64;T='2026-10-18T02:00:00Z'
selector={'kind':'text','block_id':U('block'),'anchor':{'left_element_id':None,'right_element_id':None,'element_ids':[U('element')],'scalar_range':{'start':0,'end':8}}}
source={'entity_id':U('raw'),'revision_id':U('raw-rev'),'content_hash':H,'selector':selector}
snapshot={'snapshot_id':U('snapshot'),'parent_snapshot_id':None,'contract_version':'2.0.0','purpose':'catchup','target_id':U('note'),'lesson_id':U('lesson'),'recording_id':U('recording'),'assignment_epoch':0,'sources':[source],'attachment_set_revision':U('attachments'),'fact_set_revision':None,'target_base_revision':None,'edit_revision':None,'language_tier':3,'evidence_cutoff':[{'recording_id':U('recording'),'end_ms':300000,'status':'known'}],'available_at':T,'executor_id':'qwen-adapter','executor_version':'1.0.0','model_id':'qwen3.8-flash','region':'cn-beijing','prompt_version':'1.0.0','parameters_hash':H}
recording={'recording_id':U('recording'),'lesson_id':None,'assignment_epoch':0,'state':'recording','capture_language_tier':3,'timeline_origin':T,'durable_watermark_ms':1000,'revision_id':U('recording-rev'),'entity_seq':1}
gap={'audio_span_id':U('gap'),'recording_id':U('recording'),'asset_id':None,'track_id':None,'frame_range':None,'timeline_range':{'start_ms':1000,'end_ms':2000},'mapping_revision_id':None,'coverage_status':'capture_gap','asr_coverage':'unsubmitted'}
conv={'operator_dimension':2,'scene_dimension':3,'operator_semantics':'cross_correlation','axis_order':'CHW','input':[[[1,2,3],[4,5,6],[7,8,9]]],'kernel':[[[1,0],[0,-1]]],'stride':[1,1],'padding':[0,0,0,0],'dilation':[1,1],'bias':None}
curve={'formula_id':'affine_v1','slope':2,'intercept':1,'x_min':-2,'x_max':2,'samples':5}
vis={'object_id':U('viz'),'revision_id':U('viz-rev'),'parent_revision_ids':[],'protocol':'focaus.visualization','protocol_version':'1.0.0','template_id':'conv2d_spatial','template_version':'1.0.0','registry_revision':'1.0.0','concept_fact_ids':[U('fact')],'definition_ids':[],'symbol_binding_ids':[],'scope_id':U('scope'),'semantic_key':'fixture-convolution-purpose','purpose':'explain sliding window','placement_anchor':selector,'parameters':conv,'parameter_schema_version':'1.0.0','value_bindings':[{'binding_id':U('binding'),'parameter_path':'input','value_origin':'demonstration','value':conv['input'],'unit':None,'source_refs':[],'dependency_ids':[],'formula_id':None,'scope_id':U('scope'),'unknown_reason':None,'user_override':False}],'computation':{'algorithm_id':'conv2d_v1','algorithm_version':'1.0.0','operator_semantics':'cross_correlation','formula_id':'conv2d_v1','input_shape':[1,3,3],'output_shape':[2,2],'precision_policy_id':'f64_abs_1e-9_rel_1e-9','dependency_binding_ids':[]},'resources':[],'source_refs':[source],'snapshot_id':U('snapshot'),'fact_set_revision':U('facts'),'interaction_state':{'selected_input_ids':[],'selected_output_ids':[],'step_index':0,'camera':None,'user_parameter_overrides':[]},'presentation_by_tier':[{'tier':i,'label':'演示二维算子','description':'自制数据，非课堂事实','sync_status':'ready'} for i in range(1,6)],'generation_status':'ready','validation':{'checks':{x:'pass' for x in ['schema','source','semantic','math']},'validator_version':'1.0.0','parameters_hash':canonical_hash(conv),'validated_revision':U('viz-rev'),'diagnostics':[]},'fallback':{'preview_resource_id':None,'description':'相同参数的二维矩阵和乘加','table':[['-4','-4'],['-4','-4']]}}
obj={'object_id':U('text-object'),'revision_id':U('text-rev'),'source_refs':[],'region_ids':[U('region')],'layout':{'bounds':[0,0,1,1],'z_order':0,'group_id':None,'constraints':[]},'extraction':'native','editability':'native','status':'complete','children':[],'type':'text','content':{'block_id':U('block'),'runs':[{'element_id':U('element'),'text':'Straße 学习 e\u0301 😀'}],'level':0}}
page={'page_id':U('page'),'revision_id':U('page-rev'),'source_page_index':0,'display_label':'1','width':720,'height':540,'unit':'pt','rotation':0,'source_transform':[1,0,0,1,0,0],'preview_resource_id':None,'reading_order':[obj['object_id']],'region_ids':[U('region')],'status':'basic_ready','errors':[]}
region={'region_id':U('region'),'page_id':U('page'),'bbox':[0,0,1,1],'object_ids':[obj['object_id']],'semantic_role':'body','parent_region_id':None}
manifest={'format':'focaus.material','format_version':'2.0.0','material_id':U('material'),'source_version_id':U('source-version'),'parse_version_id':U('parse-version'),'parent_parse_version_id':None,'source_hash':H,'engine_build':'fixture/1.0.0','parse_profile':'light','requested_pages':[0],'pages':[page],'regions':[region],'objects':[obj],'resources':[],'corrections_applied':[],'unresolved_conflicts':[],'provenance':{'synthetic':True},'completion':'partial','search_units':[],'search_coverage':{'requested_pages':[U('page')],'processed_pages':[],'unprocessed':[{'page_id':U('page'),'region_id':None,'reason':'fixture_not_extracted'}],'failed':[],'state':'partial'}}
legacy=copy.deepcopy(manifest);legacy['format_version']='1.0.0';legacy.pop('search_units');legacy.pop('search_coverage')
legacy['objects'].append({'object_id':U('legacy-unknown'),'type':'legacy_unknown_widget','payload':{'kept':'opaque'}})
focus={'result_semantics':'focaus_context_v2','result_id':U('focaus'),'request_generation':2,'snapshot_id':U('snapshot'),'body_revision_id':U('body-rev'),'requested_window':{'recording_id':U('recording'),'start_ms':0,'end_ms':300000,'start_source':None},'effective_window':{'recording_id':U('recording'),'start_ms':0,'end_ms':300000,'start_source':None},'estimate_basis':'unknown; five-minute default','current_focus':{'topic':'合成卷积例子','problem':'如何计算当前输出格','activity':'example','step':'逐项乘加','source_refs':[source],'status':'supported'},'path_nodes':[{'node_id':'current','role':'current','fact_ids':[U('fact')],'definition_ids':[],'binding_ids':[],'body_block_ids':[U('body-block')],'evidence_role':'teacher_path','source_refs':[source]},{'node_id':'bridge','role':'bridge','fact_ids':[],'definition_ids':[],'binding_ids':[],'body_block_ids':[U('body-block')],'evidence_role':'system_explanation','source_refs':[source]}],'path_edges':[{'from_node_id':'bridge','to_node_id':'current','relation':'dependency','source_refs':[source],'certainty':'supported'}],'prerequisites':[{'definition_id':U('definition'),'binding_id':None,'fact_id':None,'reason':'理解求和需要先前定义','source_refs':[source],'scope_id':U('scope'),'outside_window':True}],'prerequisites_reason':None,'body':[{'block_id':U('body-block'),'text':'合成示例：从当前滑窗的各项乘积相加，接回当前输出格。'}],'presentation_tier':3,'body_evidence_cutoff':[{'recording_id':U('recording'),'end_ms':300000,'status':'known'}],'body_generated_at':T,'progress_at_trigger':None,'progress_unavailable_reason':'fixture no progress','progress_binding':{'lesson_id':U('lesson'),'recording_id':U('recording'),'assignment_epoch':0,'no_pointer_reason':None},'coverage':{'audio_gaps':[],'asr_gaps':[],'material_supplement_used':False,'range_status':'complete'},'limitations':[],'completion_status':'complete','triggered_at':T,'completed_at':T,'displayed_complete_elapsed_ms':14000}
coverage={'scope_snapshot_id':U('search-snapshot'),'required_units':1,'indexed_units':0,'required_pages':[U('page')],'indexed_pages':[],'unprocessed':[{'page_id':U('page'),'region_id':None,'reason':'not_ocr'}],'failed':[],'stale':[],'state':'partial','source_extraction_revision':U('parse-version')}
search_page={'search_id':U('search'),'query_seq':3,'scope_snapshot_id':U('search-snapshot'),'hits':[],'total_count':0,'count_status':'provisional','coverage':coverage,'index_generation':1,'visibility_epoch':0,'next_cursor':None}
patch={'patch_id':U('patch'),'target_id':U('note'),'base_revision':U('note-rev'),'operation':'text_replace','selector':selector,'before_hash':H,'after_value':'用户修改','author':'user','intent':'protect edited text','undo_group_id':U('undo'),'created_at':T}
cap={'supported_contract_majors':[2],'supported_material_majors':[2],'visualization_protocol_majors':[1],'templates':{k:[v['version']] for k,v in REGISTRY['templates'].items()},'search_projection_versions':[1],'focaus_result_semantics':['focaus_context_v2']}
cases=[]
def case(name,kind,value,valid=True,level='schema'):cases.append({'name':name,'type':kind,'value':copy.deepcopy(value),'valid':valid,'level':level})
def bad(name,kind,value,change,level='schema'):
 v=copy.deepcopy(value);change(v);case(name,kind,v,False,level)
for name,kind,value in [('source-ref','SourceRef',source),('snapshot','InputSnapshot',snapshot),('unassigned-recording','RecordingSession',recording),('gap','AudioSpan',gap),('conv2d','Conv2dParameters',conv),('curve','CurveParameters',curve),('visualization','VisualizationContent',vis),('material-partial','MaterialManifest',manifest),('focaus-bridge','CatchupResult',focus),('search-unprocessed','SearchPage',search_page),('user-patch','UserPatch',patch),('capabilities','Capabilities',cap)]:case(name,kind,value)
bad('invalid-source-uuid','SourceRef',source,lambda v:v.update(entity_id='page-1'))
bad('missing-source-revision','SourceRef',source,lambda v:v.pop('revision_id'))
bad('unknown-contract-major','InputSnapshot',snapshot,lambda v:v.update(contract_version='3.0.0'))
bad('missing-local-executor','InputSnapshot',snapshot,lambda v:v.pop('executor_version'))
bad('zero-stride','Conv2dParameters',conv,lambda v:v.update(stride=[0,1]))
bad('negative-padding','Conv2dParameters',conv,lambda v:v.update(padding=[-1,0,0,0]))
bad('ragged-input','Conv2dParameters',conv,lambda v:v['input'][0][0].pop(), 'semantic')
bad('unsupported-dilation','Conv2dParameters',conv,lambda v:v.update(dilation=[2,2]),'semantic')
bad('missing-search-coverage','MaterialManifest',manifest,lambda v:v.pop('search_coverage'))
bad('missing-object-closure','MaterialManifest',manifest,lambda v:v['regions'][0]['object_ids'].append(U('absent')),'semantic')
bad('partial-cannot-complete','MaterialManifest',manifest,lambda v:v.update(completion='complete'),'semantic')
bad('unknown-template-ready','VisualizationContent',vis,lambda v:v.update(template_id='remote_javascript'))
bad('code-injection','VisualizationContent',vis,lambda v:v.update(code='eval(1)'))
bad('math-pending-not-ready','VisualizationContent',vis,lambda v:v['validation']['checks'].update(math='pending'))
bad('wrong-parameter-hash','VisualizationContent',vis,lambda v:v['validation'].update(parameters_hash='0'*64),'semantic')
bad('empty-focaus-not-complete','CatchupResult',focus,lambda v:v.update(body=[]))
bad('unknown-focus-not-complete','CatchupResult',focus,lambda v:v['current_focus'].update(status='unknown'))
bad('invented-teacher-path','CatchupResult',focus,lambda v:v['path_nodes'][0].update(source_refs=[]),'semantic')
bad('broken-understanding-path','CatchupResult',focus,lambda v:v['path_edges'][0].update(to_node_id='absent'),'semantic')
bad('partial-search-is-not-zero-exact','SearchPage',search_page,lambda v:v.update(count_status='exact'),'semantic')
for model,slot in [('qwen-audio-3.1-asr-flash-streaming','bailian_beijing_asr_streaming'),('decision-model-preview','bailian_beijing_progress'),('qwen3.8-flash','bailian_beijing_multimodal'),('qwen-audio-3.1-asr-flash-filetrans','bailian_beijing_asr_filetrans')]:
 binding={'provider':'bailian','region':'cn-beijing','model_id':model,'credential_ref':slot};case('binding-'+slot,'CredentialBinding',binding)
 bad('reject-cross-slot-'+slot,'CredentialBinding',binding,lambda v:v.update(credential_ref='bailian_beijing_progress' if slot!='bailian_beijing_progress' else 'bailian_beijing_multimodal'))
cmd={'contract_version':'2.0.0','command_id':U('command'),'name':'recording.start','actor':'user','window_id':U('window'),'payload':{'lesson_id':None,'device_id':None},'idempotency_key':'fixture-start','expected_revisions':{}}
case('start-command','Command',cmd);bad('mutation-needs-idempotency','Command',cmd,lambda v:v.pop('idempotency_key'))
bad('payload-rejects-unknown-field','Command',cmd,lambda v:v['payload'].update(api_key='not-a-key'))
layout={'columns':[[U(str(i)),U(str(i+1))] for i in [0,2,4]],'column_ratios':[1,1,1],'row_ratios':[[1,1],[1,1],[1,1]],'maximized_view_id':None,'revision_id':U('layout')}
case('six-cells','Layout',layout);bad('seventh-grid-cell','Layout',layout,lambda v:v['columns'].append([U('7')]))
# Fixtures describe expected runtime behavior; these are not claims that N/F/A/E already implement it.
scenarios=[
 {'id':'C02-overlap','kind':'recording','given':'stable streaming [0,1000), filetrans [500,1500)','expect':'preserve stable hash; admit [1000,1500) only; ambiguous word boundary review'},
 {'id':'C02-partial-final','kind':'recording','given':'same remote binding partial then final then duplicate final','expect':'one stable utterance; later partial cannot revert final'},
 {'id':'C03-late-generation','kind':'proposal','given':'generation epoch 0; recording transferred to epoch 1','expect':'STALE_INPUT; no publication'},
 {'id':'C03-user-edit','kind':'proposal','given':'AI touches active IME and user-patched cell','expect':'review/defer intersections; unaffected selectors may commit'},
 {'id':'C03-delete-intent','kind':'proposal','given':'same semantic key rediscovered after user deletion','expect':'no automatic resurrection across language/template patch'},
 {'id':'C03-five-tiers','kind':'language','given':'Fact set changes while old tier generation pending','expect':'old result stale; requested tier differs from actual until ready'},
 {'id':'C06-transfer-rollback','kind':'window','given':'view.move prepared; destination crashes before ready','expect':'old window and draft remain; single edit lease'},
 {'id':'C07-demonstration','kind':'visualization','given':'user adjusts classroom parameter','expect':'demonstration/user_override; shared Fact untouched'},
 {'id':'C08-equivalence','kind':'search','given':'same source text represented by original page and native object','expect':'one hit with dual anchors; identical string elsewhere stays distinct'},
 {'id':'C08-delete-race','kind':'search','given':'delete after query before page/locate','expect':'SOURCE_DELETED or SEARCH_INVALIDATED, no old snippet'},
 {'id':'C09-late-generation','kind':'focaus','given':'generation 1 returns after generation 2','expect':'no replacement; late history explicit only'},
 {'id':'C09-double-cutoff','kind':'focaus','given':'body cutoff 300000; pointer advances to 330000','expect':'body hash unchanged; two evidence times visible'},
]
def save(name,value): (ROOT/'fixtures/contracts'/name).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
save('cases.json',cases);save('legacy-material.json',legacy);save('runtime-scenarios.json',scenarios)
save('math-golden.json',[
 {'name':'cross-correlation','type':'Conv2dParameters','parameters':conv,'shape':[2,2],'values':[-4,-4,-4,-4]},
 {'name':'convolution-flips-kernel','type':'Conv2dParameters','parameters':{**conv,'operator_semantics':'convolution'},'shape':[2,2],'values':[4,4,4,4]},
 {'name':'stride-two','type':'Conv2dParameters','parameters':{**conv,'stride':[2,2]},'shape':[1,1],'values':[-4]},
 {'name':'padding-all','type':'Conv2dParameters','parameters':{**conv,'padding':[1,1,1,1]},'shape':[4,4],'values':[-1,-2,-3,0,-4,-4,-4,3,-7,-4,-4,6,0,7,8,9]},
 {'name':'affine','type':'CurveParameters','parameters':curve,'shape':[5,2],'values':[-2,-3,-1,-1,0,1,1,3,2,5]}])
save('search-golden.json',{'profile':'nfc_casefold_ws_v1','unicode_version':'15.0.0','cases':[
 {'text':'Straße','normalized':'strasse','query':'SS','original_range':[4,5]},
 {'text':'Cafe\u0301 😀','normalized':'café 😀','query':'é','original_range':[3,5]},
 {'text':'学\t \n习','normalized':'学 习','query':'学 习','original_range':[0,5]},
 {'text':'😀学习😀','normalized':'😀学习😀','query':'学习','original_range':[1,3]}]})
save('hash-golden.json',[{'value':v,'sha256':canonical_hash(v)} for v in [{'b':2,'a':'学习'}, {'z':-0.0,'numbers':[1.0,0.000001,1e21]},conv]])
print('synthetic contract fixtures:',len(cases),'cases; runtime scenarios:',len(scenarios))
