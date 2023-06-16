use std::{collections::VecDeque, sync::Arc, time::Duration};

use bytes::BytesMut;
use futures::channel::mpsc::Sender;
use futures::lock::Mutex;
use futures::Stream;
use tokio::sync::RwLock;
use tracing::{debug, error, error_span, info, trace_span};

use crate::common::parse::parse_change_id_from_bytes;
use crate::common::poe_api::{get_oauth_token, user_agent, OAuthResponse};
use crate::common::{ChangeId, StashTabResponse};

#[derive(Default, Debug)]
pub struct Indexer {
    pub(crate) is_stopping: bool,
}

#[derive(Debug)]
pub struct Config {
    pub client_id: String,
    pub client_secret: String,
    pub credentials: Option<OAuthResponse>,
}

impl Config {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            credentials: None,
        }
    }
}

#[cfg(feature = "async")]
impl Indexer {
    pub fn new() -> Self {
        Self { is_stopping: false }
    }

    pub fn stop(&mut self) {
        self.is_stopping = true;
        info!("Stopping indexer");
    }

    pub fn is_stopping(&self) -> bool {
        self.is_stopping
    }

    /// Start the indexer with a given change_id
    pub async fn start_at_change_id(
        &self,
        mut config: Config,
        change_id: ChangeId,
    ) -> impl Stream<Item = IndexerMessage> {
        // Workaround to not have to use [tracing::instrument]
        trace_span!("start_at_change_id", change_id = change_id.inner.as_str());

        info!("Starting at change id: {}", change_id);

        let credentials = get_oauth_token(&config.client_id, &config.client_secret)
            .await
            .expect("Fetch OAuth credentials");
        config.credentials = Some(credentials);

        let jobs = Arc::new(Mutex::new(VecDeque::new()));
        let (tx, rx) = futures::channel::mpsc::channel(42);

        schedule_job(jobs, tx, change_id, Arc::new(RwLock::new(config)));
        rx
    }
}

fn schedule_job(
    jobs: Arc<Mutex<VecDeque<ChangeId>>>,
    tx: Sender<IndexerMessage>,
    next_change_id: ChangeId,
    config: Arc<RwLock<Config>>,
) {
    tokio::spawn(async move {
        jobs.lock().await.push_back(next_change_id);
        process(jobs, tx, config).await
    });
}

#[tracing::instrument(skip(jobs, tx))]
async fn process(
    jobs: Arc<Mutex<VecDeque<ChangeId>>>,
    mut tx: Sender<IndexerMessage>,
    config: Arc<RwLock<Config>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: check if stopping

    let next = jobs.lock().await.pop_back();

    if let Some(change_id) = next {
        let url = format!(
            "https://api.pathofexile.com/public-stash-tabs?id={}",
            &change_id
        );
        tracing::trace!(change_id = ?change_id, url = ?url);
        debug!("Requesting {}", url);

        // TODO: static client somewhere
        let client = reqwest::ClientBuilder::new().build()?;
        let response = client
            .get(url)
            .header("Accept", "application/json")
            .header("User-Agent", user_agent(&config.read().await.client_id))
            .header(
                "Authorization",
                format!(
                    "Bearer {}",
                    config
                        .read()
                        .await
                        .credentials
                        .as_ref()
                        .expect("OAuth credentials are set")
                        .access_token
                )
                .as_str(),
            )
            .send()
            .await;

        let mut response = match response {
            Err(e) => {
                error_span!("handle_fetch_error").in_scope(|| {
                    error!("Error response: {:?}", e);
                    tracing::trace!(fetch_error = ?e);
                    schedule_job(jobs, tx, change_id, config);
                });
                return Ok(());
            }
            Ok(data) => data,
        };

        let mut bytes = BytesMut::new();
        let mut prefetch_done = false;
        while let Some(chunk) = response.chunk().await? {
            bytes.extend_from_slice(&chunk);

            if bytes.len() > 120 && !prefetch_done {
                let next_change_id = parse_change_id_from_bytes(&bytes).unwrap();
                tracing::trace!(next_change_id = ?next_change_id);
                prefetch_done = true;
                schedule_job(jobs.clone(), tx.clone(), next_change_id, config.clone());
            }
        }

        // TODO: handle errors by rescheduling based on error
        let response = serde_json::from_slice::<StashTabResponse>(&bytes)?;
        debug!(
            "Read response {} with {} stashes",
            response.next_change_id,
            response.stashes.len()
        );
        tracing::trace!(number_stashes = ?response.stashes.len());

        // reschedule if payload is empty
        if response.stashes.is_empty() {
            info!("Empty response, rescheduling {change_id}");
            tokio::time::sleep(Duration::from_secs(2)).await;
            schedule_job(jobs, tx, change_id, config);
            return Ok(());
        }

        tx.try_send(IndexerMessage::Tick {
            response,
            previous_change_id: change_id.clone(),
            change_id,
            created_at: std::time::SystemTime::now(),
        })
        .expect("Sending IndexerMessage failed");
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum IndexerMessage {
    Tick {
        response: StashTabResponse,
        change_id: ChangeId,
        previous_change_id: ChangeId,
        created_at: std::time::SystemTime,
    },
    RateLimited(Duration),
    Stop,
}
