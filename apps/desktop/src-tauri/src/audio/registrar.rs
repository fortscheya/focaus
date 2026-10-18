//! R0 file writing never waits on SQLite's single writer. This separate bounded
//! queue registers fsynced journals. Overflow keeps disk journals for stop/restart
//! recovery instead of retaining unbounded metadata or blocking microphone I/O.
use super::{recover_recording_journals, PendingGap, PendingSegment};
use crate::storage::{Result, Store};
use focaus_contracts::{error, ErrorCode};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
};

enum Message {
    Started,
    Segment(PendingSegment),
    Gap(PendingGap),
}
pub struct MetadataRegistrar {
    sender: SyncSender<Message>,
    join: JoinHandle<Result<()>>,
    failed: Arc<AtomicBool>,
    store: Store,
    recording: String,
}
impl MetadataRegistrar {
    pub fn new(store: Store, recording: String, capacity: usize) -> Result<Self> {
        if capacity == 0 || capacity > 4096 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Invalid audio metadata queue budget",
            ));
        }
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let failed = Arc::new(AtomicBool::new(false));
        let flag = failed.clone();
        let worker_store = store.clone();
        let worker_recording = recording.clone();
        let join = thread::Builder::new()
            .name("focaus-audio-metadata".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    while let Ok(message) = receiver.recv() {
                        match message {
                            Message::Started => mark_started(&worker_store, &worker_recording)?,
                            Message::Segment(segment) => {
                                mark_started(&worker_store, &worker_recording)?;
                                segment.register(&worker_store, false)?;
                            }
                            Message::Gap(gap) => {
                                gap.register(&worker_store)?;
                            }
                        }
                    }
                    Ok(())
                })();
                if result.is_err() {
                    flag.store(true, Ordering::Release);
                }
                result
            })
            .map_err(|_| {
                error(
                    ErrorCode::InvalidResult,
                    "Cannot create audio metadata registrar",
                )
            })?;
        Ok(Self {
            sender,
            join,
            failed,
            store,
            recording,
        })
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire) || self.join.is_finished()
    }
    fn enqueue(&self, message: Message) -> Result<()> {
        match self.sender.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Ok(()),
            // The worker's error is returned by finish. Keep sealing the
            // already-buffered original frames before reporting that failure.
            Err(TrySendError::Disconnected(_)) => Ok(()),
        }
    }
    pub fn started(&self) -> Result<()> {
        self.enqueue(Message::Started)
    }
    pub fn segment(&self, segment: PendingSegment) -> Result<()> {
        self.enqueue(Message::Segment(segment))
    }
    pub fn gap(&self, gap: PendingGap) -> Result<()> {
        self.enqueue(Message::Gap(gap))
    }
    /// Called after the device and original writer stop, outside a DB transaction.
    /// It drains queued work and any overflow journals before confirming stopped.
    pub fn finish(self) -> Result<()> {
        drop(self.sender);
        self.join.join().map_err(|_| {
            error(
                ErrorCode::InvalidResult,
                "Audio metadata registrar panicked",
            )
        })??;
        let recovered = recover_recording_journals(&self.store, &self.recording)?;
        if recovered.failed_segments > 0 {
            return Err(error(
                ErrorCode::CorruptAsset,
                "Some original audio metadata still requires recovery",
            ));
        }
        Ok(())
    }
}
fn mark_started(store: &Store, recording: &str) -> Result<()> {
    store.write(|t| {
        if crate::storage::lifecycle(t, recording)? == "deleted" {
            return Err(error(
                ErrorCode::SourceDeleted,
                "Deleted recording cannot restart audio metadata",
            ));
        }
        let current = crate::storage::head(t, recording)?;
        if current["state"] == "starting" {
            crate::capture::state_transaction(t, recording, "recording", None)?;
        }
        Ok(())
    })
}
