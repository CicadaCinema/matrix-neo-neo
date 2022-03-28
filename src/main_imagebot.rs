use std::{env, process::exit};
use std::time::Duration;

use matrix_sdk::{
    self,
    config::SyncSettings,
    room::Room,
    ruma::events::room::message::{
        MessageEventContent, MessageType, SyncMessageEvent, TextMessageEventContent,
    },
    Client,
};
use tokio::sync::broadcast;
use url::Url;


async fn on_room_message(event: SyncMessageEvent, room: Room, senderr: broadcast::Sender<String>, mut receiver: broadcast::Receiver<String>) {
    if let Room::Joined(room) = room {

        match event {
            SyncMessageEvent {
                content:
                MessageEventContent {
                    msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, .. }),
                    ..
                },
                sender,
                ..
            } => {
                let member = room.get_member(&sender).await.unwrap().unwrap();
                let name = member
                    .display_name()
                    .unwrap_or_else(|| member.user_id().as_str());
                println!("{}: {}", name, msg_body);
            }
            SyncMessageEvent {
                content:
                MessageEventContent {
                    msgtype: MessageType::Image(..),
                    ..
                },
                sender,
                event_id,
                ..
            } => {
                let member = room.get_member(&sender).await.unwrap().unwrap();

                let name = member
                    .display_name()
                    .unwrap_or_else(|| member.user_id().as_str());
                println!("{}: {}", name, event_id);

                tokio::spawn(async move {
                    // wait until someone other than the author reads the message
                    loop {
                        let read_users = room.event_read_receipts(&event_id).await.unwrap();
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        if read_users.len() != 0 {
                            break;
                        }
                    }

                    // wait for some amount of time
                    println!("[{}] started sleeping", event_id);
                    tokio::time::sleep(Duration::from_secs(20)).await;
                    println!("[{}] stopped sleeping", event_id);

                    // redact image
                    room.redact(&event_id, None, None).await.unwrap();

                    let mut received = receiver.try_recv();
                    while !received.is_err() {
                        println!("[{}] stream receive: {}", event_id, received.unwrap());
                        received = receiver.try_recv();
                    }

                });
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

    let (tx, mut _rx1) = broadcast::channel(16);

    client.register_event_handler(move |ev, room| on_room_message(ev, room, tx.clone(),tx.subscribe())).await;

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