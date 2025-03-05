use crate::articles::article::Article;
use crate::authentication::token_handler::TokenHandler;
use crate::configuration::Configuration;
use crate::logger;
use reqwest;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::error::Error;

static ACTION_DELETE: &str = "delete";

pub struct API {
    configuration: Configuration,
}

impl API {
    pub fn new() -> Self {
        Self {
            configuration: Default::default(),
        }
    }

    pub fn retrieve(&self, count: u32, offset: u32) -> Result<String, Box<dyn Error>> {
        let mut all_items = HashMap::new();
        let mut current_offset = offset;
        let mut continue_pagination = true;
        let mut page_count = 0;
        let max_pages = 100; // Increased from 50 to 100 to allow for more pages

        // Get the access token from TokenHandler
        let token_handler = TokenHandler::new();
        let access_token = token_handler.read_auth();

        while continue_pagination && page_count < max_pages {
            page_count += 1;
            let log_message = format!(
                "Retrieving page {} with offset: {}, count: {}",
                page_count, current_offset, count
            );
            logger::debug(&log_message);

            let mut params = HashMap::new();
            params.insert("state", "unread");
            params.insert("access_token", &access_token);

            // Create longer-lived values for count and offset
            let count_str = count.to_string();
            let offset_str = current_offset.to_string();

            params.insert("count", &count_str);
            params.insert("offset", &offset_str);
            params.insert("detailType", "simple");
            params.insert("consumer_key", &self.configuration.consumer_key);

            let params_debug = format!("Request parameters: {:?}", params);
            logger::debug(&params_debug);

            let client = reqwest::blocking::Client::new();
            let res = client
                .post("https://getpocket.com/v3/get")
                .form(&params)
                .send()?;

            let status = res.status();
            let body = res.text()?;

            let status_log = format!("API response status: {}", status);
            logger::debug(&status_log);

            let response: Value = serde_json::from_str(&body)?;

            if let Some(list) = response.get("list").and_then(|l| l.as_object()) {
                let items_log = format!("Page {} contains {} items", page_count, list.len());
                logger::debug(&items_log);

                if list.is_empty() {
                    logger::debug("Received empty list, stopping pagination");
                    continue_pagination = false;
                } else {
                    // Add items to our collection
                    for (id, item) in list {
                        all_items.insert(id.clone(), item.clone());
                    }

                    // Move to next page
                    current_offset += count;
                    let next_page_log =
                        format!("Moving to next page, new offset: {}", current_offset);
                    logger::debug(&next_page_log);
                }
            } else {
                logger::debug("No list found in response or list is not an object");
                continue_pagination = false;
            }

            if let Some(total) = response.get("total").and_then(|t| t.as_u64()) {
                let total_log = format!("API reports total of {} items", total);
                logger::debug(&total_log);
                if total > 0 && current_offset >= u32::try_from(total).unwrap_or(u32::MAX) {
                    let end_log = format!(
                        "Reached the end of items (offset {} >= total {})",
                        current_offset, total
                    );
                    logger::debug(&end_log);
                    continue_pagination = false;
                }
            } else {
                logger::debug("No total found in response or total is not a number");
            }
        }

        if page_count >= max_pages {
            let max_pages_log = format!(
                "Reached maximum number of pages ({}), stopping pagination",
                max_pages
            );
            logger::debug(&max_pages_log);
        }

        let total_log = format!(
            "Total items retrieved across all pages: {}",
            all_items.len()
        );
        logger::debug(&total_log);

        // Construct a response that mimics the original API response format
        let result = json!({
            "status": 1,
            "list": all_items,
            "complete": 1,
            "error": null,
            "search_meta": {
                "search_type": "normal"
            },
            "since": 1234567890,
            "total": all_items.len()
        });

        Ok(result.to_string())
    }

    pub fn delete(&self, articles: Vec<&Article>) {
        if articles.is_empty() {
            return;
        }

        // Pocket API might have limitations on the number of actions in a single request
        // Split into chunks of 100 articles to be safe
        const CHUNK_SIZE: usize = 100;

        for chunk in articles.chunks(CHUNK_SIZE) {
            self.delete_chunk(chunk.to_vec());
            logger::log(&format!("Deleted chunk of {} articles", chunk.len()));
        }
    }

    fn delete_chunk(&self, articles: Vec<&Article>) {
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
                    "action": ACTION_DELETE,
                    "item_id": article.id,
                })
            })
            .collect();

        // Convert to string
        let actions_str = actions.to_string();

        // Create a HashMap for the form parameters
        let mut params = HashMap::new();
        params.insert("consumer_key", consumer_key.as_str());
        params.insert("access_token", access_token.as_str());
        params.insert("actions", &actions_str);

        let client = reqwest::blocking::Client::new();
        let response = client.post(pocket_send_url).form(&params).send();

        match response {
            Ok(_) => {}
            Err(error) => {
                logger::log(&format!("Error deleting articles: {}", error));
            }
        }
    }
}
