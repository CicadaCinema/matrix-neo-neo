#[macro_use]
extern crate serde_derive;
extern crate tinytemplate;

use std::{env, process::exit};
use std::sync::{Arc, Mutex};

use regex::Regex;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use url::Url;

use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::events::room::message::{
        MessageType, RoomMessageEventContent, SyncRoomMessageEvent, TextMessageEventContent,
    },
    ruma::MilliSecondsSinceUnixEpoch,
    Client,
};

#[derive(Serialize)]
struct Context {
    last_triggered: String,
}

async fn on_room_message(event: SyncRoomMessageEvent, room: Room, timestamp_storage: Arc<Mutex<MilliSecondsSinceUnixEpoch>>, re_string: String, trigger_string: String) {
    if let Room::Joined(room_joined) = room {

        match event {
            // we are looking for this event type
            // (I think) this specifies a text message
            SyncRoomMessageEvent {
                content:
                RoomMessageEventContent {
                    msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, .. }),
                    ..
                },
                origin_server_ts,
                sender,
                ..
            } => {
                // uncomment this to see debug output: the author and the message body
                /*
                let member = room_joined.get_member(&sender).await.unwrap().unwrap();
                let name = member
                    .display_name()
                    .unwrap_or_else(|| member.user_id().as_str());
                println!("{}: {}", name, msg_body);
                */

                let mut update_timestamp = false;
                let mut content: RoomMessageEventContent = RoomMessageEventContent::text_plain("");
                {

                    let mut last_timestamp = timestamp_storage.lock().unwrap();
                    let re = Regex::new(&*re_string).unwrap();

                    if re.is_match(&msg_body) {
                        if *last_timestamp == MilliSecondsSinceUnixEpoch(Default::default()) {
                            *last_timestamp = origin_server_ts;
                        } else {
                            // template! to format message
                            let template = "<del>{last_triggered}</del> 0 seconds without posting Shorts";

                            let mut tt = TinyTemplate::new();
                            tt.add_template("standard_template", template).unwrap();

                            let context = Context {
                                last_triggered: (MilliSecondsSinceUnixEpoch::now().as_secs() - (*last_timestamp).as_secs()).to_string(),
                            };

                            let formatted_message = tt.render("standard_template", &context).unwrap();


                            // end template!

                            //let formatted_message = format!("<del>{}</del> 0 seconds without posting Shorts and {}", MilliSecondsSinceUnixEpoch::now().as_secs() - (*last_timestamp).as_secs(), trigger_string);
                            content = RoomMessageEventContent::text_html(formatted_message.clone(), formatted_message);
                            *last_timestamp = origin_server_ts;
                            update_timestamp = true;
                        }
                    }
                }

                if update_timestamp {
                    room_joined.send(content, None).await.unwrap();
                }
            }
            _ => {
                //println!("some other event type was sent that we don't care about");
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
    let client = Client::new(homeserver_url).await.unwrap();

    let a: Arc<Mutex<MilliSecondsSinceUnixEpoch>> = Arc::new(Mutex::new(MilliSecondsSinceUnixEpoch(Default::default())));

    client.register_event_handler(move |ev, room| on_room_message(ev, room, a.clone(), r"youtube\.com/shorts".to_string(), "khdsfkjd".to_string())).await;

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