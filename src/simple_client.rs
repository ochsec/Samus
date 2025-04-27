use dotenv::dotenv;
use std::io::{self, Write};

use crate::config::McpServerConfig;
use crate::mcp::client::OpenRouterClient;
use tokio::runtime::Runtime;

/// A simple CLI client for testing OpenRouter connection
pub async fn run_simple_client() -> Result<(), Box<dyn std::error::Error>> {
    println!("Simple OpenRouter Client");
    println!("========================");

    // Configure OpenRouter
    let api_key = match std::env::var("OPEN_ROUTER_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("OpenRouter API key not found in environment.");
            println!("Please enter your API key:");
            print!("> ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    // Create config
    let config = McpServerConfig {
        id: "openrouter".to_string(),
        name: "OpenRouter".to_string(),
        url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
        api_key: Some(api_key),
        enabled: true,
    };

    // Initialize client
    let client = match OpenRouterClient::new(config, "anthropic/claude-3-haiku".to_string()) {
        Ok(client) => {
            println!("OpenRouter client configured successfully with Claude 3.5 Haiku.");
            client
        }
        Err(e) => {
            println!("Error configuring OpenRouter client: {}", e);
            return Err(Box::new(e));
        }
    };

    // Chat loop
    println!("\nEnter messages to chat with Claude 3.5 Haiku (type 'exit' to quit):");
    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            break;
        }

        if input.trim().is_empty() {
            println!("Input cannot be empty. Please try again.");
            continue;
        }

        println!("Sending message to OpenRouter...");
        match client.chat(input.to_string()).await {
            Ok(response) => {
                println!("\nResponse:");
                println!("=========");
                println!("{}", response);
                println!("=========\n");
            }
            Err(e) => println!("Error: {}", e),
        }
    }

    println!("Goodbye!");
    Ok(())
}
