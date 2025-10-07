use crate::wasm_client::ChatClient;
use anyhow::Result;
use js_sys::Date;
use tuitalk_shared::{TalkMessage, TalkProtocol};
use uuid::Uuid;
use web_sys::js_sys;

const MESSAGE_LENGTH: usize = 250;
const USERNAME_LENGTH: usize = 15;

pub fn get_unix_timestamp() -> u64 {
    let ms = Date::now();
    (ms / 1000.0) as u64
}

pub fn join_room(room_id: i32, uuid: Uuid, username: String) -> TalkProtocol {
    TalkProtocol::JoinRoom {
        room_id,
        uuid,
        username,
        unixtime: get_unix_timestamp(),
    }
}

pub fn leave_room(room_id: i32, uuid: Uuid, username: String) -> TalkProtocol {
    TalkProtocol::LeaveRoom {
        room_id,
        uuid,
        username,
        unixtime: get_unix_timestamp(),
    }
}

pub fn parse_input(app: &mut ChatClient) {
    let trimmed = app.input_text.trim();

    if trimmed.is_empty() {
        let com = TalkProtocol::LocalError {
            message: "No command given".to_string(),
        };
        app.messages.push(com);
    }

    if trimmed.len() >= MESSAGE_LENGTH {
        let com = TalkProtocol::LocalError {
            message: "Input too long".to_string(),
        };
        app.messages.push(com);
    }

    if trimmed.starts_with('/') {
        let com = parse_command(app);
    } else {
        let com = TalkProtocol::PostMessage {
            message: TalkMessage {
                uuid: app.uuid,
                username: app.username.to_string(),
                text: trimmed.to_string(),
                room_id: app.room_id,
                unixtime: get_unix_timestamp(),
            },
        };
        if let Some(sender) = app.ws_sender.clone() {
            let _ = sender.unbounded_send(com);
        }
    }
}

fn parse_command(app: &mut ChatClient) {
    let command = app.input_text.trim_start_matches('/').trim();

    if command.starts_with("name") {
        let mut new_name = command.trim_start_matches("name").trim().to_string();
        if new_name.len() > USERNAME_LENGTH {
            let com = TalkProtocol::LocalError {
                message: "Username too long".to_string(),
            };
            app.messages.push(com);
        }
        let old_username = app.username.clone();
        app.username = new_name;
        let com = TalkProtocol::ChangeName {
            uuid: app.uuid,
            username: app.username.to_string(),
            old_username: old_username,
            unixtime: get_unix_timestamp(),
        };
        if let Some(sender) = app.ws_sender.clone() {
            let _ = sender.unbounded_send(com);
        }
    } else if command.starts_with("room") {
        let number_str = command.trim_start_matches("room").trim();
        let mut com;
        match number_str.parse::<i32>() {
            Ok(number) => {
                com = TalkProtocol::LeaveRoom {
                    room_id: number,
                    uuid: app.uuid,
                    username: app.username.to_string(),
                    unixtime: get_unix_timestamp(),
                };
                if let Some(sender) = app.ws_sender.clone() {
                    let _ = sender.unbounded_send(com);
                };
                app.room_id = number;
                com = TalkProtocol::JoinRoom {
                    room_id: number,
                    uuid: app.uuid,
                    username: app.username.to_string(),
                    unixtime: get_unix_timestamp(),
                };
                if let Some(sender) = app.ws_sender.clone() {
                    let _ = sender.unbounded_send(com);
                };
            }
            Err(err) => {
                com = TalkProtocol::LocalError {
                    message: format!("Invalid room id: {}", err),
                };
                app.messages.push(com);
            }
        }
    } else if command == "clear" {
        app.messages.clear();
    } else if command == "help" {
        let com = parse_help();
        app.messages.push(com);
    } else if command.starts_with("fetch") {
        let limit_str = command.trim_start_matches("fetch").trim();
        let mut com;
        match limit_str.parse::<i64>() {
            Ok(limit) => {
                com = TalkProtocol::Fetch {
                    room_id: app.room_id,
                    limit,
                    fetch_before: get_first_message_timestamp(app),
                };
                if let Some(sender) = app.ws_sender.clone() {
                    let _ = sender.unbounded_send(com);
                };
            }
            Err(err) => {
                com = TalkProtocol::LocalError {
                    message: format!("Invalid fetch limit: {}", err),
                };
                app.messages.push(com);
            }
        }
    } else {
        let com = TalkProtocol::LocalError {
            message: format!("The command '{}' does not exist", command),
        };
        app.messages.push(com);
    }
}

pub fn get_first_message_timestamp(app: &mut ChatClient) -> u64 {
    app.messages
        .iter()
        .find_map(|proto| match proto {
            TalkProtocol::Error { .. } => None,
            TalkProtocol::LocalError { .. } => None,
            TalkProtocol::PostMessage { message } => Some(message.unixtime),
            TalkProtocol::UserJoined { unixtime, .. } => Some(*unixtime),
            TalkProtocol::UserLeft { unixtime, .. } => Some(*unixtime),
            TalkProtocol::UsernameChanged { unixtime, .. } => Some(*unixtime),
            _ => None,
        })
        .unwrap_or(get_unix_timestamp())
}

fn parse_help() -> TalkProtocol {
    TalkProtocol::LocalInformation {
        message: "/help to show this command
        /name {string} changes the name to the given string
        /room {int} changes the room to the given number
        /fetch {number} fetches the given number of messages up from the first message in your history
        /clear clears the chat"
            .to_string(),
    }
}
