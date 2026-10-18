//! Independent original-audio persistence and rational frame mapping.
use crate::storage::{self, commit_entity, encode, io_error, sql_error, Result, Store};
use focaus_contracts::{error, ErrorCode};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub fn frame_ms(frame: u64, rate: u32) -> Result<u64> {
    if rate == 0 {
        return Err(error(ErrorCode::InvalidArgument, "Zero sample rate"));
    }
    u64::try_from((frame as u128) * 1000 / (rate as u128))
        .map_err(|_| error(ErrorCode::InvalidArgument, "Frame range overflow"))
}
pub fn raw_hash(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut total = 0;
    loop {
        let n = file.read(&mut buf).map_err(io_error)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
        total += n as u64;
    }
    Ok((format!("{:x}", digest.finalize()), total))
}
pub fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(io_error)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentMeta {
    pub recording_id: String,
    pub asset_id: String,
    pub span_id: String,
    pub mapping_id: String,
    pub resource_id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub start_ms: u64,
    pub first_frame: u64,
}
impl SegmentMeta {
    pub fn new(
        recording: &str,
        rate: u32,
        channels: u16,
        start_ms: u64,
        first_frame: u64,
    ) -> Result<Self> {
        if rate == 0 || channels == 0 || channels > 32 {
            return Err(error(ErrorCode::InvalidArgument, "Invalid capture format"));
        }
        Ok(Self {
            recording_id: recording.into(),
            asset_id: storage::id(),
            span_id: storage::id(),
            mapping_id: storage::id(),
            resource_id: storage::id(),
            sample_rate: rate,
            channels,
            start_ms,
            first_frame,
        })
    }
    pub fn relative(&self) -> String {
        format!("resources/{}.wav", self.resource_id)
    }
}

/// IEEE float WAVE preserves the device samples; no gain, mixing or denoising.
pub struct SegmentWriter {
    meta: SegmentMeta,
    file: File,
    part: PathBuf,
    journal: PathBuf,
    samples: u64,
}
impl SegmentWriter {
    pub fn create(root: &Path, meta: SegmentMeta) -> Result<Self> {
        let dir = root.join("capture").join(&meta.recording_id);
        fs::create_dir_all(&dir).map_err(io_error)?;
        let journal = dir.join(format!("{}.json", meta.asset_id));
        let mut j = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&journal)
            .map_err(io_error)?;
        j.write_all(encode(&meta)?.as_bytes()).map_err(io_error)?;
        j.sync_all().map_err(io_error)?;
        sync_directory(&dir)?;
        let part = dir.join(format!("{}.wav.part", meta.asset_id));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&part)
            .map_err(io_error)?;
        file.write_all(&wave_header(meta.sample_rate, meta.channels, 0)?)
            .map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        sync_directory(&dir)?;
        Ok(Self {
            meta,
            file,
            part,
            journal,
            samples: 0,
        })
    }
    pub fn frames(&self) -> u64 {
        self.samples / (self.meta.channels as u64)
    }
    pub fn append(&mut self, samples: &[f32]) -> Result<()> {
        if samples.len() % (self.meta.channels as usize) != 0 {
            return Err(error(ErrorCode::InvalidArgument, "Partial audio frame"));
        }
        // The bounded writer thread owns this allocation, never the device callback.
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.file.write_all(&bytes).map_err(io_error)?;
        self.samples += samples.len() as u64;
        Ok(())
    }
    pub fn seal(mut self, store: &Store, recovered: bool) -> Result<Value> {
        self.file.seek(SeekFrom::Start(0)).map_err(io_error)?;
        self.file
            .write_all(&wave_header(
                self.meta.sample_rate,
                self.meta.channels,
                self.samples
                    .checked_mul(4)
                    .ok_or_else(|| error(ErrorCode::InvalidArgument, "Audio segment too large"))?,
            )?)
            .map_err(io_error)?;
        self.file.sync_all().map_err(io_error)?;
        let final_path = store.root().join(self.meta.relative());
        fs::rename(&self.part, &final_path).map_err(io_error)?;
        sync_directory(final_path.parent().unwrap())?;
        let result = register_segment(store, &self.meta, self.frames(), recovered)?;
        fs::remove_file(&self.journal).map_err(io_error)?;
        sync_directory(self.journal.parent().unwrap())?;
        Ok(result)
    }
}
fn wave_header(rate: u32, channels: u16, data_len: u64) -> Result<[u8; 44]> {
    let data = u32::try_from(data_len)
        .map_err(|_| error(ErrorCode::InvalidArgument, "WAVE segment exceeds limit"))?;
    let riff = data
        .checked_add(36)
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "WAVE segment exceeds limit"))?;
    let mut h = [0; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&riff.to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&3u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&(rate * channels as u32 * 4).to_le_bytes());
    h[32..34].copy_from_slice(&(channels * 4).to_le_bytes());
    h[34..36].copy_from_slice(&32u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    Ok(h)
}
fn register_segment(store: &Store, m: &SegmentMeta, frames: u64, recovered: bool) -> Result<Value> {
    let (hash, length) = raw_hash(&store.root().join(m.relative()))?;
    let end_ms = m.start_ms + frame_ms(frames, m.sample_rate)?;
    let asset = json!({"audio_asset_id":m.asset_id,"recording_id":m.recording_id,"resource_id":m.resource_id,"container":"wav","tracks":[{"track_id":"microphone","codec":"pcm_f32le","sample_rate":m.sample_rate,"channels":m.channels,"time_base_num":1,"time_base_den":m.sample_rate,"pts_origin":0}],"origin":"original","decode_status":"decodable"});
    let mapping = json!({"mapping_revision_id":m.mapping_id,"derived_asset_id":m.asset_id,"segments":[{"derived_frames":{"start_frame":0,"end_frame":frames},"original_asset_id":m.asset_id,"original_track_id":"microphone","original_frames":{"start_frame":0,"end_frame":frames},"timeline":{"start_ms":m.start_ms,"end_ms":end_ms},"scale_num":1,"scale_den":1,"delay_frames":0,"max_error_ms":if recovered {1.0} else {0.999}}]});
    let span = json!({"audio_span_id":m.span_id,"recording_id":m.recording_id,"asset_id":m.asset_id,"track_id":"microphone","frame_range":{"start_frame":0,"end_frame":frames},"timeline_range":{"start_ms":m.start_ms,"end_ms":end_ms},"mapping_revision_id":m.mapping_id,"coverage_status":"captured","asr_coverage":"unsubmitted"});
    focaus_contracts::validate("AudioAsset", &asset)?;
    focaus_contracts::validate("TimeMapping", &mapping)?;
    focaus_contracts::validate("AudioSpan", &span)?;
    store.write(|t| {
        let exists:bool=t.query_row("SELECT EXISTS(SELECT 1 FROM audio_assets WHERE audio_asset_id=?1)",[&m.asset_id],|r|r.get(0)).map_err(sql_error)?;
        if exists {return Ok(span.clone());}
        t.execute("INSERT INTO resources VALUES(?1,?2,?3,'audio/wav',?4,'ready')",params![m.resource_id,m.relative(),hash,length]).map_err(sql_error)?;
        commit_entity(t,&m.asset_id,"audio_asset",asset.clone(),"recording.coverage_changed",json!({"kind":"global"}),None,None)?;
        t.execute("INSERT INTO audio_assets VALUES(?1,?2,?3,'original',?4,'decodable')",params![m.asset_id,m.recording_id,m.resource_id,encode(&asset["tracks"])?]).map_err(sql_error)?;
        t.execute("INSERT INTO resource_owners VALUES(?1,?2)",params![m.resource_id,m.asset_id]).map_err(sql_error)?;
        t.execute("INSERT INTO time_mappings VALUES(?1,?2,?3)",params![m.mapping_id,m.asset_id,encode(&mapping["segments"])?]).map_err(sql_error)?;
        commit_entity(t,&m.span_id,"audio_span",span.clone(),"recording.coverage_changed",json!({"kind":"global"}),None,None)?;
        t.execute("INSERT INTO audio_spans(audio_span_id,recording_id,asset_id,track_id,start_frame,end_frame,start_ms,end_ms,mapping_revision_id,coverage_status,asr_coverage) VALUES(?1,?2,?3,'microphone',0,?4,?5,?6,?7,'captured','unsubmitted')",params![m.span_id,m.recording_id,m.asset_id,frames,m.start_ms,end_ms,m.mapping_id]).map_err(sql_error)?;
        t.execute("UPDATE recording_sessions SET durable_watermark_ms=MAX(durable_watermark_ms,?2) WHERE recording_id=?1",params![m.recording_id,end_ms]).map_err(sql_error)?;
        let mut recording=crate::storage::head(t,&m.recording_id)?;
        recording["durable_watermark_ms"]=json!(t.query_row("SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",[&m.recording_id],|r|r.get::<_,u64>(0)).map_err(sql_error)?);
        let recording=commit_entity(t,&m.recording_id,"recording",recording,"recording.state_changed",json!({"kind":"global"}),None,None)?;
        t.execute("UPDATE outbox SET payload_json=?3 WHERE entity_id=?1 AND revision_id=?2 AND event_type='recording.state_changed'",params![m.recording_id,recording["revision_id"].as_str(),encode(&json!({"entity_id":m.recording_id,"entity_kind":"recording","recording_id":m.recording_id,"revision_id":recording["revision_id"],"change_kind":"durable_watermark","durable_watermark_ms":recording["durable_watermark_ms"]}))?]).map_err(sql_error)?;
        Ok(span.clone())
    })
}
pub fn record_gap(
    store: &Store,
    recording: &str,
    start: u64,
    end: u64,
    uncertain: bool,
) -> Result<Value> {
    if end < start {
        return Err(error(ErrorCode::InvalidArgument, "Reversed capture gap"));
    }
    let gap_id = storage::id();
    let body = json!({"audio_span_id":gap_id,"recording_id":recording,"asset_id":null,"track_id":null,"frame_range":null,"timeline_range":{"start_ms":start,"end_ms":end},"mapping_revision_id":null,"coverage_status":if uncertain {"decode_unknown"} else {"capture_gap"},"asr_coverage":"uncertain"});
    store.write(|t| {
        commit_entity(t,&gap_id,"audio_span",body.clone(),"recording.coverage_changed",json!({"kind":"global"}),None,None)?;
        t.execute("INSERT INTO audio_spans(audio_span_id,recording_id,start_ms,end_ms,coverage_status,asr_coverage) VALUES(?1,?2,?3,?4,?5,'uncertain')",params![gap_id,recording,start,end,if uncertain {"decode_unknown"}else{"capture_gap"}]).map_err(sql_error)?;Ok(body.clone())
    })
}

#[derive(Default, Debug)]
pub struct RecoveryReport {
    pub recovered_segments: u64,
    pub recovered_frames: u64,
    pub uncertain_recordings: Vec<String>,
    pub failed_segments: u64,
}
/// Reopening restores files and marks interrupted sessions; it never opens a device.
pub fn recover(store: &Store) -> Result<RecoveryReport> {
    let mut report = RecoveryReport::default();
    for dir in fs::read_dir(store.root().join("capture")).map_err(io_error)? {
        let dir = dir.map_err(io_error)?;
        if !dir.file_type().map_err(io_error)?.is_dir() {
            continue;
        }
        for entry in fs::read_dir(dir.path()).map_err(io_error)? {
            let path = entry.map_err(io_error)?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let result = (|| -> Result<(u64, String)> {
                let meta: SegmentMeta =
                    storage::decode(&fs::read_to_string(&path).map_err(io_error)?)?;
                // Journal paths are derived only from host-issued UUIDs.
                for token in [&meta.recording_id, &meta.asset_id, &meta.resource_id] {
                    if !token.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                        || token.len() != 36
                    {
                        return Err(error(ErrorCode::CorruptAsset, "Invalid capture journal"));
                    }
                }
                let deleted = store
                    .read(|c| Ok(crate::storage::lifecycle(c, &meta.recording_id)? == "deleted"))?;
                if deleted {
                    return Err(error(
                        ErrorCode::SourceDeleted,
                        "Deleted recording cannot recover",
                    ));
                }
                let final_path = store.root().join(meta.relative());
                let part = dir.path().join(format!("{}.wav.part", meta.asset_id));
                let current = if final_path.exists() {
                    final_path.clone()
                } else {
                    part.clone()
                };
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&current)
                    .map_err(io_error)?;
                let len = file.metadata().map_err(io_error)?.len();
                if len < 44 {
                    return Err(error(ErrorCode::CorruptAsset, "Incomplete audio header"));
                }
                let mut header = [0u8; 44];
                file.read_exact(&mut header).map_err(io_error)?;
                if &header[0..4] != b"RIFF"
                    || &header[8..12] != b"WAVE"
                    || &header[36..40] != b"data"
                {
                    return Err(error(ErrorCode::CorruptAsset, "Invalid capture header"));
                }
                let block = meta.channels as u64 * 4;
                if block == 0 {
                    return Err(error(ErrorCode::CorruptAsset, "Invalid capture format"));
                }
                let frames = (len - 44) / block;
                file.set_len(44 + frames * block).map_err(io_error)?;
                file.seek(SeekFrom::Start(0)).map_err(io_error)?;
                file.write_all(&wave_header(
                    meta.sample_rate,
                    meta.channels,
                    frames * block,
                )?)
                .map_err(io_error)?;
                file.sync_all().map_err(io_error)?;
                if current != final_path {
                    fs::rename(&current, &final_path).map_err(io_error)?;
                    sync_directory(final_path.parent().unwrap())?;
                }
                register_segment(store, &meta, frames, true)?;
                fs::remove_file(&path).map_err(io_error)?;
                sync_directory(path.parent().unwrap())?;
                Ok((frames, meta.recording_id))
            })();
            match result {
                Ok((frames, recording)) => {
                    report.recovered_segments += 1;
                    report.recovered_frames += frames;
                    if !report.uncertain_recordings.contains(&recording) {
                        report.uncertain_recordings.push(recording);
                    }
                }
                Err(_) => report.failed_segments += 1,
            }
        }
    }
    let active=store.read(|c|{let mut q=c.prepare("SELECT recording_id FROM recording_sessions WHERE state IN ('starting','recording','paused','stopping')").map_err(sql_error)?;let v=q.query_map([],|r|r.get::<_,String>(0)).map_err(sql_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(sql_error)?;Ok(v)})?;
    for recording in active {
        store.write(|t| {
            t.execute(
                "UPDATE recording_sessions SET state='interrupted' WHERE recording_id=?1",
                [&recording],
            )
            .map_err(sql_error)?;
            let mut body = crate::storage::head(t, &recording)?;
            body["state"] = json!("interrupted");
            body["durable_watermark_ms"] = json!(t
                .query_row(
                    "SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",
                    [&recording],
                    |r| r.get::<_, u64>(0)
                )
                .map_err(sql_error)?);
            commit_entity(
                t,
                &recording,
                "recording",
                body,
                "recording.state_changed",
                json!({"kind":"global"}),
                None,
                None,
            )?;
            Ok(())
        })?;
        if !report.uncertain_recordings.contains(&recording) {
            report.uncertain_recordings.push(recording);
        }
    }
    Ok(report)
}
