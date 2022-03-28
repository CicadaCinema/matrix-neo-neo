use std::{env, process::exit};
use std::sync::{Arc, Mutex};

use matrix_sdk::{
    self,
    config::SyncSettings,
    room::Room,
    ruma::events::room::message::{
        MessageEventContent, MessageType, SyncMessageEvent, TextMessageEventContent,
    },
    ruma::MilliSecondsSinceUnixEpoch,
    Client,
};
use url::Url;

async fn on_room_message(event: SyncMessageEvent, room: Room, timestamp_storage: Arc<Mutex<MilliSecondsSinceUnixEpoch>>) {
    if let Room::Joined(room_joined) = room {

        match event {
            SyncMessageEvent {
                content:
                MessageEventContent {
                    msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, .. }),
                    ..
                },
                origin_server_ts,
                sender,
                ..
            } => {
                let member = room_joined.get_member(&sender).await.unwrap().unwrap();
                let name = member
                    .display_name()
                    .unwrap_or_else(|| member.user_id().as_str());
                println!("{}: {}", name, msg_body);
                let mut last_timestamp = timestamp_storage.lock().unwrap();

                if msg_body.contains("shorts") && *last_timestamp != MilliSecondsSinceUnixEpoch(Default::default()) { //youtube.com/shorts
                    let content = MessageEventContent::text_html("Shorts monitor: <del>123</del> 0 seconds", "Shorts monitor: <del>123</del> 0 seconds");
                    println!("sending");
                    //room_joined.send(content, None).await.unwrap();
                    println!("message sent");
                }
                println!("count {}", last_timestamp.as_secs());
                *last_timestamp = origin_server_ts;
            }
            _ => {
                println!("something else");
            }
        }
    }
}

async fn login(
    homeserver_url: String,
    username: &str,
    password: &str,
) -> Result<(), matrix_sdk::Error> {
    let homeserver_url = Url::parse(&homeserver_url).expect("Couldn't parse the homeserver URL");
    let client = Client::new(homeserver_url).unwrap();

    let a: Arc<Mutex<MilliSecondsSinceUnixEpoch>> = Arc::new(Mutex::new(MilliSecondsSinceUnixEpoch(Default::default())));

    client.register_event_handler(move |ev, room| on_room_message(ev, room, a.clone())).await;

    client.login(username, password, None, Some("rust-sdk")).await?;
    client.sync(SyncSettings::new()).await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), matrix_sdk::Error> {
    tracing_subscriber::fmt::init();

    let (homeserver_url, username, password) =
        match (env::args().nth(1), env::args().nth(2), env::args().nth(3)) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => {
                eprintln!(
                    "Usage: {} <homeserver_url> <username> <password>",
                    env::args().next().unwrap()
                );
                exit(1)
            }
        };

    login(homeserver_url, &username, &password).await
}