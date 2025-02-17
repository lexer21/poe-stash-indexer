use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};

use crate::sync::indexer::IndexerMessage;

use super::{
    fetcher::{start_fetcher, FetchTask, FetcherMessage},
    worker::{start_worker, WorkerMessage, WorkerTask},
};

pub(crate) enum SchedulerMessage {
    Fetch(FetchTask),
    Work(WorkerTask),
    Done(IndexerMessage),
    RateLimited(Duration),
    Stop,
}

pub(crate) fn start_scheduler() -> (Receiver<IndexerMessage>, Sender<SchedulerMessage>) {
    // Channel scheduler -> fetcher/worker
    let (scheduler_fetcher_tx, fetcher_rx) = channel::<FetcherMessage>();
    let (scheduler_worker_tx, worker_rx) = channel::<WorkerMessage>();
    // Channel fetcher/worker -> scheduler
    let (scheduler_tx, scheduler_rx) = channel::<SchedulerMessage>();
    // Channel scheduler -> caller
    let (indexer_tx, indexer_rx) = channel::<IndexerMessage>();

    let ret = (indexer_rx, scheduler_tx.clone());

    std::thread::spawn(move || {
        let fetcher_handle = start_fetcher(fetcher_rx, scheduler_tx.clone());
        let worker_handle = start_worker(worker_rx, scheduler_tx.clone());

        while let Ok(msg) = scheduler_rx.recv() {
            match msg {
                SchedulerMessage::Stop => {
                    let _ = scheduler_fetcher_tx.send(FetcherMessage::Stop);
                    let _ = scheduler_worker_tx.send(WorkerMessage::Stop);
                    break;
                }
                SchedulerMessage::Fetch(task) => {
                    scheduler_fetcher_tx
                        .send(FetcherMessage::Task(task))
                        .expect("scheduler: Failed to send FetcherMessage::Task");
                }
                SchedulerMessage::Work(task) => {
                    scheduler_worker_tx
                        .send(WorkerMessage::Task(task))
                        .expect("scheduler: Failed to send WorkerMessage::Task");
                }
                SchedulerMessage::RateLimited(timer) => {
                    indexer_tx
                        .send(IndexerMessage::RateLimited(timer))
                        .expect("scheduler: Failed to send IndexerMessage::RateLimited");
                    std::thread::sleep(timer);
                }
                SchedulerMessage::Done(msg) => {
                    indexer_tx
                        .send(msg)
                        .expect("scheduler: Failed to send IndexerMessage");
                }
            }
        }

        fetcher_handle.join().unwrap();
        worker_handle.join().unwrap();

        indexer_tx
            .send(IndexerMessage::Stop)
            .expect("scheduler: Failed to send IndexerMessage::Stop");

        tracing::debug!("Shut down scheduler");
    });

    ret
}
