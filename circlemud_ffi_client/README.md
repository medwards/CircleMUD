# `circlemud_ffi_client`

Static library built into Circlemud to provide new clients.

`ByteStreamDescriptor` supports any input/output streams that implement `Read` or `Write` (useful for stdin/stdout based clients or for testing with arbitrary buffers).

`SlackBotDescriptor` supports integration with a Slack bot.

## Setting up a Slack Bot

Set up a new app in slack https://api.slack.com/bot-users#creating-bot-user

Add scopes? https://api.slack.com/apps/YOURAPPID/oauth
I have
chat:write
im:history
im:read
im:write

Enable Event Subscriptions https://api.slack.com/apps/YOURAPPID/event-subscriptions?
add message.im
