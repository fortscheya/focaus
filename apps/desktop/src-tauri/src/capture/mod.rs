//! Process-owned microphone coordinator. The CPAL callback only copies into a
//! fixed pool and try_sends; SQLite, filesystem work and cloud I/O run elsewhere.
use crate::{audio::{self, SegmentMeta, SegmentWriter}, storage::{self, commit_entity, sql_error, Store, Result}};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use focaus_contracts::{error, ErrorCode, RecordingStartInput};
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::{sync::{mpsc::{self, Receiver, SyncSender, TryRecvError}, atomic::{AtomicBool, AtomicU64, Ordering}, Arc, Mutex}, thread::{self, JoinHandle}, time::{Duration, Instant}};

const BUFFER_SAMPLES:usize=4096;
const BUFFER_COUNT:usize=32;
struct Buffer { samples:Box<[f32;BUFFER_SAMPLES]>,len:usize,first_frame:u64 }
struct Producer {free:Receiver<Buffer>,ready:SyncSender<Buffer>,frame:u64,channels:usize,lost:Arc<AtomicU64>}
impl Producer {
    fn feed<T:Copy>(&mut self,data:&[T],convert:impl Fn(T)->f32) {
        let block=BUFFER_SAMPLES/self.channels*self.channels;
        for chunk in data.chunks(block) {
            let frames=chunk.len()/self.channels;
            if frames==0 {continue;}
            let first=self.frame;self.frame+=frames as u64;
            match self.free.try_recv() {
                Ok(mut buffer)=>{buffer.len=frames*self.channels;buffer.first_frame=first;for (to,from) in buffer.samples.iter_mut().zip(&chunk[..buffer.len]){*to=convert(*from);}
                    if self.ready.try_send(buffer).is_err(){self.lost.fetch_add(frames as u64,Ordering::Relaxed);}},
                Err(_)=>{self.lost.fetch_add(frames as u64,Ordering::Relaxed);}
            }
        }
    }
}
#[derive(Default,Debug)]
pub struct RunSummary {pub frames:u64,pub dropped_frames:u64,pub segments:u64,pub end_ms:u64}
struct Running { stop:Arc<AtomicBool>,join:JoinHandle<Result<RunSummary>> }
struct Active {recording_id:String,origin:Instant,base_ms:u64,run:Option<Running>,pause_at:Option<u64>}
pub struct RecordingCoordinator {store:Store,active:Mutex<Option<Active>>,playback:AtomicBool}
impl RecordingCoordinator {
    pub fn new(store:Store)->Self {Self {store,active:Mutex::new(None),playback:AtomicBool::new(false)}}
    pub fn reserve(t:&Transaction<'_>,input:&RecordingStartInput,cause:Option<&str>)->Result<Value> {
        let busy:bool=t.query_row("SELECT EXISTS(SELECT 1 FROM recording_sessions WHERE state IN ('starting','recording','paused','stopping'))",[],|r|r.get(0)).map_err(sql_error)?;
        if busy {return Err(error(ErrorCode::RecordingBusy,"Another recording is active"));}
        let tier=if let Some(lesson)=&input.lesson_id {t.query_row("SELECT COALESCE(last_recording_tier,requested_language_tier) FROM lessons WHERE lesson_id=?1",[lesson],|r|r.get::<_,u64>(0)).optional().map_err(sql_error)?.ok_or_else(||error(ErrorCode::NotFound,"Lesson unavailable"))?}else{3};
        let rid=storage::id();let origin=storage::now();
        let body=commit_entity(t,&rid,"recording",json!({"recording_id":rid,"lesson_id":input.lesson_id,"assignment_epoch":0,"state":"starting","capture_language_tier":tier,"timeline_origin":origin,"durable_watermark_ms":0,"revision_id":storage::id(),"entity_seq":0}),"recording.state_changed",json!({"kind":"global"}),cause,None)?;
        t.execute("INSERT INTO recording_sessions(recording_id,lesson_id,state,capture_language_tier,timeline_origin) VALUES(?1,?2,'starting',?3,?4)",params![rid,input.lesson_id,tier,origin]).map_err(sql_error)?;
        if let Some(lesson)=&input.lesson_id{t.execute("UPDATE lessons SET last_recording_tier=?2 WHERE lesson_id=?1",params![lesson,tier]).map_err(sql_error)?;}
        Ok(body)
    }
    /// Called only after the reservation/intent receipt commits. Replaying start
    /// does not create a second stream, including when the first stream failed.
    pub fn activate(&self,recording:&str,device:Option<&str>)->Result<()> {
        let mut active=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?;
        if let Some(existing)=active.as_ref(){if existing.recording_id==recording{return Ok(());}return Err(error(ErrorCode::RecordingBusy,"Another recording is active"));}
        let head=self.store.head(recording)?;
        if head["state"]!="starting" {return Ok(());}
        self.playback.store(false,Ordering::Release);
        let run=spawn_microphone(self.store.clone(),recording.to_owned(),device.map(str::to_owned),0)?;
        *active=Some(Active {recording_id:recording.into(),origin:Instant::now(),base_ms:0,run:Some(run),pause_at:None});Ok(())
    }
    pub fn current_ms(&self,recording:&str)->Result<u64> {
        let active=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?;
        let a=active.as_ref().filter(|a|a.recording_id==recording).ok_or_else(||error(ErrorCode::NotFound,"Recording not active"))?;
        Ok(a.base_ms+a.origin.elapsed().as_millis() as u64)
    }
    pub fn pause(&self,recording:&str)->Result<Value> {self.finish_run(recording,false)}
    pub fn stop(&self,recording:&str)->Result<Value> {self.finish_run(recording,true)}
    /// Stop the device and seal bytes outside the SQLite transaction. The final
    /// state and the command receipt are then committed together by the gateway.
    pub(crate) fn quiesce(&self,recording:&str,terminal:bool)->Result<()> {
        let mut guard=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?;
        let mut end=None;
        if let Some(a)=guard.as_mut().filter(|a|a.recording_id==recording) {
            let mut wall_end=a.base_ms+a.origin.elapsed().as_millis() as u64;
            if let Some(run)=a.run.take(){run.stop.store(true,Ordering::Release);let summary=run.join.join().map_err(|_|error(ErrorCode::DeviceLost,"Capture worker interrupted"))??;wall_end=wall_end.max(summary.end_ms);}
            a.pause_at=Some(wall_end);end=Some(wall_end);
            if terminal{*guard=None;}
        }
        if let Some(end)=end {
            let durable=self.store.read(|c|c.query_row("SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",[recording],|r|r.get::<_,u64>(0)).map_err(sql_error))?;
            if end>durable{audio::record_gap(&self.store,recording,durable,end,false)?;}
        }Ok(())
    }
    pub(crate) fn reactivate(&self,recording:&str)->Result<()> {
        let mut guard=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?;
        if guard.as_ref().is_some_and(|a|a.recording_id!=recording){return Err(error(ErrorCode::RecordingBusy,"Another recording is active"));}
        if guard.as_ref().is_some_and(|a|a.run.is_some()){return Ok(());}
        let head=self.store.head(recording)?;if head["state"]!="starting"{return Ok(());}
        let durable=self.store.read(|c|c.query_row("SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",[recording],|r|r.get::<_,u64>(0)).map_err(sql_error))?;
        let restarting=guard.is_none();let offset=guard.as_ref().map(|a|a.base_ms+a.origin.elapsed().as_millis() as u64).unwrap_or(durable);
        audio::record_gap(&self.store,recording,durable,offset,restarting)?;
        self.playback.store(false,Ordering::Release);
        match spawn_microphone(self.store.clone(),recording.into(),None,offset) {
            Ok(run)=>{*guard=Some(Active {recording_id:recording.into(),origin:Instant::now(),base_ms:offset,run:Some(run),pause_at:None});Ok(())},
            Err(e)=>{set_state(&self.store,recording,if restarting {"interrupted"}else{"paused"})?;Err(e)}
        }
    }
    fn finish_run(&self,recording:&str,terminal:bool)->Result<Value> {
        let mut guard=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?;
        let state=self.store.head(recording)?["state"].as_str().unwrap_or("").to_owned();
        if state=="stopped" {return self.store.head(recording);}
        if !terminal && state=="paused" {return self.store.head(recording);}
        if terminal {set_state(&self.store,recording,"stopping")?;}
        let mut end=0;
        if let Some(a)=guard.as_mut().filter(|a|a.recording_id==recording) {
            end=a.base_ms+a.origin.elapsed().as_millis() as u64;
            if let Some(run)=a.run.take(){run.stop.store(true,Ordering::Release);match run.join.join(){Ok(Ok(summary))=>{end=end.max(summary.end_ms);},Ok(Err(e))=>{set_state(&self.store,recording,"interrupted")?;return Err(e);},Err(_)=>{set_state(&self.store,recording,"interrupted")?;return Err(error(ErrorCode::DeviceLost,"Capture worker interrupted"));}}}
            a.pause_at=Some(end);
        }else if !terminal {return Err(error(ErrorCode::Conflict,"Recording has no active capture"));}
        if terminal { *guard=None; }
        let durable=self.store.read(|c|c.query_row("SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",[recording],|r|r.get::<_,u64>(0)).map_err(sql_error))?;
        if end>durable {audio::record_gap(&self.store,recording,durable,end,false)?;}
        set_state(&self.store,recording,if terminal {"stopped"}else{"paused"})
    }
    pub fn resume(&self,recording:&str,device:Option<&str>)->Result<Value> {
        let mut guard=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?;
        let head=self.store.head(recording)?;let state=head["state"].as_str().unwrap_or("");
        if state!="paused"&&state!="interrupted"{return Err(error(ErrorCode::Conflict,"Only paused or interrupted capture can resume"));}
        if guard.as_ref().is_some_and(|a|a.recording_id!=recording){return Err(error(ErrorCode::RecordingBusy,"Another recording is active"));}
        let offset=guard.as_ref().map(|a|a.base_ms+a.origin.elapsed().as_millis() as u64).unwrap_or(head["durable_watermark_ms"].as_u64().unwrap_or(0));
        let durable=head["durable_watermark_ms"].as_u64().unwrap_or(0);
        // A restart cannot reconstruct the elapsed off-process time. Keep an
        // explicit uncertain boundary rather than inventing recorded silence.
        audio::record_gap(&self.store,recording,durable,offset,state=="interrupted")?;
        self.playback.store(false,Ordering::Release);
        set_state(&self.store,recording,"starting")?;
        match spawn_microphone(self.store.clone(),recording.into(),device.map(str::to_owned),offset){
            Ok(run)=>{*guard=Some(Active {recording_id:recording.into(),origin:Instant::now(),base_ms:offset,run:Some(run),pause_at:None});self.store.head(recording)},
            Err(e)=>{set_state(&self.store,recording,state)?;Err(e)}
        }
    }
    pub fn start_playback(&self)->Result<()> {
        let blocked=self.store.read(|c|c.query_row("SELECT EXISTS(SELECT 1 FROM recording_sessions WHERE state IN ('starting','recording','stopping'))",[],|r|r.get::<_,bool>(0)).map_err(sql_error))?;
        if blocked{return Err(error(ErrorCode::PlaybackBlocked,"Pause recording before playback"));}
        self.playback.store(true,Ordering::Release);Ok(())
    }
    pub fn stop_playback(&self){self.playback.store(false,Ordering::Release);}
    pub fn exit(&self)->Result<()> {
        let recording=self.active.lock().map_err(|_|error(ErrorCode::InvalidResult,"Capture coordinator unavailable"))?.as_ref().map(|a|a.recording_id.clone());
        if let Some(recording)=recording{self.stop(&recording)?;}self.store.flush()
    }
}
pub(crate) fn set_state(store:&Store,recording:&str,state:&str)->Result<Value> {
    store.write(|t|state_transaction(t,recording,state,None))
}
pub(crate) fn state_transaction(t:&Transaction<'_>,recording:&str,state:&str,cause:Option<&str>)->Result<Value> {
    let mut body=crate::storage::head(t,recording)?;body["state"]=json!(state);body["durable_watermark_ms"]=json!(t.query_row("SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",[recording],|r|r.get::<_,u64>(0)).map_err(sql_error)?);
        t.execute("UPDATE recording_sessions SET state=?2 WHERE recording_id=?1",params![recording,state]).map_err(sql_error)?;
        commit_entity(t,recording,"recording",body,"recording.state_changed",json!({"kind":"global"}),cause,None)
}
fn spawn_microphone(store:Store,recording:String,device:Option<String>,offset:u64)->Result<Running> {
    let stop=Arc::new(AtomicBool::new(false));let stop_worker=stop.clone();
    let (started_tx,started_rx)=mpsc::sync_channel(1);
    let join=thread::Builder::new().name("focaus-original-writer".into()).spawn(move||{
        let setup=(|| {
            let host=cpal::default_host();
            let dev=if let Some(name)=device {host.input_devices().map_err(|_|error(ErrorCode::DeviceLost,"Cannot list input devices"))?.find(|d|d.name().ok().as_deref()==Some(&name)).ok_or_else(||error(ErrorCode::DeviceLost,"Selected microphone unavailable"))?}else{host.default_input_device().ok_or_else(||error(ErrorCode::DeviceLost,"No input microphone"))?};
            let supported=dev.default_input_config().map_err(|_|error(ErrorCode::PermissionDenied,"Microphone configuration unavailable"))?;
            let rate=supported.sample_rate().0;let channels=supported.channels();if channels==0||channels>32{return Err(error(ErrorCode::UnsupportedCapability,"Unsupported microphone channel layout"));}
            let config=supported.config();let format=supported.sample_format();
            let (free_tx,free_rx)=mpsc::sync_channel(BUFFER_COUNT);let (ready_tx,ready_rx)=mpsc::sync_channel(BUFFER_COUNT);
            for _ in 0..BUFFER_COUNT {free_tx.send(Buffer {samples:Box::new([0.;BUFFER_SAMPLES]),len:0,first_frame:0}).unwrap();}
            let lost=Arc::new(AtomicU64::new(0));let broken=Arc::new(AtomicBool::new(false));
            let mut producer=Producer {free:free_rx,ready:ready_tx,frame:0,channels:channels as usize,lost:lost.clone()};
            let err=broken.clone();let on_error=move|_:cpal::StreamError|{err.store(true,Ordering::Release);};
            let stream=match format {
                cpal::SampleFormat::F32=>dev.build_input_stream(&config,move|x:&[f32],_:&cpal::InputCallbackInfo|producer.feed(x,|x|x),on_error,None),
                cpal::SampleFormat::I16=>dev.build_input_stream(&config,move|x:&[i16],_:&cpal::InputCallbackInfo|producer.feed(x,|x|x as f32/32768.),on_error,None),
                cpal::SampleFormat::U16=>dev.build_input_stream(&config,move|x:&[u16],_:&cpal::InputCallbackInfo|producer.feed(x,|x|(x as f32-32768.)/32768.),on_error,None),
                _=>return Err(error(ErrorCode::UnsupportedCapability,"Unsupported microphone sample format"))
            }.map_err(|_|error(ErrorCode::PermissionDenied,"Microphone could not be opened"))?;
            stream.play().map_err(|_|error(ErrorCode::DeviceLost,"Microphone could not start"))?;
            Ok((stream,ready_rx,free_tx,rate,channels,lost,broken))
        })();
        match setup {
            Err(e)=>{let _=started_tx.send(Err(e.clone()));let _=set_state(&store,&recording,"failed");Err(e)},
            Ok((stream,ready,free,rate,channels,lost,broken))=>{
                let _=started_tx.send(Ok(()));
                let result=write_loop(&store,&recording,offset,rate,channels,&ready,&free,&stop_worker,&broken,stream);
                if result.is_err(){let _=set_state(&store,&recording,"interrupted");}
                result.map(|mut r|{r.dropped_frames=lost.load(Ordering::Acquire);r})
            }
        }
    }).map_err(|_|error(ErrorCode::DeviceLost,"Cannot create capture worker"))?;
    match started_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(()))=>Ok(Running {stop,join}),
        Ok(Err(e))=>{let _=join.join();Err(e)},
        Err(_)=>{stop.store(true,Ordering::Release);Err(error(ErrorCode::DeviceLost,"Microphone setup timed out"))}
    }
}
fn write_loop(store:&Store,recording:&str,offset:u64,rate:u32,channels:u16,ready:&Receiver<Buffer>,free:&SyncSender<Buffer>,stop:&AtomicBool,broken:&AtomicBool,stream:cpal::Stream)->Result<RunSummary> {
    let mut stream=Some(stream);let mut writer:Option<SegmentWriter>=None;let mut expected_frame=0;let mut summary=RunSummary::default();let mut first=true;
    loop {
        if stop.load(Ordering::Acquire)||broken.load(Ordering::Acquire){stream.take();}
        let packet=if stream.is_none(){match ready.try_recv(){Ok(b)=>Some(b),Err(TryRecvError::Empty|TryRecvError::Disconnected)=>None}}else{match ready.recv_timeout(Duration::from_millis(50)){Ok(b)=>Some(b),Err(mpsc::RecvTimeoutError::Timeout)=>continue,Err(mpsc::RecvTimeoutError::Disconnected)=>{return Err(error(ErrorCode::DeviceLost,"Microphone callback ended"));}}};
        let Some(buffer)=packet else {break;};
        if first {set_state(store,recording,"recording")?;first=false;}
        if buffer.first_frame!=expected_frame {
            if let Some(w)=writer.take(){w.seal(store,false)?;summary.segments+=1;}
            audio::record_gap(store,recording,offset+audio::frame_ms(expected_frame,rate)?,offset+audio::frame_ms(buffer.first_frame,rate)?,false)?;
        }
        if writer.is_none(){writer=Some(SegmentWriter::create(store.root(),SegmentMeta::new(recording,rate,channels,offset+audio::frame_ms(buffer.first_frame,rate)?,buffer.first_frame)?)?);}
        let w=writer.as_mut().unwrap();w.append(&buffer.samples[..buffer.len])?;let count=buffer.len as u64/channels as u64;
        expected_frame=buffer.first_frame+count;summary.frames+=count;summary.end_ms=offset+audio::frame_ms(expected_frame,rate)?;
        let _=free.try_send(buffer);
        if w.frames()>=rate as u64 {writer.take().unwrap().seal(store,false)?;summary.segments+=1;}
    }
    if let Some(w)=writer{w.seal(store,false)?;summary.segments+=1;}
    if broken.load(Ordering::Acquire){return Err(error(ErrorCode::DeviceLost,"Microphone disconnected; saved audio retained"));}
    Ok(summary)
}
