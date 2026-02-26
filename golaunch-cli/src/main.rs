use clap::{Parser, Subcommand};
use golaunch_core::{Database, NewCommandHistory, NewItem, NewMemory, NewSlashCommand, UpdateItem};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "buddio-cli")]
#[command(about = "CLI tool for managing Buddio launcher items")]
#[command(version)]
struct Cli {
    /// Path to the database file (defaults to ~/.local/share/buddio/buddio.db)
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
enum Commands {
    /// Add a new item to the launcher
    Add {
        /// Item title (displayed in the launcher)
        #[arg(long)]
        title: String,

        /// Action type: 'command', 'url', or 'script'
        #[arg(long, default_value = "command")]
        action_type: String,

        /// The command, URL, or script to execute
        #[arg(long)]
        action_value: String,

        /// Subtitle / description
        #[arg(long)]
        subtitle: Option<String>,

        /// Icon (emoji or icon name)
        #[arg(long)]
        icon: Option<String>,

        /// Category for grouping
        #[arg(long)]
        category: Option<String>,

        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
    },

    /// Remove an item by ID
    Remove {
        /// The item ID to remove
        id: String,
    },

    /// List all items
    List {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Include disabled items
        #[arg(long)]
        all: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search items
    Search {
        /// Search query
        query: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Update an existing item
    Update {
        /// The item ID to update
        id: String,

        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        subtitle: Option<String>,

        #[arg(long)]
        icon: Option<String>,

        #[arg(long)]
        action_type: Option<String>,

        #[arg(long)]
        action_value: Option<String>,

        #[arg(long)]
        category: Option<String>,

        #[arg(long)]
        tags: Option<String>,

        /// Enable or disable the item
        #[arg(long)]
        enabled: Option<bool>,
    },

    /// Import items from a JSON file
    Import {
        /// Path to JSON file
        file: PathBuf,
    },

    /// Export all items as JSON
    Export {
        /// Output file path (defaults to stdout)
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Show database path
    DbPath,

    /// Manage memories
    Memory {
        #[command(subcommand)]
        action: MemoryCommands,
    },

    /// View command history
    History {
        /// Number of recent entries to show
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Search history for a query
        #[arg(long)]
        search: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Execute an item by ID
    Run {
        /// The item ID to execute
        id: String,
    },

    /// Manage agent conversations
    Conversations {
        #[command(subcommand)]
        action: ConversationCommands,
    },

    /// Manage slash commands
    SlashCommands {
        #[command(subcommand)]
        action: SlashCommandActions,
    },
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// Add a new memory
    Add {
        /// Memory key (e.g., "preferred_editor")
        #[arg(long)]
        key: String,

        /// Memory value (e.g., "vscode")
        #[arg(long)]
        value: String,

        /// Memory type: 'preference', 'pattern', or 'fact'
        #[arg(long, default_value = "fact")]
        r#type: String,

        /// Optional context (e.g., category or folder)
        #[arg(long)]
        context: Option<String>,

        /// Confidence (0.0 to 1.0)
        #[arg(long, default_value = "1.0")]
        confidence: f64,
    },

    /// List all memories
    List {
        /// Filter by memory type
        #[arg(long)]
        r#type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search memories
    Search {
        /// Search query
        query: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Remove a memory by ID
    Remove {
        /// Memory ID to remove
        id: String,
    },

    /// Get a memory by key
    Get {
        /// Memory key
        key: String,

        /// Optional context
        #[arg(long)]
        context: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConversationCommands {
    /// List recent conversations
    List {
        /// Maximum number of conversations to show
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search conversations
    Search {
        /// Search query
        query: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show a conversation with its messages
    Show {
        /// Conversation ID
        id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Get recent conversation context (formatted summary for agent use)
    Context {
        /// Number of recent conversations to include
        #[arg(long, default_value = "5")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum SlashCommandActions {
    /// List all slash commands
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Add (register) a new slash command
    Add {
        /// Command name (without the leading /)
        #[arg(long)]
        name: String,

        /// Description of what the command does
        #[arg(long, default_value = "")]
        description: String,

        /// Path to the script file
        #[arg(long)]
        script_path: String,
    },

    /// Remove a slash command by name
    Remove {
        /// Command name to remove
        #[arg(long)]
        name: String,
    },

    /// Run a slash command by name with arguments
    Run {
        /// Command name (without the leading /)
        #[arg(long)]
        name: String,

        /// Arguments to pass to the script
        #[arg(long, default_value = "")]
        args: String,
    },

    /// Get a slash command by name (JSON output)
    Get {
        /// Command name
        #[arg(long)]
        name: String,
    },
}

fn get_db(db_path: Option<PathBuf>) -> Result<Database, String> {
    match db_path {
        Some(path) => Database::with_path(&path),
        None => Database::new(),
    }
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Add {
            title,
            action_type,
            action_value,
            subtitle,
            icon,
            category,
            tags,
        } => {
            let db = get_db(cli.db)?;
            let item = db.add_item(NewItem {
                title,
                subtitle,
                icon,
                action_type,
                action_value,
                category,
                tags,
            })?;
            println!("{}", serde_json::to_string_pretty(&item).unwrap());
            Ok(())
        }

        Commands::Remove { id } => {
            let db = get_db(cli.db)?;
            if db.remove_item(&id)? {
                println!("Item {id} removed successfully");
            } else {
                eprintln!("Item {id} not found");
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::List {
            category,
            all,
            json,
        } => {
            let db = get_db(cli.db)?;
            let items = db.list_items(category.as_deref(), all)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&items).unwrap());
            } else {
                if items.is_empty() {
                    println!("No items found");
                    return Ok(());
                }
                let header = format!(
                    "{:<38} {:<25} {:<10} {:<10} {}",
                    "ID", "TITLE", "TYPE", "CATEGORY", "ACTION"
                );
                println!("{header}");
                println!("{}", "-".repeat(100));
                for item in &items {
                    let action_display = if item.action_value.len() > 30 {
                        format!("{}...", &item.action_value[..27])
                    } else {
                        item.action_value.clone()
                    };
                    println!(
                        "{:<38} {:<25} {:<10} {:<10} {}",
                        item.id, item.title, item.action_type, item.category, action_display
                    );
                }
                println!("\nTotal: {} items", items.len());
            }
            Ok(())
        }

        Commands::Search { query, json } => {
            let db = get_db(cli.db)?;
            let items = db.search_items(&query)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&items).unwrap());
            } else {
                if items.is_empty() {
                    println!("No items matching '{query}'");
                    return Ok(());
                }
                for item in &items {
                    let icon = item.icon.as_deref().unwrap_or("  ");
                    let subtitle = item
                        .subtitle
                        .as_deref()
                        .map(|s| format!(" - {s}"))
                        .unwrap_or_default();
                    println!(
                        "{} {} [{}]{} ({})",
                        icon, item.title, item.action_type, subtitle, item.id
                    );
                }
            }
            Ok(())
        }

        Commands::Update {
            id,
            title,
            subtitle,
            icon,
            action_type,
            action_value,
            category,
            tags,
            enabled,
        } => {
            let db = get_db(cli.db)?;
            let item = db.update_item(
                &id,
                UpdateItem {
                    title,
                    subtitle,
                    icon,
                    action_type,
                    action_value,
                    category,
                    tags,
                    enabled,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&item).unwrap());
            Ok(())
        }

        Commands::Import { file } => {
            let db = get_db(cli.db)?;
            let content = std::fs::read_to_string(&file)
                .map_err(|e| format!("Failed to read file {}: {e}", file.display()))?;
            let items: Vec<NewItem> =
                serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {e}"))?;
            let count = items.len();
            let imported = db.import_items(items)?;
            println!("Successfully imported {count} items");
            println!("{}", serde_json::to_string_pretty(&imported).unwrap());
            Ok(())
        }

        Commands::Export { output } => {
            let db = get_db(cli.db)?;
            let items = db.export_items()?;
            let json = serde_json::to_string_pretty(&items).unwrap();

            if let Some(path) = output {
                std::fs::write(&path, &json)
                    .map_err(|e| format!("Failed to write to {}: {e}", path.display()))?;
                println!("Exported {} items to {}", items.len(), path.display());
            } else {
                println!("{json}");
            }
            Ok(())
        }

        Commands::DbPath => {
            match cli.db {
                Some(path) => println!("{}", path.display()),
                None => println!("{}", Database::db_path()?.display()),
            }
            Ok(())
        }

        Commands::Memory { action } => {
            let db = get_db(cli.db)?;
            match action {
                MemoryCommands::Add {
                    key,
                    value,
                    r#type,
                    context,
                    confidence,
                } => {
                    let mem = db.add_memory(NewMemory {
                        key,
                        value,
                        context,
                        memory_type: Some(r#type),
                        confidence: Some(confidence),
                    })?;
                    println!("{}", serde_json::to_string_pretty(&mem).unwrap());
                }
                MemoryCommands::List { r#type, json } => {
                    let memories = db.list_memories(r#type.as_deref())?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&memories).unwrap());
                    } else if memories.is_empty() {
                        println!("No memories found");
                    } else {
                        let header = format!(
                            "{:<38} {:<20} {:<30} {:<12} {:<6}",
                            "ID", "KEY", "VALUE", "TYPE", "CONF"
                        );
                        println!("{header}");
                        println!("{}", "-".repeat(106));
                        for mem in &memories {
                            let value_display = if mem.value.len() > 28 {
                                format!("{}...", &mem.value[..25])
                            } else {
                                mem.value.clone()
                            };
                            println!(
                                "{:<38} {:<20} {:<30} {:<12} {:.1}",
                                mem.id, mem.key, value_display, mem.memory_type, mem.confidence
                            );
                        }
                        println!("\nTotal: {} memories", memories.len());
                    }
                }
                MemoryCommands::Search { query, json } => {
                    let memories = db.search_memories(&query)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&memories).unwrap());
                    } else if memories.is_empty() {
                        println!("No memories matching '{query}'");
                    } else {
                        for mem in &memories {
                            let context_display = mem
                                .context
                                .as_deref()
                                .map(|c| format!(" [ctx: {c}]"))
                                .unwrap_or_default();
                            println!(
                                "{} = {} ({}){} ({})",
                                mem.key, mem.value, mem.memory_type, context_display, mem.id
                            );
                        }
                    }
                }
                MemoryCommands::Remove { id } => {
                    if db.remove_memory(&id)? {
                        println!("Memory {id} removed successfully");
                    } else {
                        eprintln!("Memory {id} not found");
                        std::process::exit(1);
                    }
                }
                MemoryCommands::Get { key, context } => {
                    let mem = db.get_memory_by_key(&key, context.as_deref())?;
                    println!("{}", serde_json::to_string_pretty(&mem).unwrap());
                }
            }
            Ok(())
        }

        Commands::History {
            limit,
            search,
            json,
        } => {
            let db = get_db(cli.db)?;
            let entries = match search {
                Some(query) => db.search_command_history(&query)?,
                None => db.get_recent_commands(limit)?,
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&entries).unwrap());
            } else if entries.is_empty() {
                println!("No command history found");
            } else {
                let header = format!(
                    "{:<38} {:<30} {:<10} {:<20} {}",
                    "ID", "COMMAND", "TYPE", "EXECUTED AT", "SOURCE"
                );
                println!("{header}");
                println!("{}", "-".repeat(108));
                for entry in &entries {
                    let cmd_display = if entry.command_text.len() > 28 {
                        format!("{}...", &entry.command_text[..25])
                    } else {
                        entry.command_text.clone()
                    };
                    println!(
                        "{:<38} {:<30} {:<10} {:<20} {}",
                        entry.id, cmd_display, entry.action_type, entry.executed_at, entry.source
                    );
                }
                println!("\nTotal: {} entries", entries.len());
            }
            Ok(())
        }
        Commands::Run { id } => {
            let db = get_db(cli.db)?;
            let item = db.get_item(&id)?;
            db.increment_frequency(&id)?;
            let _ = db.record_command(NewCommandHistory {
                item_id: Some(id.clone()),
                command_text: item.action_value.clone(),
                action_type: item.action_type.clone(),
                source: Some("cli".to_string()),
            });

            match item.action_type.as_str() {
                "url" => {
                    open::that(&item.action_value)
                        .map_err(|e| format!("Failed to open URL: {e}"))?;
                }
                "command" | "script" => {
                    #[cfg(target_os = "windows")]
                    {
                        std::process::Command::new("cmd")
                            .args(["/C", &item.action_value])
                            .spawn()
                            .map_err(|e| format!("Failed to execute {}: {e}", item.action_type))?;
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        std::process::Command::new("sh")
                            .args(["-c", &item.action_value])
                            .spawn()
                            .map_err(|e| format!("Failed to execute {}: {e}", item.action_type))?;
                    }
                }
                other => return Err(format!("Unknown action type: {other}")),
            }

            println!("Executed item {} ({})", item.title, id);
            Ok(())
        }

        Commands::Conversations { action } => {
            let db = get_db(cli.db)?;
            match action {
                ConversationCommands::List { limit, json } => {
                    let conversations = db.list_conversations(limit)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&conversations).unwrap());
                    } else if conversations.is_empty() {
                        println!("No conversations found");
                    } else {
                        for conv in &conversations {
                            let preview = conv.last_message_preview.as_deref().unwrap_or("(empty)");
                            let preview_display = if preview.len() > 60 {
                                format!("{}...", &preview[..57])
                            } else {
                                preview.to_string()
                            };
                            println!(
                                "[{}] {} ({} msgs, {})\n  {}",
                                &conv.id[..8],
                                conv.title,
                                conv.message_count,
                                conv.updated_at,
                                preview_display
                            );
                        }
                        println!("\nTotal: {} conversations", conversations.len());
                    }
                }
                ConversationCommands::Search { query, json } => {
                    let conversations = db.search_conversations(&query)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&conversations).unwrap());
                    } else if conversations.is_empty() {
                        println!("No conversations matching '{query}'");
                    } else {
                        for conv in &conversations {
                            let preview = conv.last_message_preview.as_deref().unwrap_or("(empty)");
                            let preview_display = if preview.len() > 60 {
                                format!("{}...", &preview[..57])
                            } else {
                                preview.to_string()
                            };
                            println!(
                                "[{}] {} ({} msgs)\n  {}",
                                &conv.id[..8],
                                conv.title,
                                conv.message_count,
                                preview_display
                            );
                        }
                    }
                }
                ConversationCommands::Show { id, json } => {
                    let messages = db.get_conversation_messages(&id)?;
                    if json {
                        let conv = db.get_conversation(&id)?;
                        let output = serde_json::json!({
                            "conversation": conv,
                            "messages": messages,
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    } else if messages.is_empty() {
                        println!("No messages in conversation {id}");
                    } else {
                        let conv = db.get_conversation(&id)?;
                        println!("=== {} ===\n", conv.title);
                        for msg in &messages {
                            let role_label = match msg.role.as_str() {
                                "user" => "You",
                                "assistant" => "Agent",
                                other => other,
                            };
                            println!("[{} - {}]\n{}\n", role_label, msg.created_at, msg.content);
                        }
                    }
                }
                ConversationCommands::Context { limit } => {
                    let context = db.get_recent_conversation_context(limit)?;
                    if context.is_empty() {
                        println!("No recent conversations.");
                    } else {
                        for (conv, messages) in &context {
                            println!("--- Conversation: {} (ID: {}) ---", conv.title, conv.id);
                            println!("    Updated: {}", conv.updated_at);
                            for msg in messages {
                                let role_label = match msg.role.as_str() {
                                    "user" => "User",
                                    "assistant" => "Assistant",
                                    other => other,
                                };
                                let content_display = if msg.content.len() > 200 {
                                    format!("{}...", &msg.content[..197])
                                } else {
                                    msg.content.clone()
                                };
                                println!("  [{}]: {}", role_label, content_display);
                            }
                            println!();
                        }
                    }
                }
            }
            Ok(())
        }

        Commands::SlashCommands { action } => {
            let db = get_db(cli.db)?;
            match action {
                SlashCommandActions::List { json } => {
                    let commands = db.list_slash_commands()?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&commands).unwrap());
                    } else if commands.is_empty() {
                        println!("No slash commands found");
                    } else {
                        let header = format!(
                            "{:<38} {:<15} {:<30} {:<6}",
                            "ID", "NAME", "DESCRIPTION", "USED"
                        );
                        println!("{header}");
                        println!("{}", "-".repeat(89));
                        for cmd in &commands {
                            let desc = if cmd.description.len() > 28 {
                                format!("{}...", &cmd.description[..25])
                            } else {
                                cmd.description.clone()
                            };
                            println!(
                                "{:<38} {:<15} {:<30} {:<6}",
                                cmd.id, cmd.name, desc, cmd.usage_count
                            );
                        }
                        println!("\nTotal: {} commands", commands.len());
                    }
                }
                SlashCommandActions::Add {
                    name,
                    description,
                    script_path,
                } => {
                    let cmd = db.add_slash_command(NewSlashCommand {
                        name,
                        description,
                        script_path,
                    })?;
                    println!("{}", serde_json::to_string_pretty(&cmd).unwrap());
                }
                SlashCommandActions::Remove { name } => {
                    if db.remove_slash_command_by_name(&name)? {
                        println!("Slash command '/{name}' removed successfully");
                    } else {
                        eprintln!("Slash command '/{name}' not found");
                        std::process::exit(1);
                    }
                }
                SlashCommandActions::Run { name, args } => {
                    let cmd = db.get_slash_command_by_name(&name)?;
                    db.increment_slash_command_usage(&cmd.id)?;

                    let _ = db.record_command(NewCommandHistory {
                        item_id: None,
                        command_text: format!("/{} {}", name, args),
                        action_type: "slash_command".to_string(),
                        source: Some("cli".to_string()),
                    });

                    #[cfg(target_os = "windows")]
                    let output = {
                        std::process::Command::new("powershell")
                            .args(["-ExecutionPolicy", "Bypass", "-File", &cmd.script_path])
                            .args(args.split_whitespace())
                            .output()
                            .map_err(|e| format!("Failed to execute script: {e}"))?
                    };
                    #[cfg(not(target_os = "windows"))]
                    let output = {
                        std::process::Command::new("sh")
                            .arg(&cmd.script_path)
                            .args(args.split_whitespace())
                            .output()
                            .map_err(|e| format!("Failed to execute script: {e}"))?
                    };

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stdout.is_empty() {
                        print!("{stdout}");
                    }
                    if !stderr.is_empty() {
                        eprint!("{stderr}");
                    }
                    if !output.status.success() {
                        std::process::exit(output.status.code().unwrap_or(1));
                    }
                }
                SlashCommandActions::Get { name } => {
                    let cmd = db.get_slash_command_by_name(&name)?;
                    println!("{}", serde_json::to_string_pretty(&cmd).unwrap());
                }
            }
            Ok(())
        }
    }
}
