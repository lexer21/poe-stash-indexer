use std::{
    io::{Read, Write},
    sync::mpsc::{Receiver, Sender},
};

use crate::{
    common::{ChangeId, StashTabResponse},
    sync::indexer::IndexerMessage,
};

use super::scheduler::SchedulerMessage;

pub(crate) enum WorkerMessage {
    Task(WorkerTask),
    Stop,
}

pub(crate) struct WorkerTask {
    pub(crate) fetch_partial: [u8; 80],
    pub(crate) change_id: ChangeId,
    pub(crate) reader: Box<dyn Read + Send>,
}

pub(crate) fn start_worker(
    worker_rx: Receiver<WorkerMessage>,
    scheduler_tx: Sender<SchedulerMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        while let Ok(message) = worker_rx.recv() {
            buffer.clear();

            match message {
                WorkerMessage::Task(mut task) => {
                    let start = std::time::Instant::now();
                    buffer.write_all(&task.fetch_partial).unwrap();
                    task.reader.read_to_end(&mut buffer).unwrap();
                    tracing::debug!(
                        "worker: Took {}ms to read body",
                        start.elapsed().as_millis()
                    );

                    let start = std::time::Instant::now();
                    let deserialized = serde_json::from_slice::<StashTabResponse>(&buffer)
                        .expect("Deserialization of body failed");
                    tracing::debug!(
                        "worker: Took {}ms to deserialize body",
                        start.elapsed().as_millis()
                    );

                    scheduler_tx
                        .send(SchedulerMessage::Done(IndexerMessage::Tick {
                            payload: deserialized,
                            change_id: task.change_id,
                            created_at: std::time::SystemTime::now(),
                        }))
                        .expect("worker: Sending IndexerMessage::Tick failed");
                }
                WorkerMessage::Stop => break,
            }
        }

        tracing::debug!("worker: Shutting down");
    })
}
