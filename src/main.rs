mod articles;
mod authentication;
mod configuration;
mod logger;

use articles::library::Library;
use authentication::oauth::OAuth;
use clap::{Arg, Command};

fn main() {
    let matches = Command::new("Pickpocket")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Tiago Amaro <tiagopadrela@gmail.com>")
        .about("Selects a random article from your Pocket (former Read It Later)")
        .subcommand(
            Command::new("oauth")
                .about("1st authorization step: ask Pocket to allow Pickpocket app"),
        )
        .subcommand(Command::new("authorize").about(
            "2nd authorization step: allow Pickpocket read/write access to your library",
        ))
        .subcommand(Command::new("pick")
            .about("Picks a random article from your library (marking it as read)")
            .arg(
                Arg::new("quantity")
                    .short('q')
                    .help("Quantity of articles to open")
                    .required(false)
                    .value_parser(clap::value_parser!(usize))
                    .default_value("1"),
            ))
        .subcommand(Command::new("renew").about(
            "Syncs your local library with your Pocket. It will delete read articles and download new articles from your library",
        ))
        .subcommand(Command::new("status").about(
            "Show the number of read/unread articles you have on your local library",
        ))
        .get_matches();

    Library::guarantee_home_folder();

    match matches.subcommand() {
        Some(("oauth", _)) => {
            OAuth::request_authorization();
        }
        Some(("authorize", _)) => {
            OAuth::authorize();
        }
        Some(("pick", pick_matches)) => {
            let quantity = pick_matches.get_one::<usize>("quantity").unwrap();
            Library::pick(Some(*quantity));
        }
        Some(("renew", _)) => {
            Library::renew();
            Library::status();
        }
        Some(("status", _)) => {
            Library::status();
        }
        _ => {
            logger::log("Option not found. Try 'pickpocket --help' for more information.");
        }
    };
}
