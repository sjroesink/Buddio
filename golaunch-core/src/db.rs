use crate::models::{
    CommandHistory, CommandSuggestion, Conversation, ConversationMessage, ConversationWithPreview,
    Item, Memory, NewCommandHistory, NewConversation, NewConversationMessage, NewItem, NewMemory,
    NewSlashCommand, NewSlashCommandParam, Setting, SlashCommand, SlashCommandParam, UpdateItem,
};
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::PathBuf;
use uuid::Uuid;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self, String> {
        let db_path = Self::db_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create database directory: {e}"))?;
            // Also create the slash-commands directory alongside the database
            let slash_dir = parent.join("slash-commands");
            std::fs::create_dir_all(&slash_dir)
                .map_err(|e| format!("Failed to create slash-commands directory: {e}"))?;
        }
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database at {}: {e}", db_path.display()))?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    pub fn with_path(path: &PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create database directory: {e}"))?;
        }
        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open database at {}: {e}", path.display()))?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    pub fn db_path() -> Result<PathBuf, String> {
        let data_dir = dirs::data_local_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| "Cannot determine home directory".to_string())?;
        Ok(data_dir.join("golaunch").join("golaunch.db"))
    }

    pub fn slash_commands_dir() -> Result<PathBuf, String> {
        let data_dir = dirs::data_local_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| "Cannot determine home directory".to_string())?;
        Ok(data_dir.join("golaunch").join("slash-commands"))
    }

    fn initialize(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS items (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    subtitle TEXT,
                    icon TEXT,
                    action_type TEXT NOT NULL DEFAULT 'command',
                    action_value TEXT NOT NULL,
                    category TEXT NOT NULL DEFAULT 'General',
                    tags TEXT NOT NULL DEFAULT '',
                    frequency INTEGER NOT NULL DEFAULT 0,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_items_title ON items(title);
                CREATE INDEX IF NOT EXISTS idx_items_category ON items(category);
                CREATE INDEX IF NOT EXISTS idx_items_enabled ON items(enabled);

                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS command_history (
                    id TEXT PRIMARY KEY,
                    item_id TEXT,
                    command_text TEXT NOT NULL,
                    action_type TEXT NOT NULL DEFAULT 'command',
                    executed_at TEXT NOT NULL DEFAULT (datetime('now')),
                    source TEXT NOT NULL DEFAULT 'launcher'
                );
                CREATE INDEX IF NOT EXISTS idx_command_history_command ON command_history(command_text);
                CREATE INDEX IF NOT EXISTS idx_command_history_executed_at ON command_history(executed_at);

                CREATE TABLE IF NOT EXISTS memory (
                    id TEXT PRIMARY KEY,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    context TEXT,
                    memory_type TEXT NOT NULL DEFAULT 'fact',
                    confidence REAL NOT NULL DEFAULT 1.0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                    last_accessed TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_memory_key ON memory(key);
                CREATE INDEX IF NOT EXISTS idx_memory_type ON memory(memory_type);

                CREATE TABLE IF NOT EXISTS conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at);

                CREATE TABLE IF NOT EXISTS conversation_messages (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id),
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_conv_messages_conv_id ON conversation_messages(conversation_id);

                CREATE TABLE IF NOT EXISTS slash_commands (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    description TEXT NOT NULL DEFAULT '',
                    script_path TEXT NOT NULL,
                    usage_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_slash_commands_name ON slash_commands(name);
                CREATE INDEX IF NOT EXISTS idx_slash_commands_usage ON slash_commands(usage_count);

                CREATE TABLE IF NOT EXISTS slash_command_params (
                    id TEXT PRIMARY KEY,
                    command_id TEXT NOT NULL REFERENCES slash_commands(id),
                    name TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    position INTEGER NOT NULL,
                    required INTEGER NOT NULL DEFAULT 1
                );
                CREATE INDEX IF NOT EXISTS idx_slash_cmd_params_command_id ON slash_command_params(command_id);
                ",
            )
            .map_err(|e| format!("Failed to initialize database: {e}"))
    }

    pub fn add_item(&self, item: NewItem) -> Result<Item, String> {
        let id = Uuid::new_v4().to_string();
        let category = item.category.unwrap_or_else(|| "General".to_string());
        let tags = item.tags.unwrap_or_default();

        self.conn
            .execute(
                "INSERT INTO items (id, title, subtitle, icon, action_type, action_value, category, tags)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    item.title,
                    item.subtitle,
                    item.icon,
                    item.action_type,
                    item.action_value,
                    category,
                    tags,
                ],
            )
            .map_err(|e| format!("Failed to add item: {e}"))?;

        self.get_item(&id)
    }

    pub fn get_item(&self, id: &str) -> Result<Item, String> {
        self.conn
            .query_row(
                "SELECT id, title, subtitle, icon, action_type, action_value, category, tags, frequency, enabled, created_at, updated_at FROM items WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Item {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        subtitle: row.get(2)?,
                        icon: row.get(3)?,
                        action_type: row.get(4)?,
                        action_value: row.get(5)?,
                        category: row.get(6)?,
                        tags: row.get(7)?,
                        frequency: row.get(8)?,
                        enabled: row.get::<_, i64>(9)? != 0,
                        created_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    })
                },
            )
            .map_err(|e| format!("Item not found: {e}"))
    }

    pub fn update_item(&self, id: &str, update: UpdateItem) -> Result<Item, String> {
        let current = self.get_item(id)?;

        let title = update.title.unwrap_or(current.title);
        let subtitle = update.subtitle.or(current.subtitle);
        let icon = update.icon.or(current.icon);
        let action_type = update.action_type.unwrap_or(current.action_type);
        let action_value = update.action_value.unwrap_or(current.action_value);
        let category = update.category.unwrap_or(current.category);
        let tags = update.tags.unwrap_or(current.tags);
        let enabled = update.enabled.unwrap_or(current.enabled);

        self.conn
            .execute(
                "UPDATE items SET title = ?1, subtitle = ?2, icon = ?3, action_type = ?4, action_value = ?5, category = ?6, tags = ?7, enabled = ?8, updated_at = datetime('now') WHERE id = ?9",
                params![title, subtitle, icon, action_type, action_value, category, tags, enabled as i64, id],
            )
            .map_err(|e| format!("Failed to update item: {e}"))?;

        self.get_item(id)
    }

    pub fn remove_item(&self, id: &str) -> Result<bool, String> {
        let rows = self
            .conn
            .execute("DELETE FROM items WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to remove item: {e}"))?;
        Ok(rows > 0)
    }

    pub fn search_items(&self, query: &str) -> Result<Vec<Item>, String> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, subtitle, icon, action_type, action_value, category, tags, frequency, enabled, created_at, updated_at
                 FROM items
                 WHERE enabled = 1 AND (title LIKE ?1 OR subtitle LIKE ?1 OR tags LIKE ?1 OR category LIKE ?1)
                 ORDER BY frequency DESC, title ASC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let items = stmt
            .query_map(params![pattern], |row| {
                Ok(Item {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    subtitle: row.get(2)?,
                    icon: row.get(3)?,
                    action_type: row.get(4)?,
                    action_value: row.get(5)?,
                    category: row.get(6)?,
                    tags: row.get(7)?,
                    frequency: row.get(8)?,
                    enabled: row.get::<_, i64>(9)? != 0,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<Item>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(items)
    }

    pub fn list_items(
        &self,
        category: Option<&str>,
        include_disabled: bool,
    ) -> Result<Vec<Item>, String> {
        let sql = match (category, include_disabled) {
            (Some(_), false) => {
                "SELECT id, title, subtitle, icon, action_type, action_value, category, tags, frequency, enabled, created_at, updated_at
                 FROM items WHERE category = ?1 AND enabled = 1 ORDER BY frequency DESC, title ASC"
            }
            (Some(_), true) => {
                "SELECT id, title, subtitle, icon, action_type, action_value, category, tags, frequency, enabled, created_at, updated_at
                 FROM items WHERE category = ?1 ORDER BY frequency DESC, title ASC"
            }
            (None, false) => {
                "SELECT id, title, subtitle, icon, action_type, action_value, category, tags, frequency, enabled, created_at, updated_at
                 FROM items WHERE enabled = 1 ORDER BY frequency DESC, title ASC"
            }
            (None, true) => {
                "SELECT id, title, subtitle, icon, action_type, action_value, category, tags, frequency, enabled, created_at, updated_at
                 FROM items ORDER BY frequency DESC, title ASC"
            }
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let items = if let Some(cat) = category {
            stmt.query_map(params![cat], Self::row_to_item)
        } else {
            stmt.query_map([], Self::row_to_item)
        }
        .map_err(|e| format!("Failed to execute query: {e}"))?
        .collect::<SqlResult<Vec<Item>>>()
        .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(items)
    }

    pub fn get_categories(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT category FROM items WHERE enabled = 1 ORDER BY category ASC")
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let categories = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<String>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(categories)
    }

    pub fn increment_frequency(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE items SET frequency = frequency + 1, updated_at = datetime('now') WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to increment frequency: {e}"))?;
        Ok(())
    }

    pub fn import_items(&self, items: Vec<NewItem>) -> Result<Vec<Item>, String> {
        let mut imported = Vec::new();
        for item in items {
            imported.push(self.add_item(item)?);
        }
        Ok(imported)
    }

    pub fn export_items(&self) -> Result<Vec<Item>, String> {
        self.list_items(None, true)
    }

    // --- Settings CRUD ---

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        match self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to get setting: {e}")),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
                params![key, value],
            )
            .map_err(|e| format!("Failed to set setting: {e}"))?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<bool, String> {
        let rows = self
            .conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])
            .map_err(|e| format!("Failed to delete setting: {e}"))?;
        Ok(rows > 0)
    }

    pub fn get_all_settings(&self) -> Result<Vec<Setting>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key ASC")
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let settings = stmt
            .query_map([], |row| {
                Ok(Setting {
                    key: row.get(0)?,
                    value: row.get(1)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<Setting>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(settings)
    }

    // --- Command History ---

    pub fn record_command(&self, entry: NewCommandHistory) -> Result<CommandHistory, String> {
        let id = Uuid::new_v4().to_string();
        let source = entry.source.unwrap_or_else(|| "launcher".to_string());

        self.conn
            .execute(
                "INSERT INTO command_history (id, item_id, command_text, action_type, source)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    entry.item_id,
                    entry.command_text,
                    entry.action_type,
                    source
                ],
            )
            .map_err(|e| format!("Failed to record command: {e}"))?;

        self.get_command_history_entry(&id)
    }

    fn get_command_history_entry(&self, id: &str) -> Result<CommandHistory, String> {
        self.conn
            .query_row(
                "SELECT id, item_id, command_text, action_type, executed_at, source
                 FROM command_history WHERE id = ?1",
                params![id],
                |row| {
                    Ok(CommandHistory {
                        id: row.get(0)?,
                        item_id: row.get(1)?,
                        command_text: row.get(2)?,
                        action_type: row.get(3)?,
                        executed_at: row.get(4)?,
                        source: row.get(5)?,
                    })
                },
            )
            .map_err(|e| format!("Command history entry not found: {e}"))
    }

    pub fn search_command_history(&self, query: &str) -> Result<Vec<CommandHistory>, String> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, item_id, command_text, action_type, executed_at, source
                 FROM command_history
                 WHERE command_text LIKE ?1
                 ORDER BY executed_at DESC
                 LIMIT 20",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let entries = stmt
            .query_map(params![pattern], |row| {
                Ok(CommandHistory {
                    id: row.get(0)?,
                    item_id: row.get(1)?,
                    command_text: row.get(2)?,
                    action_type: row.get(3)?,
                    executed_at: row.get(4)?,
                    source: row.get(5)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<CommandHistory>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(entries)
    }

    pub fn get_recent_commands(&self, limit: usize) -> Result<Vec<CommandHistory>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, item_id, command_text, action_type, executed_at, source
                 FROM command_history
                 ORDER BY executed_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let entries = stmt
            .query_map(params![limit as i64], |row| {
                Ok(CommandHistory {
                    id: row.get(0)?,
                    item_id: row.get(1)?,
                    command_text: row.get(2)?,
                    action_type: row.get(3)?,
                    executed_at: row.get(4)?,
                    source: row.get(5)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<CommandHistory>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(entries)
    }

    pub fn suggest_commands(&self, query: &str) -> Result<Vec<CommandSuggestion>, String> {
        let mut suggestions = Vec::new();

        // 1. Check command history for matches
        let history_matches = self.search_command_history(query)?;
        for entry in history_matches.iter().take(3) {
            suggestions.push(CommandSuggestion {
                suggested_command: entry.command_text.clone(),
                reason: "history_match".to_string(),
                confidence: 0.8,
                source_item_id: entry.item_id.clone(),
            });
        }

        // 2. Check existing items for keyword overlap
        let words: Vec<&str> = query.split_whitespace().collect();
        if !words.is_empty() {
            let first_word = words[0];
            let word_pattern = format!("%{first_word}%");
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT id, title, action_type, action_value, category
                     FROM items
                     WHERE enabled = 1 AND (action_value LIKE ?1 OR title LIKE ?1)
                     ORDER BY frequency DESC
                     LIMIT 5",
                )
                .map_err(|e| format!("Failed to prepare query: {e}"))?;

            let related: Vec<(String, String, String, String, String)> = stmt
                .query_map(params![word_pattern], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(|e| format!("Failed to execute query: {e}"))?
                .collect::<SqlResult<Vec<_>>>()
                .map_err(|e| format!("Failed to collect results: {e}"))?;

            for (id, _title, _action_type, _action_value, _category) in &related {
                if !suggestions.iter().any(|s| s.suggested_command == query) {
                    suggestions.push(CommandSuggestion {
                        suggested_command: query.to_string(),
                        reason: "similar_item".to_string(),
                        confidence: 0.6,
                        source_item_id: Some(id.clone()),
                    });
                }
            }
        }

        // 3. Fallback: treat query as potential command
        if suggestions.is_empty() && query.len() > 2 {
            suggestions.push(CommandSuggestion {
                suggested_command: query.to_string(),
                reason: "query_parse".to_string(),
                confidence: 0.4,
                source_item_id: None,
            });
        }

        Ok(suggestions)
    }

    /// Get recent rewrite prompts from command history (action_type = 'rewrite').
    /// Returns distinct prompts ordered by most recent, with their execution count.
    pub fn get_recent_rewrites(&self, limit: usize) -> Result<Vec<CommandSuggestion>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT command_text, COUNT(*) as cnt, MAX(executed_at) as last_used
                 FROM command_history
                 WHERE action_type = 'rewrite'
                 GROUP BY command_text
                 ORDER BY cnt DESC, last_used DESC
                 LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let entries = stmt
            .query_map(params![limit as i64], |row| {
                let command_text: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok(CommandSuggestion {
                    suggested_command: command_text,
                    reason: "rewrite_history".to_string(),
                    confidence: (count as f64 / 10.0).min(1.0),
                    source_item_id: None,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<CommandSuggestion>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(entries)
    }

    // --- Memory ---

    pub fn add_memory(&self, mem: NewMemory) -> Result<Memory, String> {
        let memory_type = mem.memory_type.unwrap_or_else(|| "fact".to_string());
        let confidence = mem.confidence.unwrap_or(1.0);

        // Check if a memory with the same key+context already exists (NULL-safe)
        if let Ok(existing) = self.get_memory_by_key(&mem.key, mem.context.as_deref()) {
            // Update existing memory
            self.conn
                .execute(
                    "UPDATE memory SET value = ?1, confidence = ?2, memory_type = ?3, updated_at = datetime('now')
                     WHERE id = ?4",
                    params![mem.value, confidence, memory_type, existing.id],
                )
                .map_err(|e| format!("Failed to update memory: {e}"))?;
            return self.get_memory(&existing.id);
        }

        // Insert new memory
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO memory (id, key, value, context, memory_type, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, mem.key, mem.value, mem.context, memory_type, confidence],
            )
            .map_err(|e| format!("Failed to add memory: {e}"))?;

        self.get_memory(&id)
    }

    pub fn get_memory(&self, id: &str) -> Result<Memory, String> {
        self.conn
            .query_row(
                "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                 FROM memory WHERE id = ?1",
                params![id],
                Self::row_to_memory,
            )
            .map_err(|e| format!("Memory not found: {e}"))
    }

    pub fn get_memory_by_key(&self, key: &str, context: Option<&str>) -> Result<Memory, String> {
        match context {
            Some(ctx) => self
                .conn
                .query_row(
                    "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                     FROM memory WHERE key = ?1 AND context = ?2",
                    params![key, ctx],
                    Self::row_to_memory,
                )
                .map_err(|e| format!("Memory not found: {e}")),
            None => self
                .conn
                .query_row(
                    "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                     FROM memory WHERE key = ?1 AND context IS NULL",
                    params![key],
                    Self::row_to_memory,
                )
                .map_err(|e| format!("Memory not found: {e}")),
        }
    }

    pub fn remove_memory(&self, id: &str) -> Result<bool, String> {
        let rows = self
            .conn
            .execute("DELETE FROM memory WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to remove memory: {e}"))?;
        Ok(rows > 0)
    }

    pub fn search_memories(&self, query: &str) -> Result<Vec<Memory>, String> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                 FROM memory
                 WHERE key LIKE ?1 OR value LIKE ?1 OR context LIKE ?1
                 ORDER BY last_accessed DESC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let memories = stmt
            .query_map(params![pattern], Self::row_to_memory)
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<Memory>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(memories)
    }

    pub fn list_memories(&self, memory_type: Option<&str>) -> Result<Vec<Memory>, String> {
        match memory_type {
            Some(mt) => {
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                         FROM memory WHERE memory_type = ?1 ORDER BY updated_at DESC",
                    )
                    .map_err(|e| format!("Failed to prepare query: {e}"))?;

                let memories = stmt
                    .query_map(params![mt], Self::row_to_memory)
                    .map_err(|e| format!("Failed to execute query: {e}"))?
                    .collect::<SqlResult<Vec<Memory>>>()
                    .map_err(|e| format!("Failed to collect results: {e}"))?;

                Ok(memories)
            }
            None => {
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                         FROM memory ORDER BY updated_at DESC",
                    )
                    .map_err(|e| format!("Failed to prepare query: {e}"))?;

                let memories = stmt
                    .query_map([], Self::row_to_memory)
                    .map_err(|e| format!("Failed to execute query: {e}"))?
                    .collect::<SqlResult<Vec<Memory>>>()
                    .map_err(|e| format!("Failed to collect results: {e}"))?;

                Ok(memories)
            }
        }
    }

    pub fn touch_memory(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE memory SET last_accessed = datetime('now') WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to touch memory: {e}"))?;
        Ok(())
    }

    pub fn get_relevant_memories(&self, context: Option<&str>) -> Result<Vec<Memory>, String> {
        match context {
            Some(ctx) => {
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                         FROM memory
                         WHERE memory_type IN ('preference', 'pattern')
                           AND (context IS NULL OR context = ?1)
                           AND confidence > 0.3
                         ORDER BY confidence DESC, last_accessed DESC
                         LIMIT 20",
                    )
                    .map_err(|e| format!("Failed to prepare query: {e}"))?;

                let memories = stmt
                    .query_map(params![ctx], Self::row_to_memory)
                    .map_err(|e| format!("Failed to execute query: {e}"))?
                    .collect::<SqlResult<Vec<Memory>>>()
                    .map_err(|e| format!("Failed to collect results: {e}"))?;

                Ok(memories)
            }
            None => {
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT id, key, value, context, memory_type, confidence, created_at, updated_at, last_accessed
                         FROM memory
                         WHERE memory_type IN ('preference', 'pattern')
                           AND confidence > 0.3
                         ORDER BY confidence DESC, last_accessed DESC
                         LIMIT 20",
                    )
                    .map_err(|e| format!("Failed to prepare query: {e}"))?;

                let memories = stmt
                    .query_map([], Self::row_to_memory)
                    .map_err(|e| format!("Failed to execute query: {e}"))?
                    .collect::<SqlResult<Vec<Memory>>>()
                    .map_err(|e| format!("Failed to collect results: {e}"))?;

                Ok(memories)
            }
        }
    }

    // --- Conversations ---

    pub fn create_conversation(&self, conv: NewConversation) -> Result<Conversation, String> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO conversations (id, title) VALUES (?1, ?2)",
                params![id, conv.title],
            )
            .map_err(|e| format!("Failed to create conversation: {e}"))?;
        self.get_conversation(&id)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation, String> {
        self.conn
            .query_row(
                "SELECT id, title, created_at, updated_at FROM conversations WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Conversation {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .map_err(|e| format!("Conversation not found: {e}"))
    }

    pub fn list_conversations(&self, limit: usize) -> Result<Vec<ConversationWithPreview>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.id, c.title, c.created_at, c.updated_at,
                        (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id = c.id) as message_count,
                        (SELECT content FROM conversation_messages WHERE conversation_id = c.id ORDER BY created_at DESC LIMIT 1) as last_message_preview
                 FROM conversations c
                 ORDER BY c.updated_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let convs = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ConversationWithPreview {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    message_count: row.get(4)?,
                    last_message_preview: row.get(5)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<ConversationWithPreview>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(convs)
    }

    pub fn search_conversations(
        &self,
        query: &str,
    ) -> Result<Vec<ConversationWithPreview>, String> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT c.id, c.title, c.created_at, c.updated_at,
                        (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id = c.id) as message_count,
                        (SELECT content FROM conversation_messages WHERE conversation_id = c.id ORDER BY created_at DESC LIMIT 1) as last_message_preview
                 FROM conversations c
                 LEFT JOIN conversation_messages m ON m.conversation_id = c.id
                 WHERE c.title LIKE ?1 OR m.content LIKE ?1
                 ORDER BY c.updated_at DESC
                 LIMIT 20",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let convs = stmt
            .query_map(params![pattern], |row| {
                Ok(ConversationWithPreview {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    message_count: row.get(4)?,
                    last_message_preview: row.get(5)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<ConversationWithPreview>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(convs)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<bool, String> {
        // Delete messages first (no FK cascade without pragma)
        self.conn
            .execute(
                "DELETE FROM conversation_messages WHERE conversation_id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to delete conversation messages: {e}"))?;
        let rows = self
            .conn
            .execute("DELETE FROM conversations WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete conversation: {e}"))?;
        Ok(rows > 0)
    }

    pub fn touch_conversation(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to touch conversation: {e}"))?;
        Ok(())
    }

    pub fn add_conversation_message(
        &self,
        msg: NewConversationMessage,
    ) -> Result<ConversationMessage, String> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO conversation_messages (id, conversation_id, role, content)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, msg.conversation_id, msg.role, msg.content],
            )
            .map_err(|e| format!("Failed to add conversation message: {e}"))?;

        // Touch the conversation's updated_at
        let _ = self.touch_conversation(&msg.conversation_id);

        self.conn
            .query_row(
                "SELECT id, conversation_id, role, content, created_at
                 FROM conversation_messages WHERE id = ?1",
                params![id],
                |row| {
                    Ok(ConversationMessage {
                        id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .map_err(|e| format!("Conversation message not found: {e}"))
    }

    pub fn get_conversation_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessage>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, conversation_id, role, content, created_at
                 FROM conversation_messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let messages = stmt
            .query_map(params![conversation_id], |row| {
                Ok(ConversationMessage {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<ConversationMessage>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(messages)
    }

    pub fn search_conversation_messages(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationMessage>, String> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, conversation_id, role, content, created_at
                 FROM conversation_messages
                 WHERE content LIKE ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let messages = stmt
            .query_map(params![pattern, limit as i64], |row| {
                Ok(ConversationMessage {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<ConversationMessage>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(messages)
    }

    pub fn get_recent_conversation_context(
        &self,
        limit: usize,
    ) -> Result<Vec<(Conversation, Vec<ConversationMessage>)>, String> {
        let conversations = self.list_conversations(limit)?;
        let mut result = Vec::new();

        for preview in conversations {
            let conv = self.get_conversation(&preview.id)?;
            // Get last 5 messages per conversation for context
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT id, conversation_id, role, content, created_at
                     FROM conversation_messages
                     WHERE conversation_id = ?1
                     ORDER BY created_at DESC
                     LIMIT 5",
                )
                .map_err(|e| format!("Failed to prepare query: {e}"))?;

            let mut messages: Vec<ConversationMessage> = stmt
                .query_map(params![preview.id], |row| {
                    Ok(ConversationMessage {
                        id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })
                .map_err(|e| format!("Failed to execute query: {e}"))?
                .collect::<SqlResult<Vec<ConversationMessage>>>()
                .map_err(|e| format!("Failed to collect results: {e}"))?;

            // Reverse to chronological order
            messages.reverse();
            result.push((conv, messages));
        }

        Ok(result)
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
        Ok(Memory {
            id: row.get(0)?,
            key: row.get(1)?,
            value: row.get(2)?,
            context: row.get(3)?,
            memory_type: row.get(4)?,
            confidence: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            last_accessed: row.get(8)?,
        })
    }

    fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<Item> {
        Ok(Item {
            id: row.get(0)?,
            title: row.get(1)?,
            subtitle: row.get(2)?,
            icon: row.get(3)?,
            action_type: row.get(4)?,
            action_value: row.get(5)?,
            category: row.get(6)?,
            tags: row.get(7)?,
            frequency: row.get(8)?,
            enabled: row.get::<_, i64>(9)? != 0,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }

    // --- Slash Commands ---

    pub fn add_slash_command(&self, cmd: NewSlashCommand) -> Result<SlashCommand, String> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO slash_commands (id, name, description, script_path)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, cmd.name, cmd.description, cmd.script_path],
            )
            .map_err(|e| format!("Failed to add slash command: {e}"))?;
        self.get_slash_command(&id)
    }

    pub fn get_slash_command(&self, id: &str) -> Result<SlashCommand, String> {
        self.conn
            .query_row(
                "SELECT id, name, description, script_path, usage_count, created_at, updated_at
                 FROM slash_commands WHERE id = ?1",
                params![id],
                Self::row_to_slash_command,
            )
            .map_err(|e| format!("Slash command not found: {e}"))
    }

    pub fn get_slash_command_by_name(&self, name: &str) -> Result<SlashCommand, String> {
        self.conn
            .query_row(
                "SELECT id, name, description, script_path, usage_count, created_at, updated_at
                 FROM slash_commands WHERE name = ?1",
                params![name],
                Self::row_to_slash_command,
            )
            .map_err(|e| format!("Slash command '/{name}' not found: {e}"))
    }

    pub fn list_slash_commands(&self) -> Result<Vec<SlashCommand>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, description, script_path, usage_count, created_at, updated_at
                 FROM slash_commands
                 ORDER BY usage_count DESC, name ASC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let commands = stmt
            .query_map([], Self::row_to_slash_command)
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<SlashCommand>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(commands)
    }

    pub fn search_slash_commands(&self, query: &str) -> Result<Vec<SlashCommand>, String> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, description, script_path, usage_count, created_at, updated_at
                 FROM slash_commands
                 WHERE name LIKE ?1 OR description LIKE ?1
                 ORDER BY usage_count DESC, name ASC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let commands = stmt
            .query_map(params![pattern], Self::row_to_slash_command)
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<SlashCommand>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(commands)
    }

    pub fn remove_slash_command_by_name(&self, name: &str) -> Result<bool, String> {
        // Delete params first (no FK cascade in SQLite by default)
        self.conn
            .execute(
                "DELETE FROM slash_command_params WHERE command_id = (SELECT id FROM slash_commands WHERE name = ?1)",
                params![name],
            )
            .map_err(|e| format!("Failed to remove slash command params: {e}"))?;
        let rows = self
            .conn
            .execute("DELETE FROM slash_commands WHERE name = ?1", params![name])
            .map_err(|e| format!("Failed to remove slash command: {e}"))?;
        Ok(rows > 0)
    }

    pub fn increment_slash_command_usage(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE slash_commands SET usage_count = usage_count + 1, updated_at = datetime('now') WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to increment slash command usage: {e}"))?;
        Ok(())
    }

    fn row_to_slash_command(row: &rusqlite::Row) -> rusqlite::Result<SlashCommand> {
        Ok(SlashCommand {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            script_path: row.get(3)?,
            usage_count: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }

    // --- Slash Command Params ---

    pub fn add_slash_command_params(
        &self,
        command_id: &str,
        params: Vec<NewSlashCommandParam>,
    ) -> Result<Vec<SlashCommandParam>, String> {
        // Replace all existing params for this command
        self.conn
            .execute(
                "DELETE FROM slash_command_params WHERE command_id = ?1",
                params![command_id],
            )
            .map_err(|e| format!("Failed to clear existing params: {e}"))?;

        for p in &params {
            let id = Uuid::new_v4().to_string();
            self.conn
                .execute(
                    "INSERT INTO slash_command_params (id, command_id, name, description, position, required)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, command_id, p.name, p.description, p.position, p.required],
                )
                .map_err(|e| format!("Failed to add slash command param: {e}"))?;
        }

        self.get_slash_command_params(command_id)
    }

    pub fn get_slash_command_params(
        &self,
        command_id: &str,
    ) -> Result<Vec<SlashCommandParam>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, command_id, name, description, position, required
                 FROM slash_command_params
                 WHERE command_id = ?1
                 ORDER BY position ASC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let results = stmt
            .query_map(params![command_id], Self::row_to_slash_command_param)
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<SlashCommandParam>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(results)
    }

    pub fn get_params_by_command_name(
        &self,
        name: &str,
    ) -> Result<Vec<SlashCommandParam>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT p.id, p.command_id, p.name, p.description, p.position, p.required
                 FROM slash_command_params p
                 JOIN slash_commands c ON c.id = p.command_id
                 WHERE c.name = ?1
                 ORDER BY p.position ASC",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let results = stmt
            .query_map(params![name], Self::row_to_slash_command_param)
            .map_err(|e| format!("Failed to execute query: {e}"))?
            .collect::<SqlResult<Vec<SlashCommandParam>>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(results)
    }

    fn row_to_slash_command_param(row: &rusqlite::Row) -> rusqlite::Result<SlashCommandParam> {
        Ok(SlashCommandParam {
            id: row.get(0)?,
            command_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            position: row.get(4)?,
            required: row.get(5)?,
        })
    }
}
