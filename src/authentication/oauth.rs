use crate::authentication::token_handler::TokenHandler;
use crate::configuration::Configuration;
use crate::logger;

pub struct OAuth {}

impl OAuth {
    pub fn request_authorization() {
        // Create a runtime for async operations
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(Self::request_authorization_async());
    }

    async fn request_authorization_async() {
        let token_handler = TokenHandler::new();
        let configuration = Configuration::default();
        let (auth_url, oauth_url, consumer_key, pocket_homepage) = (
            &configuration.pocket_user_authorize_url,
            &configuration.pocket_oauth_request_url,
            &configuration.consumer_key,
            &configuration.pocket_homepage,
        );

        // Fetch Pocket OAuth token
        let params = [
            ("consumer_key", consumer_key),
            ("redirect_uri", pocket_homepage),
        ];
        let response = reqwest::Client::new()
            .post(oauth_url)
            .form(&params)
            .send()
            .await;

        let response_token = match response {
            Ok(response) => match response.text().await {
                Ok(response_text) => {
                    let mut parse = url::form_urlencoded::parse(response_text.as_bytes());
                    match parse.next() {
                        Some((_code, response_token)) => response_token.to_string(),
                        None => {
                            logger::log("Invalid response format from Pocket");
                            "Error".to_owned()
                        }
                    }
                }
                Err(e) => {
                    logger::log(&format!("Error reading response: {}", e));
                    "Error".to_owned()
                }
            },
            Err(e) => {
                logger::log(&format!("Could not connect to Pocket: {}", e));
                "Error".to_owned()
            }
        };

        if response_token == "Error" {
            return;
        }

        // Open auth on browser
        let query_string = format!(
            "request_token={}&redirect_uri={}",
            response_token, pocket_homepage
        );
        let mut open_on_browser_url = url::Url::parse(auth_url).unwrap();
        open_on_browser_url.set_query(Some(&query_string));
        open::that(open_on_browser_url.into_string()).ok();

        // Save OAuth token on file
        token_handler.save_oauth(&response_token);
    }

    pub fn authorize() {
        // Create a runtime for async operations
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(Self::authorize_async());
    }

    async fn authorize_async() {
        let token_handler = TokenHandler::new();
        let configuration = Configuration::default();
        let (uri, consumer_key, response_token) = (
            &configuration.pocket_oauth_authorize_url,
            &configuration.consumer_key,
            &token_handler.read_oauth(),
        );

        // Request authorization token (with OAuth token + consumer key)
        let params = [("consumer_key", consumer_key), ("code", &response_token)];
        let response = reqwest::Client::new().post(uri).form(&params).send().await;

        let response_token = match response {
            Ok(response) => match response.text().await {
                Ok(response_text) => {
                    let mut parse = url::form_urlencoded::parse(response_text.as_bytes());
                    match parse.next() {
                        Some((_code, response_token)) => response_token.to_string(),
                        None => {
                            logger::log("Invalid response format from Pocket");
                            "Error".to_owned()
                        }
                    }
                }
                Err(e) => {
                    logger::log(&format!("Error reading response: {}", e));
                    "Error".to_owned()
                }
            },
            Err(e) => {
                logger::log(&format!("Could not connect to Pocket: {}", e));
                "Error".to_owned()
            }
        };

        if response_token == "Error" {
            return;
        }

        // Save authentication token
        token_handler.save_auth(&response_token);
    }
}
