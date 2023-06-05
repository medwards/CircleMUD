# CircleMUD (with Rust)

CircleMUD (with Rust) is CircleMUD with a modern toolchain and player input technology. It aims to abstract away 90s-era Telnet socket management and replace it with a swappable I/O mechanism. Supported methods (will) include: traditional Telnet (for legacy support), direct bytestreams (for testing), and Slack.

CircleMUD (with Rust) is also a useful demonstration of how to refactor legacy C code to support modern use-cases. Much of CircleMUD uses code and practices which were advanced for their time but today hold the codebase back from supporting new features. Furthermore it mixes game logic, IO features, and primitive IO indiscriminately making it difficult to change any of these layers without signficant research. The history of this repository shows the first steps to both a "rewrite it in Rust" project and a general refactoring of legacy code.

The original CircleMUD README is still available as README.old

## Building

1. (optional) `autoconf -I cnf -o configure cnf/configure.in`
2. `./configure`
3. `make -C src`

## Testing

1. `cargo test --manifest-dir mud-comms`

## Running

`./bin/circle`

(Note: this will run reading and writing `lib/`, if you want a "clean" world then copy `lib` somewhere else and use `./bin/circle -d /tmp/otherlocation/lib`)

Different communication backends (ie socket/telnet server vs Slack) are selected in `mud_comms::new_descriptor_manager` and require rebuilding.

### Slack

#### Setup

You will need to install an App into your Slack workspace:

1. [Create an App](https://api.slack.com/apps?new_app)
2. Add the `chat:write` and `im:read` OAuth Scopes under *OAuth & Permissions*
3. *Install App* into your workspace
4. `SLACK_BOT_USER_OAUTH_TOKEN` comes from *Bot User OAuth token* (visible under *Install App* or *OAuth & Permissions*)
5. `SLACK_SIGNING_SECRET` Store the values from *Basic Information*/*App Credentials*/*Signing Secret*
7. Launch the server with all the environment variables set (`SLACK_SIGNING_SECRET=123 SLACK_BOT_TOKEN=xoxb-123 ./bin/circle`)
8. *Enable Events* in *Event Subscriptions* using `http[s]://YOUR_DOMAIN_OR_IP:8000/push`
9. *Subscribe to bot events* `message.im` or `message.channels` in *Event Subscriptions* (Watch for the *Save Changes* banner on the bottom of your screen, its easy to miss. Until you click *Save Changes* you will not receive events).

#### Environment

`SLACK_BOT_USER_OAUTH_TOKEN` and `SLACK_SIGNING_SECRET` are required environment variables. `SLACK_SOCKET_ADDR` is recommended. Make sure it serves the Events URL from Setup step 8.
