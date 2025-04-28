```
  ███████╗ █████╗ ███╗   ███╗██╗   ██╗███████╗
  ██╔════╝██╔══██╗████╗ ████║██║   ██║██╔════╝
  ███████╗███████║██╔████╔██║██║   ██║███████╗
  ╚════██║██╔══██║██║╚██╔╝██║██║   ██║╚════██║
  ███████║██║  ██║██║ ╚═╝ ██║╚██████╔╝███████║
  ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝ ╚═════╝ ╚══════╝
```

# Samus TUI: Advanced Terminal Interface for AI Interaction

Samus is a terminal-based user interface that lets you interact with AI models using a clean, intuitive interface right in your terminal. Built with Rust, it provides a lightning-fast and responsive experience while giving you the power of modern AI at your fingertips.

## Features

- **Beautiful Terminal UI**: Enjoy a visually appealing interface with colors, styling, and clean layouts
- **AI Chat Interface**: Talk directly to AI models through OpenRouter
- **Command Support**: Use slash commands for quick actions like `/help`, `/model`, and more
- **Multi-View Interface**: Switch between different views including chat, search, and file operations

## Getting Started

### Requirements

- Rust 1.70.0 or later
- An OpenRouter API key

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/samus.git
   cd samus
   ```

2. Create a `.env` file in the project root with your OpenRouter API key:
   ```
   OPEN_ROUTER_API_KEY=your-api-key-here
   ```

3. Build and run:
   ```bash
   cargo build
   cargo run
   ```

## Using Samus

### Basic Commands

- **Chat**: Just type your message and press Enter to talk to the AI
- **Slash Commands**:
  - `/help`: Show available commands
  - `/model haiku`: Switch to Claude 3.5 Haiku
  - `/model sonnet`: Switch to Claude 3 Sonnet
  - `/model opus`: Switch to Claude 3 Opus
  - `/quit`: Exit the application

### Keyboard Shortcuts

- **Ctrl+Q**: Quit the application
- **Enter**: Send message
- **Shift+Enter**: Add a new line in your message
- **Up/Down arrows**: Navigate through command history

## Configuration

You can set up your OpenRouter API key in two ways:

1. Add it to your `.env` file as `OPEN_ROUTER_API_KEY=your-key-here`
2. Use the command `/config your-key-here` in the application

## Customization

Samus is built with a modular architecture that allows for extensive customization. Check out the `src/ui` directory to modify the interface components.

## License

Apache 2.0

---
