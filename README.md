# Repeater

Repeater is a versatile Rust library designed to serve as a foundational template for building real-time applications. Whether you’re creating a chat application, a collaborative tool, or any other system that requires real-time communication, Repeater provides the necessary building blocks to get started quickly.

## Features
- **WebSocket Communication**: Efficiently handle multiple WebSocket connections.
- **Command-Based Architecture**: Easily extendable command system for processing various types of messages.
- **Client Management**: Simplified management of client connections and message routing.
- **Modular Design**: Organized structure with separate modules for easy customization and scalability.
- **Tokio Runtime**: Built on top of the Tokio asynchronous runtime, ensuring high performance and scalability.

## Example
Here's a basic example of how you might use Repeater:

```rust
use repeater::start_server;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    start_server(addr).await;
}
```

## Contributing
This project is still under development, and contributions are welcome! Feel free to open issues or submit pull requests.

## License
Repeater is licensed under the MIT License. See [LICENSE](./LICENSE) for more information.
