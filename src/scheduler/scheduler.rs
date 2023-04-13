use log::{error, info};
use nostr_sdk::Tag;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tokio_cron_scheduler::Job;

use crate::{
    nostr::nostr::NostrInstance,
    rss::{config::Feed, parser::RssParser},
    template::template::TemplateProcessor,
};

/// Cronjob creation method
pub async fn schedule(
    rule: &str,
    feed: Feed,
    map: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, NostrInstance>>>,
) -> Job {
    // Create a copy of the map arc that will be solely used into the job
    let map_job_copy = Arc::clone(&map);

    let job_feed = feed.clone();
    let job = Job::new_async(rule, move |uuid, _lock| {
        // Copy feed for job execution
        let feed = job_feed.clone();

        // Get the id of the feed for further use
        let profile_ids = feed
            .profiles
            .clone()
            .unwrap_or(["default".to_string()].to_vec());

        // Arc instances for current job
        let clients_arc = Arc::clone(&clients);
        let map_arc = Arc::clone(&map_job_copy);

        Box::pin(async move {
            let mut map_lock = map_arc.lock().await;
            let feed = feed.clone();
            let uuid = &uuid.to_string();
            let mut map = map_lock[uuid].clone();

            match RssParser::get_items(feed.url.to_string()).await {
                Ok(entries) => {
                    let clients_lock = clients_arc.lock().await;

                    for entry in entries {
                        let entry_id = &entry.id;

                        match &map.contains(entry_id) {
                            true => {
                                info!(
                                    "Found entry for {} on feed with id {}, skipping publication.",
                                    entry_id, &feed.id
                                );
                            }
                            false => {
                                info!(
                                    "Entry not found for {} on feed with id {}, publishing...",
                                    entry_id, &feed.id
                                );

                                let mut tags = Vec::new();

                                if feed.clone().tags.is_some() {
                                    for tag in feed.clone().tags.unwrap() {
                                        tags.push(Tag::Hashtag(tag.clone()));
                                    }
                                }

                                let message =
                                    match TemplateProcessor::parse(feed.clone(), entry.clone()) {
                                        Ok(message) => message,
                                        Err(e) => {
                                            // make tick fail in non-critical way
                                            error!("{}", e);
                                            return ();
                                        }
                                    };

                                for profile_id in &profile_ids {
                                    let client = clients_lock.get(profile_id);

                                    if client.is_none() {
                                        error!(
                                            "No client found for this stream : {}. Job skipped.",
                                            feed.name
                                        );
                                    }

                                    if client.is_some() {
                                        client.unwrap().send_message(&message, &tags).await;
                                    }
                                }

                                map.insert(0, entry.id);
                            }
                        }
                    }

                    // Remove old entries if the vec has over 200 elements
                    // The limit of entries should be provided dynamicaly in further
                    // iterations.
                    // @todo: move to env config
                    map.truncate(feed.cache_size);
                    _ = &map_lock.insert(uuid.to_string(), map);
                }
                Err(_) => {
                    error!(
                        "Error while parsing RSS stream for feed with {} id. Skipping...",
                        feed.id
                    );
                }
            };
        })
    })
    .unwrap();

    let f = feed.clone();

    // Initialize the Vec that will store the retained entries of feed for current feed.
    let mut map_lock = map.lock().await;
    let initial_snapshot = feed_snapshot(f).await;
    map_lock.insert(job.guid().to_string(), initial_snapshot);

    job
}

// Retrieves a feed and returns a vec of ids for the feed.
// This method is used to provide initial snapshot of the rss feeds
// In order to avoid to spam relays with initial rss feed fetch.
pub async fn feed_snapshot(feed: Feed) -> Vec<String> {
    let mut vec = Vec::new();
    match RssParser::get_items(feed.url.to_string()).await {
        Ok(entries) => {
            for entry in entries {
                vec.push(entry.id)
            }
        }
        Err(_) => {
            error!(
                "Error while parsing RSS stream for feed with {} id. Skipping initial snapshot",
                feed.id
            );
        }
    };

    vec
}
