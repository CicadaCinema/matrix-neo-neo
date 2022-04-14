#[macro_use]
extern crate serde_derive;
extern crate tinytemplate;

use std::{env, process::exit};
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use regex::Regex;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use url::Url;

use matrix_sdk::{
    self,
    config::SyncSettings,
    room::Room,
    ruma::events::room::message::{
        MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent, TextMessageEventContent,
    },
    ruma::{
        MilliSecondsSinceUnixEpoch, UInt,
    },
    Client,
};

#[derive(Serialize)]
struct Context {
    last_triggered: String,
    trigger_today_count: usize,
    first_trigger_today: bool,
}

struct Storage {
    last_timestamp: MilliSecondsSinceUnixEpoch,
    timestamp_list: Vec<MilliSecondsSinceUnixEpoch>,
    last_reply: String,
}

async fn on_room_message(event: OriginalSyncRoomMessageEvent, room: Room, storage: Arc<Mutex<Storage>>, trigger_shorts: String, trigger_reply: String) {
    // copied from login.rs example
    if let Room::Joined(room_joined) = room {
        match event {
            OriginalSyncRoomMessageEvent {
                content:
                RoomMessageEventContent {
                    msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, formatted: formatted_body, .. }),
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
                println!("PLAIN {}: {}", name, msg_body);
                println!("FORMATTED {}: {}", name, formatted_body.unwrap().body);
                */

                let mut send_message = false;
                let mut content: RoomMessageEventContent = RoomMessageEventContent::text_plain("");
                {
                    // set up storage and regular expressions
                    let mut current_storage = storage.lock().unwrap();
                    let trigger_re = Regex::new(&*trigger_shorts).unwrap();
                    let reply_re = Regex::new(&*trigger_reply).unwrap();

                    // handle replies
                    if let Some(fmted_body) = formatted_body {
                        if let Some(caps) = reply_re.captures(&fmted_body.body) {
                            // this should be our reply link - store it
                            (*current_storage).last_reply = caps.get(1).unwrap().as_str().parse().unwrap();
                        }
                    } else if msg_body == ".r" && (*current_storage).last_reply != String::new() {
                        // prepare message for sending
                        content = RoomMessageEventContent::text_plain(&(*current_storage).last_reply);
                        send_message = true;
                    }
                    // handle Shorts trigger
                    else if trigger_re.is_match(&msg_body) {
                        // if bot has been triggered at least once before
                        if (*current_storage).last_timestamp != MilliSecondsSinceUnixEpoch(Default::default()) {
                            // calculate number of times triggered today
                            let mut times_triggered_today: usize = 0;
                            let unix_secs_one_day_ago: UInt = MilliSecondsSinceUnixEpoch::now().as_secs() - UInt::try_from(86400).unwrap();
                            for this_timestamp in (*current_storage).timestamp_list.iter().rev() {
                                if this_timestamp.as_secs() > unix_secs_one_day_ago {
                                    // if this trigger happened within the last day, count it
                                    times_triggered_today += 1;
                                } else {
                                    // otherwise, we can stop counting (we use .rev() to start from the end)
                                    break;
                                }
                            }

                            // set up TinyTemplate
                            let mut tt = TinyTemplate::new();
                            const TEMPLATE: &str = "<del>{last_triggered}</del> 0 seconds without posting Shorts<br>{{ if first_trigger_today }}This is the first time you've posted Shorts in the past day!{{ else }}You've posted Shorts {trigger_today_count} times in the past day!{{ endif }}";
                            tt.add_template("standard_template", TEMPLATE).unwrap();

                            // set up template arguments
                            let context = Context {
                                last_triggered: (MilliSecondsSinceUnixEpoch::now().as_secs() - ((*current_storage).last_timestamp).as_secs()).to_string(),
                                // account for THIS trigger
                                trigger_today_count: times_triggered_today + 1,
                                first_trigger_today: times_triggered_today == 0,
                            };

                            // render message based on template
                            let formatted_message = tt.render("standard_template", &context).unwrap();

                            // prepare message for sending
                            content = RoomMessageEventContent::text_html(formatted_message.clone(), formatted_message);
                            send_message = true;
                        }

                        // update storage, take THIS trigger into account
                        (*current_storage).timestamp_list.push(origin_server_ts);
                        (*current_storage).last_timestamp = origin_server_ts;
                    }
                }

                // actually post message event
                if send_message {
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

    let a: Arc<Mutex<Storage>> = Arc::new(
        Mutex::new(
            Storage {
                last_timestamp: MilliSecondsSinceUnixEpoch(Default::default()),
                timestamp_list: vec![],
                last_reply: String::new(),
            }
        )
    );

    client.register_event_handler(move |ev, room| on_room_message(ev, room, a.clone(), r"youtube\.com/shorts".to_string(), r###"(https://matrix\.to/#/.*?)(?:\?|")"###.to_string())).await;

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