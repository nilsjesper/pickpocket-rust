use crate::articles::article::Article;
use crate::authentication::token_handler::TokenHandler;
use crate::configuration::Configuration;
use crate::logger;
use futures::future::join_all;
use serde_json::{json, Value};

static ACTION_ARCHIVE: &str = "archive";
static STATE_UNREAD: &str = "unread";
static PAGE_SIZE: usize = 30;
static MAX_CONCURRENT_REQUESTS: usize = 5;

pub struct API {
    configuration: Configuration,
}

impl API {
    pub fn new() -> Self {
        Self {
            configuration: Default::default(),
        }
    }

    pub fn retrieve(&self) -> serde_json::Value {
        // Create a runtime for async operations
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(self.retrieve_async())
    }

    async fn retrieve_async(&self) -> serde_json::Value {
        let token_handler = TokenHandler::new();
        let (consumer_key, pocket_retrieve_url, access_token) = (
            &self.configuration.consumer_key,
            &self.configuration.pocket_retrieve_url,
            &token_handler.read_auth(),
        );

        let client = reqwest::Client::new();

        // Get the first page to determine how many items we have
        logger::log("Retrieving first page of articles...");
        let first_page = self
            .fetch_page(&client, pocket_retrieve_url, consumer_key, access_token, 0)
            .await;

        if first_page.is_null() {
            logger::log("Could not retrieve Pocket's data");
            return serde_json::Value::Null;
        }

        // Extract the list of articles from the first page
        let mut all_items = match first_page["list"].as_object() {
            Some(items) => items.clone(),
            None => {
                logger::log("No items found in the first page");
                return first_page;
            }
        };

        // Check if we need to fetch more pages
        let first_page_count = all_items.len();
        logger::log(&format!(
            "Retrieved {} articles from first page",
            first_page_count
        ));

        if first_page_count == 0 || first_page_count < PAGE_SIZE {
            // No more pages to fetch
            return first_page;
        }

        // Fetch additional pages in parallel
        logger::log("Fetching additional pages in parallel...");
        let mut offset = PAGE_SIZE;

        // Create futures for each page request, processing in batches for controlled concurrency
        loop {
            let mut batch_futures = Vec::new();

            for _ in 0..MAX_CONCURRENT_REQUESTS {
                batch_futures.push(self.fetch_page(
                    &client,
                    pocket_retrieve_url,
                    consumer_key,
                    access_token,
                    offset,
                ));
                offset += PAGE_SIZE;

                // Safety limit to prevent too many requests
                if offset > PAGE_SIZE * 50 {
                    break;
                }
            }

            // Process this batch of requests
            let batch_results = join_all(batch_futures).await;
            let mut empty_page_found = false;

            // Process the results from this batch
            for page_result in batch_results {
                if let Some(items) = page_result["list"].as_object() {
                    if items.is_empty() {
                        empty_page_found = true;
                        break;
                    }

                    logger::log(&format!("Retrieved {} articles from page", items.len()));

                    // Add items to our collection
                    for (id, item) in items {
                        all_items.insert(id.clone(), item.clone());
                    }
                } else {
                    empty_page_found = true;
                    break;
                }
            }

            if empty_page_found {
                break;
            }

            // If we've hit our safety limit, stop
            if offset > PAGE_SIZE * 50 {
                logger::log("Reached maximum number of pages, stopping pagination");
                break;
            }
        }

        logger::log(&format!("Total articles retrieved: {}", all_items.len()));

        // Construct the final response
        json!({
            "status": 1,
            "list": all_items
        })
    }

    async fn fetch_page(
        &self,
        client: &reqwest::Client,
        url: &str,
        consumer_key: &str,
        access_token: &str,
        offset: usize,
    ) -> Value {
        let page_num = (offset / PAGE_SIZE) + 1;
        logger::log(&format!(
            "Retrieving page {} (offset: {})",
            page_num, offset
        ));

        let params = [
            ("consumer_key", consumer_key),
            ("access_token", access_token),
            ("state", &STATE_UNREAD.to_owned()),
            ("count", &PAGE_SIZE.to_string()),
            ("offset", &offset.to_string()),
            ("detailType", &"simple".to_owned()),
        ];

        match client.post(url).form(&params).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(text) => match serde_json::from_str(&text) {
                            Ok(json) => {
                                let json: Value = json;
                                if let Some(items) = json["list"].as_object() {
                                    logger::log(&format!(
                                        "Page {} contains {} items",
                                        page_num,
                                        items.len()
                                    ));
                                }
                                json
                            }
                            Err(e) => {
                                logger::log(&format!(
                                    "Error parsing JSON from page {}: {}",
                                    page_num, e
                                ));
                                Value::Null
                            }
                        },
                        Err(e) => {
                            logger::log(&format!(
                                "Error reading response text from page {}: {}",
                                page_num, e
                            ));
                            Value::Null
                        }
                    }
                } else {
                    logger::log(&format!(
                        "Error response from page {}: {}",
                        page_num,
                        response.status()
                    ));
                    Value::Null
                }
            }
            Err(e) => {
                logger::log(&format!("Error fetching page {}: {}", page_num, e));
                Value::Null
            }
        }
    }

    pub fn archive(&self, articles: Vec<&Article>) {
        // Create a runtime for async operations
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(self.archive_async(articles));
    }

    async fn archive_async(&self, articles: Vec<&Article>) {
        let token_handler = TokenHandler::new();
        let (consumer_key, pocket_send_url, access_token) = (
            &self.configuration.consumer_key,
            &self.configuration.pocket_send_url,
            &token_handler.read_auth(),
        );

        let actions: serde_json::Value = articles
            .into_iter()
            .map(|article| {
                json!({
                    "action": ACTION_ARCHIVE,
                    "item_id": article.id,
                })
            })
            .collect();

        let params = [
            ("consumer_key", consumer_key),
            ("access_token", access_token),
            ("actions", &actions.to_string()),
        ];

        match reqwest::Client::new()
            .post(pocket_send_url)
            .form(&params)
            .send()
            .await
        {
            Ok(_) => {
                logger::log(&format!(
                    "Successfully archived {} articles",
                    actions.as_array().unwrap().len()
                ));
            }
            Err(error) => {
                logger::log(&error.to_string());
            }
        }
    }
}
