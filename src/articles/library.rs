use crate::articles::api::API;
use crate::articles::article::Article;
use crate::articles::inventory::Inventory;
use crate::configuration::Configuration;
use crate::logger;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct Library {
    read: Inventory,
    unread: Inventory,
}

impl Library {
    pub fn new() -> Library {
        Library {
            read: Inventory::new(),
            unread: Inventory::new(),
        }
    }

    pub fn guarantee_home_folder() {
        let config = Configuration::default();
        match std::fs::create_dir_all(config.home_folder) {
            Ok(_) => {}
            Err(error) => {
                let message = format!("Could not create home folder. Motive: {}", error);
                logger::log(&message);
            }
        };
    }

    fn write_inventory(library: &Library) {
        let config = Configuration::default();
        let library_string = serde_yaml::to_string(library).unwrap();

        std::fs::write(config.library_file, library_string).ok();
    }

    fn load() -> Library {
        let config = Configuration::default();

        if !Path::new(&config.library_file).exists() {
            logger::log("Inventory file not found. Creating...");
            Library::write_inventory(&Library::new());
            File::open(&config.library_file).unwrap();
        }

        let content = std::fs::read_to_string(config.library_file).unwrap();
        serde_yaml::from_str::<Library>(&content).unwrap()
    }

    fn random_unread_article() -> Option<Article> {
        let library = Library::load();
        let article_ids: Vec<&String> = library.unread.articles.keys().collect();
        let mut rng = rand::thread_rng();
        let choice = article_ids.choose(&mut rng);

        match choice {
            Some(article_id) => {
                let id = article_id.to_string();
                let article = &library.unread.articles[&id];

                Some(article.to_owned())
            }
            None => None,
        }
    }

    fn move_to_read(article_id: String) {
        let mut library = Library::load();

        match library.unread.articles.remove(&article_id) {
            Some(read_article) => {
                library
                    .read
                    .articles
                    .insert(read_article.id.to_owned(), read_article.to_owned());
            }
            None => {}
        };

        Library::write_inventory(&library);
    }

    pub fn status() {
        let library = Library::load();

        logger::log(&format!(
            "You have {} read articles",
            &library.read.articles.len()
        ));
        logger::log(&format!(
            "You have {} unread articles",
            &library.unread.articles.len()
        ));
    }

    pub fn pick(quantity: Option<usize>) {
        let quantity = quantity.unwrap_or(1);
        let mut opened_count = 0;

        for i in 0..quantity {
            match Library::random_unread_article() {
                Some(article) => {
                    Library::move_to_read(article.id.clone());

                    logger::log(&format!(
                        "Opening article {}/{}: {}",
                        i + 1,
                        quantity,
                        article.title
                    ));

                    match open::that(&article.url) {
                        Ok(_) => {
                            opened_count += 1;
                        }
                        Err(e) => {
                            logger::log(&format!("Failed to open article: {}", e));
                            logger::log(&format!("URL: {}", article.url));
                        }
                    }
                }
                None => {
                    logger::log("You have read all articles!");
                    break;
                }
            };
        }

        if opened_count > 0 {
            logger::log(&format!("Opened {} article(s)", opened_count));
        }
    }

    pub fn renew() {
        let api = API::new();
        let library = Library::load();

        // Delete read articles from Pocket
        let read_articles: Vec<&Article> = library.read.articles.values().collect();
        if !read_articles.is_empty() {
            logger::log(&format!(
                "Deleting {} read articles from Pocket",
                read_articles.len()
            ));
            api.delete(read_articles);
        } else {
            logger::log("No read articles to delete");
        }

        // Retrieve new articles from Pocket
        logger::log(
            "Retrieving articles from Pocket (this may take a while for large libraries)...",
        );

        // Call the new retrieve method with count=30 and offset=0
        let api_response_result = api.retrieve(30, 0);

        if let Err(e) = &api_response_result {
            logger::error(&format!("Failed to retrieve articles from Pocket: {}", e));
            return;
        }

        let api_response_str = api_response_result.unwrap();
        let api_response: serde_json::Value = match serde_json::from_str(&api_response_str) {
            Ok(value) => value,
            Err(e) => {
                logger::error(&format!("Error parsing API response: {}", e));
                return;
            }
        };

        logger::debug("Examining API response structure");
        if let Some(status) = api_response.get("status") {
            logger::debug(&format!("API response status: {}", status));
        }

        let api_list = api_response["list"].to_owned();
        logger::debug(&format!(
            "API list type: {}",
            if api_list.is_object() {
                "object"
            } else {
                "not object"
            }
        ));

        let api_articles =
            match serde_json::from_value::<HashMap<String, serde_json::Value>>(api_list) {
                Ok(articles) => {
                    logger::debug(&format!(
                        "Successfully parsed {} articles from API response",
                        articles.len()
                    ));
                    articles
                }
                Err(e) => {
                    logger::error(&format!("Error parsing Pocket response: {}", e));
                    HashMap::new()
                }
            };

        logger::log(&format!(
            "Retrieved {} articles from Pocket",
            api_articles.len()
        ));

        // Sample a few articles to verify content
        if !api_articles.is_empty() {
            let sample_count = std::cmp::min(3, api_articles.len());
            logger::debug(&format!("Sampling {} articles:", sample_count));

            for (i, (id, article)) in api_articles.iter().take(sample_count).enumerate() {
                let title = article["resolved_title"]
                    .as_str()
                    .unwrap_or_else(|| article["given_title"].as_str().unwrap_or("No title"));
                logger::debug(&format!("  Sample {}: ID={}, Title={}", i + 1, id, title));
            }
        }

        let new_inventory: HashMap<String, Article> = api_articles
            .into_iter()
            .map(|(id, data)| {
                let resolved_title = data["resolved_title"].as_str();
                let given_title = data["given_title"].as_str();

                let title = match resolved_title {
                    Some(title) if !title.is_empty() => title,
                    _ => given_title.unwrap_or("Untitled"),
                };

                (
                    id.to_string(),
                    Article {
                        id: id.to_owned(),
                        url: data["given_url"].as_str().unwrap_or("").to_owned(),
                        title: title.to_owned(),
                    },
                )
            })
            .collect();

        // Create new Library
        let new_library = Library {
            read: Inventory::new(),
            unread: Inventory {
                articles: new_inventory,
            },
        };

        Library::write_inventory(&new_library);
        logger::log(&format!(
            "Refreshed library with {} unread articles",
            new_library.unread.articles.len()
        ));
    }
}
