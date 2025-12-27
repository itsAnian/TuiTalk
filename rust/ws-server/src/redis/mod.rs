use futures_util::StreamExt;
use redis::Client;
use redis::Connection;
use redis::aio::PubSub;
use std::{sync::Arc, env};
use tokio::sync::oneshot::Sender;
use tokio::sync::{
    Mutex as TMutex,
    mpsc::{UnboundedReceiver as TUnboundedReceiver, UnboundedSender as TUnboundedSender},
};
use tokio_tungstenite::tungstenite::protocol::Message;
use tuitalk_shared::TalkProtocol;

pub type SharedRedis = Arc<TMutex<Connection>>;

pub async fn create_redis_async_pubsub_connection() -> Result<PubSub, redis::RedisError> {
    let node_env = env::var("REDIS_NODES").unwrap_or(
        "localhost:7001".to_string()
    );
    let node_url = format!("redis://{}", node_env);
    let client = Client::open(node_url)?;

    let connection = client.get_async_pubsub().await?;

    Ok(connection)
}

pub async fn create_redis_connection() -> Result<Connection, redis::RedisError> {
    let node_env = env::var("REDIS_NODES").unwrap_or(
        "localhost:7001".to_string()
    );
    let node_url = format!("redis://{}", node_env);
    let client = Client::open(node_url).unwrap();
    let publish_conn = client.get_connection()?;
    Ok(publish_conn)
}

pub async fn subscribe_to_redis(
    tx: TUnboundedSender<Message>,
    mut room_id_receiver: TUnboundedReceiver<(i32, Sender<()>)>,
) {
    println!("[REDIS] Subbing to redis");

    // create one persistent redis connection for all rooms
    let connection = create_redis_async_pubsub_connection().await.unwrap();

    let (mut sink, mut stream) = connection.split();

    // spawn background task to receive all messages
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        while let Some(message) = stream.next().await {
            println!("[REDIS] type {:?}", message);
            if let Ok(deserialized) =
                bincode::deserialize::<TalkProtocol>(message.get_payload_bytes())
            {
                println!("[REDIS] Received {:?}", deserialized);
                let _ = tx_clone.send(Message::Binary(deserialized.serialize().unwrap().into()));
            } else {
                eprintln!("Failed to deserialize message from Redis");
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
            let _ = sink.unsubscribe(old).await;
        }

        // subscribe to new room
        println!("[REDIS] Subscribing to {}", channel);
        sink.subscribe(&channel).await.expect("SSUBSCRIBE failed");

        current_room = Some(channel);
        let _ = ack.send(());
    }
}
