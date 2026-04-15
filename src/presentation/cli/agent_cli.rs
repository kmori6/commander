use crate::application::port::llm_client::LlmClient;
use crate::application::usecase::agent_usecase::{AgentEvent, AgentUsecase, HandleAgentInput};
use crate::presentation::error::agent_cli_error::AgentCliError;
use std::io::{Write, stdin, stdout};

pub async fn run<L: LlmClient>(usecase: &AgentUsecase<L>) -> Result<(), AgentCliError> {
    println!("Agent mock CLI");
    println!("type /help for commands");

    loop {
        let Some(line) = read_line("agent > ")? else {
            println!();
            break;
        };

        let input = line.trim();

        if input.is_empty() {
            continue;
        }

        match input {
            "/help" => print_help(),
            "/reset" => println!("mock session reset"),
            "/exit" | "/quit" => break,
            _ if input.starts_with('/') => println!("unknown command: {input}"),
            _ => {
                let output = usecase
                    .handle(HandleAgentInput {
                        user_input: input.to_string(),
                    })
                    .await?;

                for event in output.reply {
                    match event {
                        AgentEvent::AssistantMessage(message) => println!("{message}"),
                    }
                }
            }
        }
    }

    Ok(())
}

fn read_line(prompt: &str) -> Result<Option<String>, AgentCliError> {
    print!("{prompt}");
    stdout().flush()?;

    let mut buf = String::new();
    let bytes_read = stdin().read_line(&mut buf)?;

    if bytes_read == 0 {
        return Ok(None);
    }

    Ok(Some(buf))
}

fn print_help() {
    println!("/help  show help");
    println!("/reset reset mock session");
    println!("/exit  quit");
}
