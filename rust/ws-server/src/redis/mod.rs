// use redis::cluster::{ClusterClient, ClusterClientBuilder, ClusterConnection};
// use redis::cluster_async::ClusterConnection as ClusterConnectionAsync;
use redis::AsyncCommands;
use redis::Client;
use redis::Connection;
use redis::PubSubCommands;
use redis::aio::PubSub;
use redis::{PushInfo, Value};
use std::{env, sync::Arc};
use tokio::sync::oneshot::Sender;
use tokio::sync::{
    Mutex as TMutex,
    mpsc::{UnboundedReceiver as TUnboundedReceiver, UnboundedSender as TUnboundedSender},
};
use tokio_tungstenite::tungstenite::protocol::Message;
use tuitalk_shared::TalkProtocol;

pub type SharedRedis = Arc<TMutex<Connection>>;

pub async fn create_redis_async_pubsub_connection() -> Result<PubSub, redis::RedisError> {
    let node_url = "redis://localhost:7001/?protocol=3";

    let client = Client::open(node_url)?;

    let connection = client.get_async_pubsub().await?;

    Ok(connection)
}

pub async fn create_redis_connection() -> Result<Connection, redis::RedisError> {
    let node_url: String = "redis://localhost:7001".to_string();

    let client = Client::open(node_url).unwrap();
    let publish_conn = client.get_connection()?;
    Ok(publish_conn)
}

fn extract_binary_payload_from_message(data: Vec<Value>) -> Option<Vec<u8>> {
    // Redis PMessage data format: [channel, binary_payload]
    if data.len() >= 2 {
        if let Value::BulkString(binary_data) = &data[2] {
            return Some(binary_data.clone());
        }
    }
    None
}

pub async fn subscribe_to_redis(
    tx: TUnboundedSender<Message>,
    mut room_id_receiver: TUnboundedReceiver<(i32, Sender<()>)>,
) {
    println!("[REDIS] Subbing to redis");

    // create one persistent redis connection for all rooms
    let r = create_redis_connection().await.unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    r.set_push_sender(tx);

    // spawn background task to receive all messages
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        while let Ok(message) = rx.recv() {
            println!("[REDIS] type {:?}", message);

            match message.kind {
                redis::PushKind::SMessage => {
                    if let Some(payload) = extract_binary_payload_from_message(message.data) {
                        if let Ok(deserialized) = bincode::deserialize::<TalkProtocol>(&payload) {
                            println!("[REDIS] Received {:?}", deserialized);
                            let _ = tx_clone
                                .send(Message::Binary(deserialized.serialize().unwrap().into()));
                        } else {
                            eprintln!("Failed to deserialize message from Redis");
                        }
                    }
                }
                _ => eprintln!("[SERVER] Couldn't subscribe to redis"),
            }
        }
    });

    // track currently active room
    let mut current_room: Option<String> = None;

    // listen on channel for room changes
    while let Some((room_id, ack)) = room_id_receiver.recv().await {
        let channel = format!("{}", room_id);

        // unsubscribe from old room if there was one
        if let Some(old) = &current_room {
            println!("[REDIS] Unsubscribing from {}", old);
            let _ = r.unsubscribe_resp3(old);
        }

        // subscribe to new room
        println!("[REDIS] Subscribing to {}", channel);
        r.subscribe_resp3(&channel).expect("SSUBSCRIBE failed");

        current_room = Some(channel);
        let _ = ack.send(());
    }
}
