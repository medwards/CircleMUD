use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;

use slack_morphism::prelude::*;
use slack_morphism_hyper::*;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use log::*;
use tokio::runtime::Runtime;

use std::sync::Arc;

use crate::descriptor;

// Manager launches the Events API listener and waits
// on receiving a message
// checks if that channel id is in descriptors/send_channels
// if not (new connection)
//   create a new descriptor and send channel
//   add them to the hash maps *hash maps have to be thread safe here*
// send the message to the send channel
//
// CircleMUD
// has a list of descriptor ids (basically formatted channel ids)
// asks Manager if there are new descriptors, provides a list of current descriptors
// Manager gives back a single descriptor that is not in the existing list *this should return
// multiple descriptors eventually*
// later calls descriptor read/write for all descriptors

pub struct SlackDescriptorManager {
    server: thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    bot_token: SlackApiTokenValue,
    last_seen_descriptors: HashSet<String>,
    descriptors: Arc<Mutex<HashMap<String, Arc<Mutex<Box<dyn descriptor::Descriptor>>>>>>,
    send_channels: Arc<Mutex<HashMap<String, mpsc::Sender<SlackMessageContent>>>>,
}

impl SlackDescriptorManager {
    pub fn new(signing_secret: &str, bot_token: SlackApiTokenValue) -> Self {
        // TODO: server admins will want to specify port, socket addr, other stuff
        let descriptors = Arc::new(Mutex::new(HashMap::new()));
        let send_channels = Arc::new(Mutex::new(HashMap::new()));
        SlackDescriptorManager {
            server: SlackDescriptorManager::launch_server(
                signing_secret.to_owned(),
                SlackApiToken::new(bot_token.clone()),
                descriptors.clone(),
                send_channels.clone(),
            ),
            bot_token: bot_token,
            last_seen_descriptors: HashSet::new(),
            descriptors: descriptors,
            send_channels: send_channels,
        }
    }

    fn launch_server(
        signing_secret: String,
        bot_token: SlackApiToken,
        descriptors: Arc<Mutex<HashMap<String, Arc<Mutex<Box<dyn descriptor::Descriptor>>>>>>,
        send_channels: Arc<Mutex<HashMap<String, mpsc::Sender<SlackMessageContent>>>>,
    ) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        thread::spawn(|| {
            let runtime = Runtime::new().expect("Unable to create Runtime");
            info!("Launching Slack Event API callback server");
            runtime.block_on(async {
                init_log()?;
                let hyper_connector = SlackClientHyperConnector::new();
                let client: Arc<SlackHyperClient> = Arc::new(SlackClient::new(hyper_connector));
                create_server(
                    client,
                    signing_secret,
                    bot_token,
                    descriptors,
                    send_channels,
                )
                .await
            })
        })
    }
}

impl descriptor::DescriptorManager for SlackDescriptorManager {
    fn get_new_descriptor(&mut self) -> Option<descriptor::DescriptorId> {
        // see if there are any new descriptors and return them
        let current_identifiers: HashSet<String> = self
            .descriptors
            .lock()
            .expect("unable to get lock on descriptors")
            .keys()
            .map(String::clone)
            .collect();
        let new_identifiers: HashSet<_> = current_identifiers
            .difference(&self.last_seen_descriptors)
            .collect();
        if new_identifiers.is_empty() {
            None
        } else {
            info!("Found {:?} new descriptors", new_identifiers);
            let new_identifier = new_identifiers
                .iter()
                .next()
                .expect("unexpectedly empty new_identifiers")
                .to_string();
            info!("Returning {:?}", new_identifier);
            self.last_seen_descriptors.insert(new_identifier.clone());
            Some(descriptor::DescriptorId {
                identifier: new_identifier,
                descriptor_type: "SLACK".to_owned(),
            })
        }
    }

    fn get_descriptor(
        &mut self,
        descriptor: &descriptor::DescriptorId,
    ) -> Option<Arc<Mutex<Box<dyn descriptor::Descriptor>>>> {
        self.descriptors
            .lock()
            .expect("Unable to lock descriptors")
            .get_mut(&descriptor.identifier)
            .cloned()
    }

    fn close_descriptor(&mut self, descriptor: &descriptor::DescriptorId) {
        info!("closing {:#?}", descriptor);
        let mut descriptors = self
            .descriptors
            .lock()
            .expect("unable to get lock on descriptors");
        descriptors.remove(&descriptor.identifier);
        let mut send_channels = self
            .send_channels
            .lock()
            .expect("unable to get lock on send channels");
        send_channels.remove(&descriptor.identifier);
        self.last_seen_descriptors.remove(&descriptor.identifier);
    }
}

async fn push_events_handler(
    event: SlackPushEvent,
    _client: Arc<SlackHyperClient>,
    bot_token: &SlackApiToken,
    descriptors: Arc<Mutex<HashMap<String, Arc<Mutex<Box<dyn descriptor::Descriptor>>>>>>,
    send_channels: Arc<Mutex<HashMap<String, mpsc::Sender<SlackMessageContent>>>>,
) {
    info!("{}", display_push_event(&event));
    debug!("{:#?}", event);
    if let SlackPushEvent::EventCallback(callback) = event {
        if let SlackEventCallbackBody::Message(message) = callback.event {
            if let Some(channel_type) = message.origin.channel_type {
                if channel_type == SlackChannelType("im".to_owned())
                    && message.sender.bot_id.is_none()
                {
                    let mut senders = send_channels
                        .lock()
                        .expect("Unable to get lock for senders hashmap");
                    if let Some(channel) = message.origin.channel {
                        let key = channel.to_string();
                        let mut descriptors = descriptors
                            .lock()
                            .expect("Unable to get lock for senders hashmap");
                        if !senders.contains_key(&key) && !descriptors.contains_key(&key) {
                            let (sender, receiver) = mpsc::channel();
                            senders.insert(key.clone(), sender);
                            descriptors.insert(
                                key.clone(),
                                Arc::new(Mutex::new(Box::new(SlackDescriptor::new(
                                    channel.clone(),
                                    receiver,
                                    bot_token.clone(),
                                )))),
                            );
                            info!("New Events connection from {:?}", channel);
                            // Ignore this message that added the channel (it'll be junk)
                        } else if let Some(content) = message.content {
                            // content can be None if its message.subtype is MessageDeleted
                            senders
                                .get(&key)
                                .expect("Sender went missing")
                                .send(content.clone())
                                .expect("Unable to send message over channel");
                            info!("Sent event from {:?} to descriptor", channel);
                        }
                    }
                }
            }
        }
    }
}

fn display_push_event(event: &SlackPushEvent) -> String {
    match event {
        SlackPushEvent::EventCallback(cb) => match &cb.event {
            SlackEventCallbackBody::Message(message) => format!(
                "{:?} {:?} {:?} - had text content {:?}",
                message.origin.channel,
                message.origin.channel_type,
                message.subtype,
                message
                    .content
                    .as_ref()
                    .and_then(|c| c.text.as_ref())
                    .is_some()
            ),
            _ => format!("unexpected event"),
        },
        SlackPushEvent::AppRateLimited(event) => {
            format!("AppRateLimited event {:?}", event.minute_rate_limited)
        }
        _ => "unexpected push event".to_owned(),
    }
}

fn test_error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
) {
    println!("{:#?}", err);
}

async fn create_server(
    client: Arc<SlackHyperClient>,
    signing_secret: String,
    bot_token: SlackApiToken,
    descriptors: Arc<Mutex<HashMap<String, Arc<Mutex<Box<dyn descriptor::Descriptor>>>>>>,
    send_channels: Arc<Mutex<HashMap<String, mpsc::Sender<SlackMessageContent>>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 80));
    info!("Loading server: {}", addr);

    async fn your_others_routes(
        _req: Request<Body>,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        Response::builder()
            .body("Hey, this is a default users route handler".into())
            .map_err(|e| e.into())
    }

    let push_events_config = Arc::new(SlackPushEventsListenerConfig::new(signing_secret));

    // TODO: all of this nested closure scopes is some black magic: come back and understand this
    let wrapped_push_events_handler = move |event, client| {
        let descriptors_clone = descriptors.clone();
        let send_channels_clone = send_channels.clone();
        let bot_token_clone = bot_token.clone();
        async move {
            push_events_handler(
                event,
                client,
                &bot_token_clone,
                descriptors_clone,
                send_channels_clone,
            )
            .await
        }
    };

    let make_svc = make_service_fn(move |_| {
        let thread_push_events_config = push_events_config.clone();
        let wrapped_p_clone = wrapped_push_events_handler.clone();
        let listener_environment = SlackClientEventsListenerEnvironment::new(client.clone())
            .with_error_handler(test_error_handler);
        let listener = SlackClientEventsHyperListener::new(listener_environment);
        async move {
            let routes = chain_service_routes_fn(
                listener.push_events_service_fn(thread_push_events_config, wrapped_p_clone),
                your_others_routes,
            );

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(service_fn(routes))
        }
    });

    let server = hyper::server::Server::bind(&addr).serve(make_svc);
    server.await.map_err(|e| {
        error!("Server error: {}", e);
        e.into()
    })
}

fn init_log() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use fern::colors::{Color, ColoredLevelConfig};

    let colors_level = ColoredLevelConfig::new()
        .info(Color::Green)
        .warn(Color::Magenta);

    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}{}\x1B[0m",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors_level.color(record.level()),
                format_args!(
                    "\x1B[{}m",
                    colors_level.get_color(&record.level()).to_fg_str()
                ),
                message
            ))
        })
        // Add blanket level filter -
        .level(log::LevelFilter::Info)
        // - and per-module overrides
        .level_for("hyper", log::LevelFilter::Info)
        // Output to stdout, files, and other Dispatch configurations
        .chain(std::io::stdout())
        // Apply globally
        .apply()?;

    Ok(())
}

pub struct SlackDescriptor {
    slack_bot_token: SlackApiToken,
    input_channel: Arc<Mutex<mpsc::Receiver<SlackMessageContent>>>,
    channel_id: SlackChannelId,
    identifier: descriptor::DescriptorId,
    user_id: u32,
}

impl SlackDescriptor {
    pub fn new(
        channel_id: SlackChannelId,
        input_channel: mpsc::Receiver<SlackMessageContent>,
        token: SlackApiToken,
    ) -> Self {
        let identifier = descriptor::DescriptorId {
            identifier: channel_id.to_string(),
            descriptor_type: "SLACK".to_owned(),
        };
        SlackDescriptor {
            slack_bot_token: token,
            input_channel: Arc::new(Mutex::new(input_channel)),
            channel_id: channel_id,
            identifier: identifier,
            user_id: 0,
        }
    }

    async fn send_message(
        &self,
        content: SlackMessageContent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "chat_post_message response: {:?} to {:?}",
            content.clone().text.map(|mut s| s.truncate(10)),
            self.channel_id
        );
        let request = SlackApiChatPostMessageRequest::new(self.channel_id.clone(), content);

        let hyper_connector = SlackClientHyperConnector::new();
        let client = SlackClient::new(hyper_connector);
        let session = client.open_session(&self.slack_bot_token);

        session
            .chat_post_message(&request)
            .await
            .map(|_response| ())
    }

    // TODO: on drop, close the conversation
}

impl descriptor::Descriptor for SlackDescriptor {
    fn identifier(&self) -> &descriptor::DescriptorId {
        &self.identifier
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, descriptor::ErrorCode> {
        let temp = self
            .input_channel
            .lock()
            .expect("Unable to get lock on input channel")
            .try_recv();
        match temp {
            Ok(content) => {
                let text_raw = format!("{}\n", content.text.expect("text content was empty")); // CircleMUD expects newline delimiters
                let text = text_raw.as_bytes();
                if text.len() > buf.len() {
                    // TODO: do this properly
                    println!(
                        "ERROR got slack message bigger than buffer CircleMUD allocated ({} > {}",
                        text.len(),
                        buf.len()
                    );
                    return Err(1);
                }
                let common_length = std::cmp::min(text.len(), buf.len());
                buf[0..common_length].copy_from_slice(&text[0..common_length]);
                Ok(common_length)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(0),
            Err(_) => Err(2),
        }
    }

    fn write(&mut self, content: String) -> Result<usize, descriptor::ErrorCode> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to create local runtime");
        match runtime
            .block_on(self.send_message(SlackMessageContent::new().with_text(content.clone())))
        {
            Ok(()) => Ok(content.as_bytes().len()),
            Err(_) => Err(1),
        }
    }
}
