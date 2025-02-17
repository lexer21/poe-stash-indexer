mod config;
mod filter;
mod metrics;
mod resumption;
mod schema;
mod sinks;
mod stash_record;

#[macro_use]
extern crate diesel;
extern crate dotenv;

use std::{
    convert::TryInto,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::metrics::setup_metrics;
use crate::{
    config::{user_config::RestartMode, Configuration},
    resumption::State,
    sinks::postgres::PostgresSink,
};
use crate::{filter::filter_stash_record, sinks::rabbitmq::RabbitMqSink};
use crate::{resumption::StateWrapper, stash_record::map_to_stash_records};

use dotenv::dotenv;
use sinks::sink::Sink;
use stash_api::{
    common::{poe_ninja_client::PoeNinjaClient, ChangeId},
    r#async::indexer::{Indexer, IndexerMessage},
};
use trade_common::telemetry::setup_telemetry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    setup_telemetry("indexer").expect("Telemetry setup");

    let config = Configuration::from_env()?;
    tracing::info!("Chosen configuration: {:#?}", config);

    let signal_flag = setup_signal_handlers()?;
    let metrics = setup_metrics(config.metrics_port)?;
    let sinks = setup_sinks(&config).await?;
    let client_id = config.client_id.clone();
    let client_secret = config.client_secret.clone();

    let mut resumption = StateWrapper::load_from_file(&"./indexer_state.json");
    let indexer = Indexer::new();
    let mut rx = match (&config.user_config.restart_mode, &resumption.inner) {
        (RestartMode::Fresh, _) => {
            let latest_change_id = PoeNinjaClient::fetch_latest_change_id_async().await?;
            indexer.start_at_change_id(client_id, client_secret, latest_change_id)
        }
        (RestartMode::Resume, Some(next)) => indexer.start_at_change_id(
            client_id,
            client_secret,
            ChangeId::from_str(&next.next_change_id).unwrap(),
        ),
        (RestartMode::Resume, None) => {
            tracing::info!("No previous data found, falling back to RestartMode::Fresh");
            let latest_change_id = PoeNinjaClient::fetch_latest_change_id_async().await?;
            indexer.start_at_change_id(client_id, client_secret, latest_change_id)
        }
    }
    .await;

    let mut next_chunk_id = resumption.chunk_counter();

    while let Some(msg) = rx.recv().await {
        if signal_flag.load(Ordering::Relaxed) {
            tracing::info!("Shutdown signal detected. Shutting down gracefully.");
            break;
        }

        match msg {
            IndexerMessage::Stop => break,
            IndexerMessage::RateLimited(timer) => {
                tracing::info!("Rate limited for {} seconds...waiting", timer.as_secs());
                metrics.rate_limited.inc();
            }
            IndexerMessage::Tick {
                change_id,
                response,
                created_at,
                ..
            } => {
                tracing::info!(
                    "Processing {} ({} stashes)",
                    change_id,
                    response.stashes.len()
                );

                metrics
                    .stashes_processed
                    .inc_by(response.stashes.len().try_into().unwrap());
                metrics.chunks_processed.inc();

                let next_change_id = response.next_change_id.clone();
                let stashes =
                    map_to_stash_records(change_id.clone(), created_at, response, next_chunk_id)
                        .filter_map(|mut stash| match filter_stash_record(&mut stash, &config) {
                            filter::FilterResult::Block { reason } => {
                                tracing::debug!("Filter: Blocked stash, reason: {}", reason);
                                None
                            }
                            filter::FilterResult::Pass => Some(stash),
                            filter::FilterResult::Filter {
                                n_total,
                                n_retained,
                            } => {
                                let n_removed = n_total - n_retained;
                                if n_removed > 0 {
                                    tracing::debug!(
                                        "Filter: Removed {} \t Retained {} \t Total {}",
                                        n_removed,
                                        n_retained,
                                        n_total
                                    );
                                }
                                Some(stash)
                            }
                        })
                        .collect::<Vec<_>>();

                if !stashes.is_empty() {
                    next_chunk_id += 1;
                    for sink in &sinks {
                        sink.handle(&stashes).await?;
                    }
                }

                // Update resumption state at the end of each tick
                resumption.update(State {
                    change_id: change_id.to_string(),
                    next_change_id,
                    chunk_counter: next_chunk_id,
                });
            }
        }
    }

    match resumption.save() {
        Ok(_) => tracing::info!("Saved resumption state"),
        Err(_) => tracing::error!("Saving resumption state failed"),
    }

    Ok(())
}

fn setup_signal_handlers() -> Result<Arc<AtomicBool>, Box<dyn std::error::Error>> {
    let signal_flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, signal_flag.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, signal_flag.clone())?;
    Ok(signal_flag)
}

async fn setup_sinks<'a>(
    config: &'a Configuration,
) -> Result<Vec<Box<dyn Sink + 'a>>, Box<dyn std::error::Error>> {
    let mut sinks: Vec<Box<dyn Sink>> = vec![];

    if let Some(conf) = &config.rabbitmq {
        let mq_sink = RabbitMqSink::connect(conf.clone()).await?;
        sinks.push(Box::new(mq_sink));
        tracing::info!("Configured RabbitMQ fanout sink");
    }

    if let Some(url) = &config.database_url {
        if !url.is_empty() {
            sinks.push(Box::new(PostgresSink::connect(url).await));
            tracing::info!("Configured PostgreSQL sink");
        }
    }

    Ok(sinks)
}
