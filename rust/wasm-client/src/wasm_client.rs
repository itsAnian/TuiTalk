use crate::command;
use chrono::{Local, TimeZone, Utc};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_utils::document;
use shared::wasm::{receiver_task, sender_task};
use shared::{TalkMessage, TalkProtocol};
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;
use web_sys::wasm_bindgen::JsCast;
use yew::prelude::*;

pub struct ChatClient {
    pub ws_sender: Option<UnboundedSender<TalkProtocol>>,
    pub messages: Vec<TalkProtocol>,
    pub input_text: String,
    pub username: String,
    pub room_id: i32,
    pub connected: bool,
    pub uuid: Uuid,
}

pub enum Msg {
    Connect,
    Disconnect,
    SendMessage,
    UpdateInput(String),
    UpdateUsername(String),
    UpdateRoomId(String),
    ReceivedMessage(TalkProtocol),
    ConnectionClosed,
}

fn format_timestamp(unixtime: u64) -> String {
    let timestamp = Utc.timestamp_opt(unixtime as i64, 0).single().unwrap();
    format!("<{}> ", timestamp.with_timezone(&Local).format("%H:%M"))
}

impl Component for ChatClient {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let client = Self {
            ws_sender: None,
            messages: Vec::new(),
            input_text: String::new(),
            username: "Client".to_string(),
            room_id: 0,
            connected: false,
            uuid: Uuid::new_v4(),
        };

        ctx.link().send_message(Msg::Connect);

        client
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        if let Some(container) = document().get_element_by_id("messages-container") {
            if let Ok(container) = container.dyn_into::<HtmlElement>() {
                container.set_scroll_top(container.scroll_height());
            }
        }

        match msg {
            Msg::Connect => {
                if self.connected {
                    return false;
                }

                let url = "ws://localhost:8080".to_string();
                match WebSocket::open(&url) {
                    Ok(ws) => {
                        let (write, read) = ws.split();
                        let (tx, rx) = unbounded();

                        self.ws_sender = Some(tx.clone());
                        self.connected = true;

                        let link = ctx.link().clone();
                        spawn_local(async move {
                            sender_task(rx, write).await;
                            link.send_message(Msg::ConnectionClosed);
                        });

                        let link = ctx.link().clone();
                        spawn_local(async move {
                            receiver_task(read, move |msg| match msg {
                                TalkProtocol::History { text } => {
                                    for entry in text {
                                        link.send_message(Msg::ReceivedMessage(entry));
                                    }
                                }
                                _ => {
                                    link.send_message(Msg::ReceivedMessage(msg));
                                }
                            })
                            .await;
                        });

                        if let Some(sender) = &self.ws_sender {
                            let join_msg =
                                command::join_room(self.room_id, self.uuid, self.username.clone());
                            let _ = sender.unbounded_send(join_msg);
                        }
                    }
                    Err(e) => log::error!("Failed to connect: {:?}", e),
                }
                true
            }

            Msg::Disconnect => {
                if let Some(sender) = self.ws_sender.take() {
                    let leave_msg =
                        command::leave_room(self.room_id, self.uuid, self.username.clone());
                    let _ = sender.unbounded_send(leave_msg);
                    drop(sender);
                }
                self.connected = false;
                true
            }

            Msg::SendMessage => {
                if let Some(sender) = self.ws_sender.clone() {
                    if !self.input_text.trim().is_empty() {
                        let _ = command::parse_input(self);
                        self.input_text.clear();
                    }
                }
                true
            }

            Msg::UpdateInput(text) => {
                self.input_text = text;
                true
            }

            Msg::UpdateUsername(username) => true,

            Msg::UpdateRoomId(room_str) => true,

            Msg::ReceivedMessage(msg) => {
                self.messages.push(msg);
                true
            }

            Msg::ConnectionClosed => {
                self.connected = false;
                self.ws_sender = None;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_send = ctx.link().callback(|_: MouseEvent| Msg::SendMessage);
        let on_input = ctx.link().batch_callback(|e: InputEvent| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            Some(Msg::UpdateInput(input.value()))
        });

        html! {
            <div class="chat-client">
                <div class="chat-header">
                    <h1>{"Chat Client"}</h1>
                </div>
                <div class="input-area">
                    <input
                        type="text"
                        value={self.input_text.clone()}
                        placeholder="Type a message or /command..."
                        oninput={on_input}
                        onkeypress={ctx.link().batch_callback(|e: KeyboardEvent| {
                            if e.key() == "Enter" {
                                Some(Msg::SendMessage)
                            } else { None }
                        })}
                        disabled={!self.connected}
                    />
                    <button onclick={on_send} disabled={!self.connected}>
                        {"Send"}
                    </button>
                </div>
                <div id="messages-container" class="messages">
                    {for self.messages.iter().map(|msg| self.render_message(msg))}
                </div>
            </div>
        }
    }
}

impl ChatClient {
    fn render_message(&self, msg: &TalkProtocol) -> Html {
        match msg {
            TalkProtocol::PostMessage { message } => html! {
                <div class="message-header">
                    <span class="time">{format!("{} ", format_timestamp(message.unixtime))}</span>
                    <span class="username">{format!("{}: ", &message.username)}</span>
                    <span class="message-text">{&message.text}</span>
                </div>
            },
            TalkProtocol::UserJoined {
                username, unixtime, ..
            } => html! {
                <div class="system-message">
                    <span class="time">{format!("{} ", format_timestamp(*unixtime))}</span>
                    <span>{format!("{} joined room", username)}</span>
                </div>
            },
            TalkProtocol::UserLeft { username, .. } => html! {
                <div class="system-message">
                    {format!("{} left room", username)}
                </div>
            },
            TalkProtocol::UsernameChanged {
                unixtime,
                username,
                old_username,
                ..
            } => html! {
                <div class="system-message">
                    <span class="time">{format!("{} ", format_timestamp(*unixtime))}</span>
                    <span>{format!("{} changed name to {}", old_username, username)}</span>
                </div>
            },
            TalkProtocol::Error { code, message } => html! {
                <div class="error-message">{format!("Error {}: {}", code, message)}</div>
            },
            TalkProtocol::LocalError { message } => html! {
                <div class="error-message">{format!("Error: {}", message)}</div>
            },
            TalkProtocol::LocalInformation { message } => html! {
                <div class="message" style="white-space: pre-line;">
                    {format!("Info: {}", message.trim_end())}
                </div>
            },
            _ => html! {},
        }
    }
}
