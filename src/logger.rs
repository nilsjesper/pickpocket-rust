use std::time::{SystemTime, UNIX_EPOCH};

pub fn log(message: &str) -> &str {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    println!("[Pickpocket {}] {}", timestamp, message);
    message
}

pub fn info(message: &str) -> &str {
    println!("[Pickpocket INFO] {}", message);
    message
}

pub fn error(message: &str) -> &str {
    eprintln!("[Pickpocket ERROR] {}", message);
    message
}

pub fn debug(message: &str) -> &str {
    // For testing, enable debug logging by default
    println!("[Pickpocket DEBUG] {}", message);
    message
}
