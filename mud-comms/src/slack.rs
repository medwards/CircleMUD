use std::collections::HashMap;
use std::error::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use crossbeam_channel::Receiver;
use crossbeam_channel::Select;
use crossbeam_channel::Sender;
use crossbeam_channel::TryRecvError;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use log::*;
use slack_morphism::prelude::SlackApiChatPostMessageRequest;
use slack_morphism::prelude::SlackClientEventsListenerEnvironment;
use slack_morphism::prelude::SlackPushEventsListenerConfig;
use slack_morphism::SlackApiToken;
use slack_morphism::SlackApiTokenValue;
use slack_morphism::SlackClient;
use slack_morphism_hyper::chain_service_routes_fn;
use slack_morphism_hyper::SlackClientEventsHyperListener;
use slack_morphism_hyper::SlackClientHyperConnector;
use slack_morphism_hyper::SlackHyperClient;
use slack_morphism_models::events::SlackEventCallbackBody;
use slack_morphism_models::events::SlackPushEvent;
use slack_morphism_models::SlackChannelId;
use slack_morphism_models::SlackChannelType;
use slack_morphism_models::SlackMessageContent;
use tokio::runtime::Runtime;

use crate::descriptor::Descriptor;
use crate::descriptor::DescriptorManager;

pub struct SlackDescriptorManager {
    server: thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    bot_token: SlackApiTokenValue,
    descriptors: HashMap<String, Box<dyn Descriptor>>,
    new_descriptors: Receiver<SlackDescriptor>,
}

impl SlackDescriptorManager {
    pub fn new(signing_secret: &str, bot_token: SlackApiTokenValue) -> Self {
        let (new_descriptors_send, new_descriptors) = crossbeam_channel::unbounded();
        let server = SlackDescriptorManager::launch_server(
            signing_secret.to_owned(),
            SlackApiToken::new(bot_token.clone()),
            new_descriptors_send,
        );
        SlackDescriptorManager {
            server,
            bot_token,
            descriptors: HashMap::new(),
            new_descriptors,
        }
    }

    fn launch_server(
        signing_secret: String,
        bot_token: SlackApiToken,
        new_descriptors_sender: Sender<SlackDescriptor>,
    ) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        // get socket addr for server
        // TODO: accept SocketAddr from arguments (instead of env)
        let runtime = Runtime::new().expect("Unable to create Runtime");
        let addr = std::env::var("SLACK_SOCKET_ADDR")
            .unwrap_or("127.0.0.1:8000".to_owned())
            .parse()
            .expect("Invalid SLACK_SOCKET_ADDR provided");
        info!("Server binding address {}", addr);

        // We fail here so that binding panics happen in the main thread (otherwise the server
        // thread just dies silently)
        // `try_bind` needs an async runtime even though it says it's not async
        let server = runtime
            .block_on(async { hyper::server::Server::try_bind(&addr) })
            .expect(format!("SLOCK_SOCKET_ADDR {} should be available", &addr).as_str());

        thread::spawn(move || {
            info!("Launching Slack Event API callback server");
            runtime.block_on(async {
                let hyper_connector = SlackClientHyperConnector::new();
                let client: Arc<SlackHyperClient> = Arc::new(SlackClient::new(hyper_connector));
                serve(
                    server,
                    client,
                    signing_secret,
                    bot_token,
                    new_descriptors_sender,
                )
                .await
            })
        })
    }
}

async fn serve(
    server: hyper::server::Builder<hyper::server::conn::AddrIncoming>,
    client: Arc<SlackHyperClient>,
    signing_secret: String,
    bot_token: SlackApiToken,
    new_descriptors_sender: Sender<SlackDescriptor>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
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

    let service_fn = make_service_fn(move |_| {
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

    server.serve(service_fn).await.map_err(|e| {
        error!("Server error: {}", e);
        e.into()
    })
}

fn test_error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
) {
    println!("{:#?}", err);
}

async fn push_events_handler(
    event: SlackPushEvent,
    _client: Arc<SlackHyperClient>,
    bot_token: &SlackApiToken,
    new_descriptors_sender: Arc<Mutex<Sender<SlackDescriptor>>>,
    message_senders: Arc<Mutex<HashMap<String, Sender<SlackMessageContent>>>>,
) {
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
    new_descriptors_sender: Arc<Mutex<Sender<SlackDescriptor>>>,
    message_senders: &mut HashMap<String, Sender<SlackMessageContent>>,
) {
    let (sender, receiver) = crossbeam_channel::unbounded();
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

impl DescriptorManager for SlackDescriptorManager {
    fn block_until_descriptor(&self) -> Result<(), std::io::Error> {
        let mut s = Select::new();
        s.recv(&self.new_descriptors);
        s.ready();
        Ok(())
    }

    fn new_descriptor(
        &self,
    ) -> Result<Box<dyn Descriptor>, Box<dyn std::error::Error + Send + Sync>> {
        match self.new_descriptors.try_recv() {
            Ok(descriptor) => {
                /* TODO: why are we maintaining this hashmap?
                let ret = descriptor.identifier().clone();
                self.descriptors
                    .insert(ret.identifier.clone(), Box::new(descriptor));
                */
                Ok(Box::new(descriptor))
            }
            Err(e) => Err(Box::new(e)),
        }
    }
}

pub struct SlackDescriptor {
    slack_bot_token: SlackApiToken,
    input_channel: Arc<Mutex<Receiver<SlackMessageContent>>>,
    channel_id: SlackChannelId,
    chat_buffer: Vec<u8>,
    //identifier: descriptor::DescriptorId,
    hostname: String,
}

impl SlackDescriptor {
    pub fn new(
        channel_id: SlackChannelId,
        input_channel: Receiver<SlackMessageContent>,
        token: SlackApiToken,
    ) -> Self {
        /*
        let identifier = descriptor::DescriptorId {
            identifier: channel_id.to_string(),
            descriptor_type: "SLACK".to_owned(),
        };
            */
        let hostname = format!("SLACK:{}", channel_id);
        Self {
            slack_bot_token: token,
            input_channel: Arc::new(Mutex::new(input_channel)),
            channel_id,
            chat_buffer: Vec::new(),
            //identifier: identifier,
            hostname,
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
}

impl Descriptor for SlackDescriptor {
    fn get_hostname(&self) -> &str {
        self.hostname.as_str()
    }
}

impl Read for SlackDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let temp = self
            .input_channel
            .lock()
            .expect("Unable to get lock on input channel")
            .try_recv();
        match temp {
            Ok(content) => {
                match content.text {
                    Some(mut text) => {
                        text.push('\n'); // CircleMUD expects newline delimiters

                        // store the slack message in case it's too big for CircleMUD (ie buf.len())
                        // Currently unnecessary - CircleMUD errors if it exceeds its buffer gets
                        // filled by this.
                        self.chat_buffer.extend(text.as_bytes());
                        let common_length = std::cmp::min(self.chat_buffer.len(), buf.len());
                        buf[0..common_length].copy_from_slice(&self.chat_buffer[0..common_length]);
                        drop(self.chat_buffer.drain(..common_length)); // dropping drain removes the elements
                        Ok(common_length)
                    }
                    None => Ok(0),
                }
            }
            Err(TryRecvError::Empty) => Ok(0),
            Err(e) => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Unable to send message: {}", dbg!(e)),
            )),
        }
    }
}

impl Write for SlackDescriptor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to create local runtime");
        let content = String::from_utf8_lossy(buf);
        match runtime
            .block_on(self.send_message(SlackMessageContent::new().with_text(content.to_string())))
        {
            Ok(()) => Ok(buf.len()),
            Err(e) => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Unable to send message: {}", e),
            )),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        todo!()
    }
}
