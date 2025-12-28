use crate::redis::*;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt, stream::TryStreamExt};
use redis::Commands;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex as TMutex;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc::unbounded_channel, oneshot};
use tokio::{
    net::{TcpListener, TcpStream},
};
use tuitalk_shared::{TalkProtocol};

pub async fn handle_connection(
    raw_stream: TcpStream,
    addr: SocketAddr,
    shared_redis: SharedRedis,
) -> Result<()> {
    println!("[SERVER] Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream).await?;
    println!("[SERVER] WebSocket connection established: {}", addr);

    let (tx, mut rx) = unbounded_channel();
    let (room_tx, room_rx) = unbounded_channel::<(i32, oneshot::Sender<()>)>();

    let (mut outgoing, incoming) = ws_stream.split();

    // Spawn Redis subscriber
    tokio::spawn(subscribe_to_redis(tx.clone(), room_rx));

    // Process incoming messages
    let message_handler = async {
        incoming
            .try_for_each(|msg| async {
                let deserialize_msg: TalkProtocol =
                    bincode::deserialize(&msg.into_data()).expect("deserializing");
                let _ = handle_message(deserialize_msg, &room_tx, &shared_redis).await;
                Ok(())
            })
            .await
    };

    // Forward Redis messages to WebSocket
    let redis_forwarder = async {
        while let Some(msg) = rx.recv().await {
            outgoing.send(msg).await?;
        }
        Ok(())
    };

    // Run both tasks concurrently
    tokio::select! {
        result = message_handler => result,
        result = redis_forwarder => result,
    }?;

    println!("{} disconnected", addr);
    Ok(())
}

async fn handle_message(
    msg: TalkProtocol,
    room_tx: &UnboundedSender<(i32, oneshot::Sender<()>)>,
    shared_redis: &SharedRedis,
) -> Result<()> {
    println!("[SERVER] Received {:?}", msg);
    match &msg {
        TalkProtocol::JoinRoom {
            room_id,
            uuid,
            username,
            unixtime,
        } => {
            handle_join(room_id, room_tx).await?;

            let response = TalkProtocol::UserJoined {
                uuid: *uuid,
                username: username.clone(),
                room_id: *room_id,
                unixtime: *unixtime,
            };
            publish_message(shared_redis, &response, room_id).await?;
        }
        TalkProtocol::LeaveRoom {
            room_id,
            uuid,
            unixtime,
            username,
        } => {
            let response = TalkProtocol::UserLeft {
                uuid: *uuid,
                username: username.clone(),
                room_id: *room_id,
                unixtime: *unixtime,
            };
            publish_message(shared_redis, &response, room_id).await?;
        }
        TalkProtocol::PostMessage { message } => {
            publish_message(shared_redis, &msg, &message.room_id).await?;
        }
        TalkProtocol::ChangeName {
            uuid,
            username,
            unixtime,
            old_username,
        } => {
            let response = TalkProtocol::UsernameChanged {
                uuid: *uuid,
                username: username.clone(),
                old_username: old_username.clone(),
                unixtime: *unixtime,
            };

            publish_message(shared_redis, &response, &0).await?; // Fix: add room_id to ChangeName
            // Protocol
        }

        // Server -> Client events typically don't need handling here
        // These are usually sent from server to client, not received
        _ => {
            eprintln!("Unexpected server-to-client message received");
        }
    }
    Ok(())
}

async fn handle_join(
    room_id: &i32,
    room_tx: &UnboundedSender<(i32, oneshot::Sender<()>)>,
) -> Result<()> {
    let (ack_tx, ack_rx) = oneshot::channel();
    room_tx.send((*room_id, ack_tx))?;
    ack_rx.await?;
    Ok(())
}

async fn publish_message(
    shared_redis: &SharedRedis,
    msg: &TalkProtocol,
    room_id: &i32,
) -> Result<()> {
    let mut conn = shared_redis.lock().await;
    let msg_json = msg.serialize()?;
    println!("[SERVER] Publishing message: {:?}", msg_json);
    match conn.publish(room_id, msg_json) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("[SERVER] Redis publish error: {}", e);
        }
    }
    Ok(())
}

pub async fn start_ws_server() -> Result<()> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");

    println!("[SERVER] Listening on: {}", addr);

    let redis_con = create_redis_connection().await?;
    // let better_redis_con = redis_con.as_pubsub();

    let shared_con: SharedRedis = Arc::new(TMutex::new(redis_con));

    while let Ok((stream, addr)) = listener.accept().await {
        let rd_clone = Arc::clone(&shared_con);
        tokio::spawn(handle_connection(stream, addr, rd_clone)); // spawn task for each incoming connection
    }

    Ok(())
}
