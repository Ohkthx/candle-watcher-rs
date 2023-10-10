use cbadv::config::{self, BaseConfig};
use cbadv::product::{Candle, CandleUpdate, ListProductsQuery};
use cbadv::rest::{self, Client as RestClient};
use cbadv::utils::Result as APIResult;
use cbadv::websocket::{self, CandlesEvent, Channel, Message, MessageCallback, WebSocketReader};

use std::cmp::{Ord, Ordering};
use std::collections::HashMap;
use std::process::exit;

/// Tracks the candle watcher task.
pub struct TaskTracker {
    /// Total processed candles.
    processed: usize,
    /// Holds most recent candle processed for each product.
    candles: HashMap<String, Candle>,
}

impl TaskTracker {
    /// Starts the task tracking of candles.
    pub async fn start(reader: WebSocketReader) {
        let tracker: TaskTracker = TaskTracker {
            processed: 0,
            candles: HashMap::new(),
        };

        // Start the listener.
        websocket::listener_with(reader, tracker).await;
    }

    /// Ejects completed candles.
    fn check_candle(&mut self, product_id: &str, new_candle: Candle) -> Option<Candle> {
        match self.candles.get(product_id) {
            Some(candle) => {
                if candle.start < new_candle.start {
                    // Remove/eject complete candle, and replace with new series candle.
                    let old = self.candles.remove(product_id).unwrap();
                    self.candles.insert(product_id.to_string(), new_candle);
                    return Some(old as Candle);
                } else {
                    // Replace existing.
                    self.candles.insert(product_id.to_string(), new_candle);
                }
            }
            None => {
                // Insert first candle occurrence.
                self.candles.insert(product_id.to_string(), new_candle);
            }
        }
        return None;
    }
}

impl MessageCallback for TaskTracker {
    /// Required to pass TaskTracker to the websocket listener.
    fn message_callback(&mut self, msg: APIResult<Message>) {
        // Filter all non-candle and empty updates.
        let ev: Vec<CandlesEvent> = match msg {
            Ok(value) => match value {
                Message::Candles(value) => {
                    if value.events.len() == 0 {
                        // No events / updates to process.
                        return;
                    }
                    // Events being worked with.
                    value.events
                }
                // Non-candle message.
                _ => return,
            },
            // WebSocket error.
            Err(err) => {
                println!("!WEBSOCKET ERROR! {}", err);
                return;
            }
        };

        // Combine all updates and sort by more recent -> oldest.
        let mut updates: Vec<CandleUpdate> = ev.iter().flat_map(|c| c.candles.clone()).collect();
        self.processed += updates.len();

        match updates.len().cmp(&1usize) {
            // Sort if there are more than 1 update.
            Ordering::Greater => updates.sort_by(|a, b| b.data.start.cmp(&a.data.start)),
            // No updates to process.
            Ordering::Less => return,
            _ => (),
        };

        // Check the candle, see if there is a completed cycle.
        let update = updates.remove(0);
        let product_id: String = update.product_id;
        let candle = match self.check_candle(&product_id, update.data) {
            Some(c) => c,
            None => return,
        };

        // Total Processed | Product_Id | Candle Start
        println!(
            "{} {:>10} ({}): finished candle.",
            self.processed, product_id, candle.start
        );
        // println!("{} {}: {:#?}", self.processed, product_id, candle);
    }
}

/// Watches candles for a set of products, producing candles once they are complete.
async fn candle_watcher(client: &mut websocket::Client, products: &Vec<String>) {
    // Connect and spawn a task.
    let reader = client.connect().await.unwrap();
    let listener = tokio::spawn(TaskTracker::start(reader));

    // Keep the connection open and subscribe to candles.
    client.sub(Channel::HEARTBEATS, &vec![]).await.unwrap();
    client.sub(Channel::CANDLES, products).await.unwrap();
    listener.await.unwrap()
}

/// Obtain product names of candles to be obtained.
async fn get_products(client: &RestClient) -> Vec<String> {
    println!("Getting '*-USD' products.");
    let query = ListProductsQuery {
        ..Default::default()
    };

    // Holds all of the product names.
    let mut product_names: Vec<String> = vec![];

    // Pull multiple products from the Product API.
    match client.product.get_bulk(&query).await {
        Ok(products) => {
            product_names = products
                .iter()
                // Filter products to only containing *-USD pairs.
                .filter_map(|p| match p.quote_currency_id.as_str() {
                    "USD" => Some(p.product_id.clone()),
                    _ => None,
                })
                .collect();
        }
        Err(error) => println!("Unable to get products: {}", error),
    }

    product_names
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    // Load the configuration file.
    let config: BaseConfig = match config::load("config.toml") {
        Ok(c) => c,
        Err(err) => {
            println!("Could not load configuration file.");
            if config::exists("config.toml") {
                println!("File exists, {}", err);
                exit(1);
            }

            // Create a new configuration file with defaults.
            config::create_base_config("config.toml").unwrap();
            println!("Empty configuration file created, please update it.");
            exit(1);
        }
    };

    // Create a client to interact with the API.
    let rclient = rest::from_config(&config);
    let mut wsclient = websocket::from_config(&config);

    // Products of interest.
    let products = get_products(&rclient).await;
    // let products = vec!["BTC-USD".to_string()];
    println!("Obtained {} products.", products.len());

    // Start watching candles.
    let task = candle_watcher(&mut wsclient, &products);
    task.await;

    Ok(())
}
