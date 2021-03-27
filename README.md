# CircleMUD 3.1-modern

An update to CircleMUD 3.1 to enable easier integration with new frontends. Included is integration with Slack and a trait interface that allows implementing other integration transparently to the legacy CircleMUD codebase.

See `README` for the original instructions.

## Building

In addition to normal CircleMUD build dependencies you will need Rust installed. The `configure` script has been updated to look for `cargo`. Alternative platforms and build tools were untested (ie `Makefile.os2` or `Makefile.msvc` should not work).

```
./configure
cd src
make
```

## Running

Copy `lib` and `bin` to a server that is accessible over the internet. You do not require this if you are exclusively using telnet.

```
SLACK_SIGNING_SECRET=123 SLACK_BOT_TOKEN=xoxb-123 ./bin/circle
```

See `Slack Integration` for help on getting the secrets/tokens.

### Slack Integration

You will need to install an App into your Slack workspace:

1. [Create an App](https://api.slack.com/apps?new_app)
2. Add the `chat:write` and `im:read` OAuth Scopes under *OAuth & Permissions*
3. *Install App* into your workspace
4. `SLACK_BOT_TOKEN` comes from *Bot User OAuth token* (visible under *Install App* or *OAuth & Permissions*)
5. `SLACK_SIGNING_SECRET` Store the values from *Basic Information*/*App Credentials*/*Signing Secret*
7. Launch the server with all the environment variables set (`SLACK_SIGNING_SECRET=123 SLACK_BOT_TOKEN=xoxb-123 ./bin/circle`)
8. *Enable Events* in *Event Subscriptions* using `http[s]://YOUR_DOMAIN_OR_IP:8000/push`
9. *Subscribe to bot events* `message.im` or `message.channels` in *Event Subscriptions* (Watch for the *Save Changes* banner on the bottom of your screen, its easy to miss. Until you click *Save Changes* you will not receive events).


You can force the server address and port with the `SLACK_SOCKET_ADDR` environment variable, ie `SLACK_SOCKET_ADDR=0.0.0.0:80`. Otherwise this defaults to `127.0.0.1:8000`. You will need to provide a different address in Step 8.


## Future

* Integration tests
* Discord integration
* Telnet refactor (move all telnet code to the Rust side and remove related functionality from C)
