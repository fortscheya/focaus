import copy,json,sqlite3,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'schemas'))
from source import SCHEMA,MODEL_BINDINGS
from validate import validate,ContractError,negotiate,adapt_legacy_material,adapt_legacy_catchup,canonical_hash
from jsonschema import Draft202012Validator,FormatChecker
class Contracts(unittest.TestCase):
 def test_all_fixtures(self):
  for c in json.loads((ROOT/'fixtures/contracts/cases.json').read_text()):
   with self.subTest(c['name']):
    if c['valid']:validate(c['type'],c['value'])
    else:
     with self.assertRaises(ContractError):validate(c['type'],c['value'])
 def test_legacy_keeps_original_and_marks_unprocessed(self):
  old=json.loads((ROOT/'fixtures/contracts/legacy-material.json').read_text());saved=copy.deepcopy(old)
  out=adapt_legacy_material(old,'e94b3ddd-8e7c-4176-b937-2955a2eddf96')
  self.assertEqual(old,saved);self.assertEqual(out['original'],old)
  self.assertEqual(out['manifest']['source_version_id'],old['source_version_id'])
  self.assertNotEqual(out['manifest']['parse_version_id'],old['parse_version_id'])
  self.assertEqual(out['unsupported_count'],1);self.assertEqual(out['manifest']['search_coverage']['state'],'partial')
  self.assertEqual(out['manifest']['search_units'][0]['display_text'],'Straße 学习 e\u0301 😀')
  self.assertFalse(adapt_legacy_catchup({'text':'old'})['new_acceptance_eligible'])
 def test_unknown_major_and_capabilities(self):
  cases=json.loads((ROOT/'fixtures/contracts/cases.json').read_text());cap=next(c['value'] for c in cases if c['name']=='capabilities')
  self.assertTrue(negotiate(cap)['write_allowed'])
  cap['templates']={};self.assertFalse(negotiate(cap)['write_allowed'])
  cap['supported_contract_majors']=[99]
  with self.assertRaises(ContractError):negotiate(cap)
 def test_schema_meta(self):Draft202012Validator.check_schema(SCHEMA)
 def test_hash_vectors(self):
  for c in json.loads((ROOT/'fixtures/contracts/hash-golden.json').read_text()):self.assertEqual(canonical_hash(c['value']),c['sha256'])
class Migration(unittest.TestCase):
 def setUp(self):
  self.db=sqlite3.connect(':memory:');self.db.execute('PRAGMA foreign_keys=ON')
  self.db.executescript((ROOT/'apps/desktop/src-tauri/migrations/0001_initial.sql').read_text())
 def tearDown(self):self.db.close()
 def entity(self,id,kind='test'):self.db.execute('INSERT INTO entities(entity_id,kind) VALUES(?,?)',(id,kind))
 def recording(self,id):
  self.entity(id,'recording');self.db.execute("INSERT INTO recording_sessions(recording_id,state,capture_language_tier,timeline_origin) VALUES(?,'recording',3,'2026-10-18T00:00:00Z')",(id,))
 def test_version_and_foreign_keys(self):
  self.assertEqual(self.db.execute('PRAGMA user_version').fetchone()[0],1)
  self.assertEqual(self.db.execute('PRAGMA foreign_key_check').fetchall(),[])
  with self.assertRaises(sqlite3.IntegrityError):self.db.execute("INSERT INTO courses VALUES('absent','x')")
 def test_only_one_active_session(self):
  self.recording('r1')
  with self.assertRaises(sqlite3.IntegrityError):self.recording('r2')
 def test_stable_raw_and_purge_requires_tombstone(self):
  self.recording('r');self.entity('u','raw')
  self.db.execute("INSERT INTO raw_utterances VALUES('u','r',0,'final','original','rev',NULL,'{}','[]')")
  with self.assertRaises(sqlite3.IntegrityError):self.db.execute("UPDATE raw_utterances SET text='changed' WHERE utterance_id='u'")
  with self.assertRaises(sqlite3.IntegrityError):self.db.execute("UPDATE raw_utterances SET text=NULL WHERE utterance_id='u'")
  self.db.execute("UPDATE entities SET lifecycle='deleted' WHERE entity_id='u'")
  self.db.execute("UPDATE raw_utterances SET text=NULL WHERE utterance_id='u'")
  self.assertEqual(self.db.execute('SELECT visibility_epoch FROM visibility_barrier').fetchone()[0],1)
 def test_outbox_rollback(self):
  self.db.execute('BEGIN')
  self.entity('e')
  self.db.execute("INSERT INTO outbox(event_id,scope_json,event_type,entity_id,entity_seq,revision_id,committed_at,payload_json) VALUES('event','{}','content.committed','e',1,'r','time','{}')")
  self.db.rollback()
  self.assertEqual(self.db.execute('SELECT count(*) FROM entities').fetchone()[0],0)
  self.assertEqual(self.db.execute('SELECT count(*) FROM outbox').fetchone()[0],0)
 def test_schema_reapply_rolls_back(self):
  with self.assertRaises(sqlite3.OperationalError):self.db.executescript((ROOT/'apps/desktop/src-tauri/migrations/0001_initial.sql').read_text())
  self.db.rollback();self.assertEqual(self.db.execute('SELECT count(*) FROM schema_migrations').fetchone()[0],1)
 def test_all_model_binding_cross_products(self):
  self.db.execute("INSERT INTO input_snapshots VALUES('s',NULL,'2.0.0','{}',?,'now')",('a'*64,))
  for i,(model,expected) in enumerate(MODEL_BINDINGS.items()):
   for j,slot in enumerate(MODEL_BINDINGS.values()):
    values=(f'{i}-{j}','progress','t','s','queued',1,1,None,f'{i}-{j}','{}',None,model,slot,'cn-beijing','now','now')
    if expected==slot:self.db.execute('INSERT INTO tasks VALUES('+','.join('?' for _ in values)+')',values)
    else:
     with self.assertRaises(sqlite3.IntegrityError):self.db.execute('INSERT INTO tasks VALUES('+','.join('?' for _ in values)+')',values)
if __name__=='__main__':unittest.main()
