# candle-watcher-rs

Proof-of-Concept for watching candles from the Coinbase Advanced API WebSocket using [cbadv-rs](https://github.com/Ohkthx/cbadv-rs). Candles are considered "complete" and "ejected" once a candle comes in with a more recent `start` timestamp.

#### Important Note

At the current time, candles that are received use 5 minute granularity. This cannot be currently changed within the API for smaller or larger granularities. To achieve other granularities, the REST API would be needed to poll the API for changes instead of using a WebSocket.

## Purpose

This is template code for storing candle data in a database and/or techical analysis using [tatk-rs](https://github.com/Ohkthx/tatk-rs). If a viable and clean way to achieve this then it will be implemented into the [cbadv-rs](https://github.com/Ohkthx/cbadv-rs) porject.