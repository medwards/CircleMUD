use std::collections::{HashMap, HashSet};
use std::io::{Read, Result as IoResult, Write};
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
use crate::descriptor::Descriptor;

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
    descriptors: HashMap<String, Box<dyn descriptor::Descriptor>>,
    new_descriptors: mpsc::Receiver<SlackDescriptor>,
}

impl SlackDescriptorManager {
    pub fn new(signing_secret: &str, bot_token: SlackApiTokenValue) -> Self {
        // TODO: server admins will want to specify port, socket addr, other stuff
        let (new_descriptors_send, new_descriptors) = mpsc::channel();
        SlackDescriptorManager {
            server: SlackDescriptorManager::launch_server(
                signing_secret.to_owned(),
                SlackApiToken::new(bot_token.clone()),
                new_descriptors_send,
            ),
            bot_token: bot_token,
            descriptors: HashMap::new(),
            new_descriptors: new_descriptors,
        }
    }

    fn launch_server(
        signing_secret: String,
        bot_token: SlackApiToken,
        new_descriptors_sender: mpsc::Sender<SlackDescriptor>,
    ) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        thread::spawn(|| {
            let runtime = Runtime::new().expect("Unable to create Runtime");
            info!("Launching Slack Event API callback server");
            runtime.block_on(async {
                init_log()?;
                let hyper_connector = SlackClientHyperConnector::new();
                let client: Arc<SlackHyperClient> = Arc::new(SlackClient::new(hyper_connector));
                create_server(client, signing_secret, bot_token, new_descriptors_sender).await
            })
        })
    }
}

impl descriptor::DescriptorManager for SlackDescriptorManager {
    fn get_new_descriptor(&mut self) -> Option<descriptor::DescriptorId> {
        // see if there are any new descriptors and return them
        match self.new_descriptors.try_recv() {
            Ok(descriptor) => {
                let ret = descriptor.identifier().clone();
                self.descriptors
                    .insert(ret.identifier.clone(), Box::new(descriptor));
                Some(ret)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => panic!("Channel for receiving new SlackDescriptors unexpectedly closed"),
        }
    }

    fn get_descriptor(
        &mut self,
        descriptor: &descriptor::DescriptorId,
    ) -> Option<&mut Box<dyn descriptor::Descriptor>> {
        self.descriptors.get_mut(&descriptor.identifier)
    }

    fn close_descriptor(&mut self, descriptor: &descriptor::DescriptorId) {
        info!("closing {:#?}", descriptor);
        self.descriptors.remove(&descriptor.identifier);
        // Notify new_descriptor_senders that the descriptor is gone (and therefore the message
        // receiver)?
    }
}

async fn push_events_handler(
    event: SlackPushEvent,
    _client: Arc<SlackHyperClient>,
    bot_token: &SlackApiToken,
    new_descriptors_sender: Arc<Mutex<mpsc::Sender<SlackDescriptor>>>,
    message_senders: Arc<Mutex<HashMap<String, mpsc::Sender<SlackMessageContent>>>>,
) {
    info!("{}", display_push_event(&event));
    debug!("{:#?}", event);
    if let SlackPushEvent::EventCallback(callback) = event {
        if let SlackEventCallbackBody::Message(message) = callback.event {
            if let Some(channel_type) = message.origin.channel_type {
                if channel_type == SlackChannelType("im".to_owned())
                    && message.sender.bot_id.is_none()
                {
                    let mut message_senders = message_senders
                        .lock()
                        .expect("Unable to get lock for senders hashmap");
                    if let Some(channel) = message.origin.channel {
                        let key = channel.to_string();
                        if !message_senders.contains_key(&key) {
                            insert_new_session_for_channel(
                                channel.clone(),
                                bot_token.clone(),
                                new_descriptors_sender,
                                &mut message_senders,
                            );
                            info!("New Events connection from {:?}", channel);
                            // Ignore this message that added the channel (it'll be junk)
                        } else if let Some(content) = message.content {
                            // content can be None if its message.subtype is MessageDeleted
                            match message_senders
                                .get(&key)
                                .expect("Sender went missing")
                                .send(content.clone())
                            {
                                Ok(()) => info!("Sent event from {:?} to descriptor", channel),
                                Err(e) => {
                                    insert_new_session_for_channel(
                                        channel.clone(),
                                        bot_token.clone(),
                                        new_descriptors_sender,
                                        &mut message_senders,
                                    );
                                    info!(
                                        "Old events connection failed for {:?}, creating a new one",
                                        channel
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn insert_new_session_for_channel(
    channel: SlackChannelId,
    bot_token: SlackApiToken,
    new_descriptors_sender: Arc<Mutex<mpsc::Sender<SlackDescriptor>>>,
    message_senders: &mut HashMap<String, mpsc::Sender<SlackMessageContent>>,
) {
    let (sender, receiver) = mpsc::channel();
    message_senders.insert(channel.to_string(), sender);
    new_descriptors_sender
        .lock()
        .expect("Unable to lock SlackDescriptor sender")
        .send(SlackDescriptor::new(channel.clone(), receiver, bot_token))
        .expect(&format!(
            "Unable to send new SlackDescriptor {:?} to SlackDescriptorManager",
            channel
        ));
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
    new_descriptors_sender: mpsc::Sender<SlackDescriptor>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = std::env::var("SLACK_SOCKET_ADDR")
        .unwrap_or("127.0.0.1:8000".to_owned())
        .parse()
        .expect("Invalid SLACK_SOCKET_ADDR provided");
    info!("Loading server: {}", addr);

    async fn your_others_routes(
        _req: Request<Body>,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        Response::builder()
            .body("Hey, this is a default users route handler".into())
            .map_err(|e| e.into())
    }

    let push_events_config = Arc::new(SlackPushEventsListenerConfig::new(signing_secret));
    let message_senders = Arc::new(Mutex::new(HashMap::new()));
    // TODO: Why isn't the original just cloned so there is a new Sender per thread
    let new_descriptors_sender = Arc::new(Mutex::new(new_descriptors_sender));

    // TODO: all of this nested closure scopes is some black magic: come back and understand this
    let wrapped_push_events_handler = move |event, client| {
        let message_senders_clone = message_senders.clone();
        let new_descriptors_sender_clone = new_descriptors_sender.clone();
        let bot_token_clone = bot_token.clone();
        async move {
            push_events_handler(
                event,
                client,
                &bot_token_clone,
                new_descriptors_sender_clone,
                message_senders_clone,
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
}

impl Read for SlackDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // TODO: read from a local buffer before fetching from the channel
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
                    // TODO: store the excess in a local growable buffer
                    unimplemented!();
                }
                let common_length = std::cmp::min(text.len(), buf.len());
                buf[0..common_length].copy_from_slice(&text[0..common_length]);
                Ok(common_length)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(0),
            Err(mpsc::TryRecvError::Disconnected) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "SlackDescriptorManager closed the message channel",
            )),
        }
    }
}

impl Write for SlackDescriptor {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let text = std::str::from_utf8(buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
            .to_string();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to create local runtime");
        match runtime.block_on(self.send_message(SlackMessageContent::new().with_text(text))) {
            Ok(()) => Ok(buf.len()),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Unable to send message",
            )),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        // TODO: Is there an upper limit to slack message content size? if so we might need to impl
        // this
        Ok(())
    }
}
