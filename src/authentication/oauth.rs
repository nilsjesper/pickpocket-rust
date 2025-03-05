use crate::authentication::token_handler::TokenHandler;
use crate::configuration::Configuration;
use crate::logger;

pub struct OAuth {}

impl OAuth {
    pub fn request_authorization() {
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

        let client = reqwest::blocking::Client::new();
        let response = client.post(oauth_url).form(&params).send();

        let response_token = match response {
            Ok(response) => match response.text() {
                Ok(response_text) => {
                    let mut parse = url::form_urlencoded::parse(response_text.as_bytes());

                    match parse.next() {
                        Some((_code, response_token)) => response_token.to_string(),
                        None => {
                            logger::log("Could not parse Pocket's response");
                            "Error".to_owned()
                        }
                    }
                }
                Err(e) => {
                    logger::log(&format!("Could not read Pocket's response: {}", e));
                    "Error".to_owned()
                }
            },
            Err(e) => {
                logger::log(&format!("Could not connect to Pocket: {}", e));
                "Error".to_owned()
            }
        };

        if response_token == "Error" {
            logger::log("OAuth authorization failed. Please try again.");
            return;
        }

        // Open auth on browser
        let query_string = format!(
            "request_token={}&redirect_uri={}",
            response_token, pocket_homepage
        );
        let mut open_on_browser_url = url::Url::parse(auth_url).unwrap();
        open_on_browser_url.set_query(Some(&query_string));

        match open::that(open_on_browser_url.to_string()) {
            Ok(_) => {
                logger::log(
                    "Browser opened for authorization. Please authorize the app in your browser.",
                );
            }
            Err(e) => {
                logger::log(&format!("Could not open browser: {}", e));
            }
        }

        // Save OAuth token on file
        token_handler.save_oauth(&response_token);
        logger::log("OAuth token saved. Now run 'pickpocket authorize' to complete the authorization process.");
    }

    pub fn authorize() {
        let token_handler = TokenHandler::new();
        let configuration = Configuration::default();
        let (uri, consumer_key, response_token) = (
            &configuration.pocket_oauth_authorize_url,
            &configuration.consumer_key,
            &token_handler.read_oauth(),
        );

        // Request authorization token (with OAuth token + consumer key)
        let params = [("consumer_key", consumer_key), ("code", &response_token)];

        let client = reqwest::blocking::Client::new();
        let response = client.post(uri).form(&params).send();

        let response_token = match response {
            Ok(response) => match response.text() {
                Ok(response_text) => {
                    let mut parse = url::form_urlencoded::parse(response_text.as_bytes());

                    match parse.next() {
                        Some((_code, response_token)) => response_token.to_string(),
                        None => {
                            logger::log("Could not parse Pocket's response");
                            "Error".to_owned()
                        }
                    }
                }
                Err(e) => {
                    logger::log(&format!("Could not read Pocket's response: {}", e));
                    "Error".to_owned()
                }
            },
            Err(e) => {
                logger::log(&format!("Could not connect to Pocket: {}", e));
                "Error".to_owned()
            }
        };

        if response_token == "Error" {
            logger::log("Authorization failed. Please try the OAuth process again.");
            return;
        }

        // Save authentication token
        token_handler.save_auth(&response_token);
        logger::log("Authorization successful! You can now use pickpocket.");
    }
}
