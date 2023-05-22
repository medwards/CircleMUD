# CircleMUD (with Rust)

CircleMUD (with Rust) is CircleMUD with a modern toolchain and player input technology. It aims to abstract away 90s-era Telnet socket management and replace it with a swappable I/O mechanism. Supported methods (will) include: traditional Telnet (for legacy support), direct bytestreams (for testing), and Slack.

CircleMUD (with Rust) is also a useful demonstration of how to refactor legacy C code to support modern use-cases. Much of CircleMUD uses code and practices which were advanced for their time but today hold the codebase back from supporting new features. Furthermore it mixes game logic, IO features, and primitive IO indiscriminately making it difficult to change any of these layers without signficant research. The history of this repository shows the first steps to both a "rewrite it in Rust" project and a general refactoring of legacy code.

The original CircleMUD README is still available as README.old

## Building

1. (optional) `autoconf -I cnf -o configure cnf/configure.in`
2. `./configure`
3. `make -C src`

## Testing

TBD

## Running

`./bin/circle`

(Note: this will run reading and writing `lib/`, if you want a "clean" world then copy `lib` somewhere else and use `./bin/circle -d /tmp/otherlocation/lib`)
