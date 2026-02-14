use rusqlite::Connection;
use std::path::Path;

fn main() {
    let path = Path::new("data/settings.db");
    match Connection::open(path) {
        Ok(_) => println!("Successfully opened data/settings.db"),
        Err(e) => println!("Failed to open data/settings.db: {}", e),
    }
}
