//! Authenticated WebView gateway and gap-free snapshot/outbox delivery.
use crate::{capture::RecordingCoordinator,storage::{self,sql_error,Store,Result},tasks::TaskManager,visualizations::Visualizations};
use focaus_contracts::*;
use rusqlite::OptionalExtension;
use serde::de::DeserializeOwned;
use serde_json::{json,Value};
use std::{collections::BTreeMap,path::Path,sync::Mutex};

#[derive(Clone)]
struct WindowBinding {id:String,scopes:Vec<Scope>}
struct Subscription {window_id:String,scopes:Vec<Scope>,acked:u64,delivered:u64,snapshot_pending:bool}
pub struct NativeHost {
    pub store:Store,pub recording:RecordingCoordinator,pub tasks:TaskManager,pub visualizations:Visualizations,
    pub run_id:String,
    windows:Mutex<BTreeMap<String,WindowBinding>>,subscriptions:Mutex<BTreeMap<String,Subscription>>,
    dispatch_gate:Mutex<()>,
}
fn parse<T:DeserializeOwned>(v:&Value)->Result<T>{serde_json::from_value(v.clone()).map_err(|_|error(ErrorCode::InvalidArgument,"Invalid command payload"))}
fn scope_allows(allowed:&[Scope],requested:&Scope)->bool {
    allowed.iter().any(|s|matches!(s,Scope::V0(_))||s==requested)
}
fn scope_match(scopes:&[Scope],event:&Scope)->bool {scopes.iter().any(|s|matches!(s,Scope::V0(_))||s==event)}
impl NativeHost {
    /// Send+Sync composition object; CPAL streams stay on their owning thread.
    /// Opening a library restores data/tasks but never opens a microphone.
    pub fn open(root:impl AsRef<Path>,migration:&str)->Result<Self> {
        let store=Store::open(root,migration)?;let _=crate::audio::recover(&store)?;
        let tasks=TaskManager::new(store.clone());let _=tasks.recover()?;
        Ok(Self {recording:RecordingCoordinator::new(store.clone()),visualizations:Visualizations::new(store.clone()),tasks,store,run_id:storage::id(),windows:Mutex::new(BTreeMap::new()),subscriptions:Mutex::new(BTreeMap::new()),dispatch_gate:Mutex::new(())})
    }
    pub fn bind_window(&self,webview_label:&str,window_id:&str,allowed_scopes:Vec<Scope>)->Result<()> {
        if webview_label.is_empty()||window_id.is_empty(){return Err(error(ErrorCode::InvalidArgument,"Window binding required"));}
        let mut bindings=self.windows.lock().map_err(|_|error(ErrorCode::InvalidResult,"Window registry unavailable"))?;
        if let Some(old)=bindings.get(webview_label){if old.id!=window_id{return Err(error(ErrorCode::Conflict,"Window already bound"));}return Ok(());}
        bindings.insert(webview_label.into(),WindowBinding {id:window_id.into(),scopes:allowed_scopes});Ok(())
    }
    /// Closing a window removes only its subscriptions. Capture/tasks stay alive.
    pub fn unbind_window(&self,label:&str)->Result<()> {
        let old=self.windows.lock().map_err(|_|error(ErrorCode::InvalidResult,"Window registry unavailable"))?.remove(label);
        if let Some(old)=old{self.subscriptions.lock().map_err(|_|error(ErrorCode::InvalidResult,"Subscriptions unavailable"))?.retain(|_,s|s.window_id!=old.id);}Ok(())
    }
    pub fn dispatch_webview(&self,label:&str,command:Command)->Result<CommandResult> {
        let _serial=self.dispatch_gate.lock().map_err(|_|error(ErrorCode::InvalidResult,"Command dispatcher unavailable"))?;
        let window=self.windows.lock().map_err(|_|error(ErrorCode::InvalidResult,"Window registry unavailable"))?.get(label).cloned().ok_or_else(||error(ErrorCode::PermissionDenied,"Unknown calling WebView"))?;
        if command.actor!=CommandActor::User||command.window_id.as_deref()!=Some(window.id.as_str()){return Err(error(ErrorCode::PermissionDenied,"Actor or window binding mismatch"));}
        let name=storage::value(&command.name)?.as_str().unwrap_or("").to_owned();
        if schema()["x-host-only-commands"].as_array().is_some_and(|xs|xs.iter().any(|x|x==&json!(name))){return Err(error(ErrorCode::PermissionDenied,"Host-only command"));}
        validate("Command",&storage::value(&command)?)?;
        let catalogue=command_catalogue();let entry=catalogue.get(&name).ok_or_else(||error(ErrorCode::InvalidArgument,"Unknown command"))?;
        validate(entry["input"].as_str().unwrap_or(""),&command.payload)?;
        // Narrow bindings are enforced before any content lookup. The initial C
        // bootstrap is a single-user local global scope; it grants no host APIs.
        if !window.scopes.iter().any(|s|matches!(s,Scope::V0(_))) {return Err(error(ErrorCode::PermissionDenied,"Scoped window capability not yet granted for this command"));}
        if entry["mutates"]==true&&command.expected_revisions.is_none(){return Err(error(ErrorCode::InvalidArgument,"Expected revisions required"));}
        let result=match name.as_str() {
            "state.open"=>self.state_open(&window,parse(&command.payload)?)?,
            "state.ack"=>{self.state_ack(&window.id,parse(&command.payload)?)?;json!({})},
            "state.replay"=>self.state_replay(&window.id,parse(&command.payload)?)?,
            "state.close"=>{let input:StateCloseInput=parse(&command.payload)?;let mut subscriptions=self.subscriptions.lock().map_err(|_|error(ErrorCode::InvalidResult,"Subscriptions unavailable"))?;if subscriptions.get(&input.subscription_id).is_some_and(|s|s.window_id!=window.id){return Err(error(ErrorCode::PermissionDenied,"Subscription belongs to another window"));}subscriptions.remove(&input.subscription_id);json!({})},
            "content.get"=>self.store.read(|c|crate::library::get_content(c,&parse(&command.payload)?))?,
            "visualization.evaluate"=>storage::value(&self.visualizations.evaluate(parse(&command.payload)?)?)?,
            "course.create"=>return self.store.command(&window.id,&command,|t|crate::library::create_course(t,&parse(&command.payload)?,Some(&command.command_id))),
            "lesson.create"=>return self.store.command(&window.id,&command,|t|crate::library::create_lesson(t,&parse(&command.payload)?,Some(&command.command_id))),
            "recording.start"=>{
                let input:RecordingStartInput=parse(&command.payload)?;
                let receipt=self.store.command(&window.id,&command,|t|RecordingCoordinator::reserve(t,&input,Some(&command.command_id)))?;
                let recording=receipt.result.as_ref().and_then(|v|v["recording_id"].as_str()).ok_or_else(||error(ErrorCode::InvalidResult,"Recording reservation unavailable"))?;
                self.recording.activate(recording,input.device_id.as_deref())?;return Ok(receipt);
            },
            "recording.transfer"=>return self.store.command(&window.id,&command,|t|crate::library::transfer(t,&parse(&command.payload)?,Some(&command.command_id))),
            "edit.begin"=>return self.store.command(&window.id,&command,|t|{let input:EditBeginInput=parse(&command.payload)?;crate::library::begin_edit(t,&input.lease)}),
            "edit.end"=>return self.store.command(&window.id,&command,|t|crate::library::end_edit(t,&parse(&command.payload)?)),
            "audio.play"=>{self.recording.start_playback()?;return Err(error(ErrorCode::UnsupportedCapability,"Playback output adapter is not connected"));},
            "audio.stop"=>{self.recording.stop_playback();json!({})},
            "recording.pause"|"recording.resume"|"recording.stop"=>{
                if let Some(receipt)=self.store.preflight(&window.id,&command)? {return Ok(receipt);}
                let rid=command.payload["recording_id"].as_str().ok_or_else(||error(ErrorCode::InvalidArgument,"Recording required"))?;
                if command.expected_revisions.as_ref().is_none_or(|r|!r.contains_key(rid)){return Err(error(ErrorCode::InvalidArgument,"Recording revision required"));}
                let current=self.store.head(rid)?;
                if current["state"]!=command.payload["expected_state"]{return Err(error(ErrorCode::Conflict,"Recording state changed"));}
                let state=current["state"].as_str().unwrap_or("");
                let next=if name=="recording.resume" {if !["paused","interrupted"].contains(&state){return Err(error(ErrorCode::Conflict,"Recording cannot resume from this state"));}"starting"}else if name=="recording.pause" {if !["starting","recording"].contains(&state){return Err(error(ErrorCode::Conflict,"Recording cannot pause from this state"));}"paused"}else{if !["starting","recording","paused","interrupted"].contains(&state){return Err(error(ErrorCode::Conflict,"Recording cannot stop from this state"));}"stopped"};
                if name!="recording.resume" {self.recording.quiesce(rid,name=="recording.stop")?;}
                let receipt=self.store.command(&window.id,&command,|t|crate::capture::state_transaction(t,rid,next,Some(&command.command_id)));
                let receipt=match receipt {Ok(receipt)=>receipt,Err(e)=>{if name!="recording.resume"{let _=crate::capture::set_state(&self.store,rid,"interrupted");}return Err(e);}};
                if name=="recording.resume"{self.recording.reactivate(rid)?;}
                return Ok(receipt);
            },
            "app.exit"=>{self.exit()?;json!({"sealed":true})},
            _=>return Err(error(ErrorCode::UnsupportedCapability,"Native command is not connected"))
        };
        Ok(CommandResult {contract_version:CONTRACT_VERSION.into(),command_id:command.command_id,status:CommandResultStatus::Ok,result:Some(result),committed_revisions:Some(BTreeMap::new()),event_seq:Some(self.store.read(storage::sequence)?),task_id:None,current_state:None,error:None})
    }
    fn state_open(&self,window:&WindowBinding,input:StateOpenInput)->Result<Value> {
        let negotiated=negotiate(&input.capabilities)?;
        for scope in &input.scopes{if !scope_allows(&window.scopes,scope){return Err(error(ErrorCode::PermissionDenied,"Scope not granted"));}}
        let mut subscriptions=self.subscriptions.lock().map_err(|_|error(ErrorCode::InvalidResult,"Subscriptions unavailable"))?;
        let subscription_id=storage::id();
        let snapshot=self.store.consistent_snapshot(|c,seq| {
            let mut records:BTreeMap<String,BTreeMap<String,Value>>=BTreeMap::new();
            let mut q=c.prepare("SELECT entity_id,kind,revision_id,entity_seq,lifecycle FROM entities ORDER BY entity_id").map_err(sql_error)?;
            for row in q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,u64>(3)?,r.get::<_,String>(4)?))).map_err(sql_error)? {
                let (entity,kind,revision,entity_seq,lifecycle)=row.map_err(sql_error)?;
                if !input.scopes.iter().any(|s|matches!(s,Scope::V0(_))||storage::value(s).ok().is_some_and(|v|v["entity_id"]==entity||v["course_id"]==entity||v["lesson_id"]==entity)){continue;}
                let collection=match kind.as_str(){"course"=>"courses","lesson"=>"lessons",_=>"content_heads"};
                let body=if lifecycle!="deleted"&&matches!(kind.as_str(),"course"|"lesson"){crate::storage::head(c,&entity)?}else{json!({"entity_id":entity,"kind":kind,"revision_id":revision,"entity_seq":entity_seq,"lifecycle":lifecycle})};
                records.entry(collection.into()).or_default().insert(entity,body);
            }
            for collection in ["courses","lessons","content_heads","task_summaries","progress","view_states","visualizations","search_indexes","catchup_heads"]{records.entry(collection.into()).or_default();}
            let recording:Option<String>=c.query_row("SELECT recording_id FROM recording_sessions ORDER BY state IN ('starting','recording','paused','stopping') DESC,rowid DESC LIMIT 1",[],|r|r.get(0)).optional().map_err(sql_error)?;
            let mut global=BTreeMap::new();global.insert("recording".into(),recording.map(|id|crate::storage::head(c,&id)).transpose()?.unwrap_or(Value::Null));global.insert("visibility_epoch".into(),json!(crate::storage::epoch(c)?));global.insert("windows".into(),json!([]));
            Ok(AppSnapshot {contract_version:CONTRACT_VERSION.into(),host_run_id:self.run_id.clone(),snapshot_seq:seq,scopes:input.scopes.clone(),global,records,continuations:Vec::new()})
        })?;
        subscriptions.insert(subscription_id.clone(),Subscription {window_id:window.id.clone(),scopes:input.scopes,acked:snapshot.snapshot_seq,delivered:snapshot.snapshot_seq,snapshot_pending:true});
        storage::value(&StateOpenResult {subscription_id,snapshot_seq:snapshot.snapshot_seq,snapshot,negotiated})
    }
    fn state_ack(&self,window:&str,input:StateAckInput)->Result<()> {
        let mut subscriptions=self.subscriptions.lock().map_err(|_|error(ErrorCode::InvalidResult,"Subscriptions unavailable"))?;
        let sub=subscriptions.get_mut(&input.subscription_id).ok_or_else(||error(ErrorCode::NotFound,"Subscription unavailable"))?;
        if sub.window_id!=window{return Err(error(ErrorCode::PermissionDenied,"Subscription belongs to another window"));}
        if input.upto_seq<sub.acked||input.upto_seq>sub.delivered{return Err(error(ErrorCode::Conflict,"Invalid event acknowledgment watermark"));}
        sub.acked=input.upto_seq;sub.snapshot_pending=false;Ok(())
    }
    fn state_replay(&self,window:&str,input:StateReplayInput)->Result<Value> {
        let mut subscriptions=self.subscriptions.lock().map_err(|_|error(ErrorCode::InvalidResult,"Subscriptions unavailable"))?;
        let sub=subscriptions.get_mut(&input.subscription_id).ok_or_else(||error(ErrorCode::NotFound,"Subscription unavailable"))?;
        if sub.window_id!=window{return Err(error(ErrorCode::PermissionDenied,"Subscription belongs to another window"));}
        if input.after_seq!=sub.acked{return Err(error(ErrorCode::Conflict,"Replay cursor does not match acknowledged watermark"));}
        let batch=self.batch(&input.subscription_id,sub)?;storage::value(&batch)
    }
    fn batch(&self,id:&str,sub:&mut Subscription)->Result<EventBatch> {
        let (through,events)=self.store.read(|c| {
            let oldest:Option<u64>=c.query_row("SELECT MIN(global_seq) FROM outbox",[],|r|r.get(0)).map_err(sql_error)?;
            if oldest.is_some_and(|old|sub.acked+1<old){return Err(error(ErrorCode::CursorExpired,"Event cursor expired; reopen snapshot"));}
            crate::storage::events(c,sub.acked,256)
        })?;
        sub.delivered=through;
        Ok(EventBatch {subscription_id:id.into(),after_seq:sub.acked,through_seq:through,events:events.into_iter().filter(|e|scope_match(&sub.scopes,&e.scope)).collect()})
    }
    pub fn take_event_batches(&self,label:&str)->Result<Vec<EventBatch>> {
        let window=self.windows.lock().map_err(|_|error(ErrorCode::InvalidResult,"Window registry unavailable"))?.get(label).cloned().ok_or_else(||error(ErrorCode::PermissionDenied,"Unknown calling WebView"))?;
        let mut subscriptions=self.subscriptions.lock().map_err(|_|error(ErrorCode::InvalidResult,"Subscriptions unavailable"))?;let mut batches=Vec::new();
        for (id,sub) in subscriptions.iter_mut(){if sub.window_id==window.id&&!sub.snapshot_pending {let batch=self.batch(id,sub)?;if batch.through_seq>batch.after_seq{batches.push(batch);}}}Ok(batches)
    }
    pub fn exit(&self)->Result<()> {self.recording.exit()?;self.store.flush()}
}
