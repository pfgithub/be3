use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerLocation {
    Local,
    Remote(String),
}

impl ServerLocation {
    pub fn key(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Remote(url) => url,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedAccount {
    pub server: ServerLocation,
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub last_workspace_id: Option<Uuid>,
}

pub struct AppStateStore {
    connection: Connection,
}

impl AppStateStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS saved_accounts (
                server_key        TEXT NOT NULL,
                server_url        TEXT,
                account_id        TEXT NOT NULL,
                email             TEXT NOT NULL,
                display_name      TEXT NOT NULL,
                last_workspace_id TEXT,
                PRIMARY KEY (server_key, account_id)
            );
            CREATE TABLE IF NOT EXISTS app_settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(Self { connection })
    }

    pub fn accounts(&self) -> Result<Vec<SavedAccount>, rusqlite::Error> {
        let mut statement = self.connection.prepare(
            "SELECT server_key, server_url, account_id, email, display_name, last_workspace_id
             FROM saved_accounts ORDER BY display_name COLLATE NOCASE, account_id",
        )?;
        let accounts = statement
            .query_map([], |row| {
                let server_key = row.get::<_, String>(0)?;
                let server_url = row.get::<_, Option<String>>(1)?;
                let account_id = parse_uuid(row.get::<_, String>(2)?)?;
                let last_workspace_id = row
                    .get::<_, Option<String>>(5)?
                    .map(parse_uuid)
                    .transpose()?;
                Ok(SavedAccount {
                    server: if server_key == "local" {
                        ServerLocation::Local
                    } else {
                        ServerLocation::Remote(server_url.unwrap_or(server_key))
                    },
                    id: account_id,
                    email: row.get(3)?,
                    name: row.get(4)?,
                    last_workspace_id,
                })
            })?
            .collect();
        accounts
    }

    pub fn save_account(&self, saved: &SavedAccount) -> Result<(), rusqlite::Error> {
        let server_url = match &saved.server {
            ServerLocation::Local => None,
            ServerLocation::Remote(url) => Some(url.as_str()),
        };
        self.connection.execute(
            "INSERT INTO saved_accounts
             (server_key, server_url, account_id, email, display_name, last_workspace_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(server_key, account_id) DO UPDATE SET
                server_url = excluded.server_url,
                email = excluded.email,
                display_name = excluded.display_name,
                last_workspace_id = excluded.last_workspace_id",
            params![
                saved.server.key(),
                server_url,
                saved.id.to_string(),
                &saved.email,
                &saved.name,
                saved.last_workspace_id.map(|id| id.to_string())
            ],
        )?;
        Ok(())
    }

    pub fn remove_account(&self, saved: &SavedAccount) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "DELETE FROM saved_accounts WHERE server_key = ?1 AND account_id = ?2",
            params![saved.server.key(), saved.id.to_string()],
        )?;
        if self.active_account()?.as_ref() == Some(&(saved.server.key().into(), saved.id)) {
            self.clear_active_account()?;
        }
        Ok(())
    }

    pub fn set_active_account(&self, saved: &SavedAccount) -> Result<(), rusqlite::Error> {
        self.set_setting("active_server", saved.server.key())?;
        self.set_setting("active_account", &saved.id.to_string())
    }

    pub fn clear_active_account(&self) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "DELETE FROM app_settings WHERE key IN ('active_server', 'active_account')",
            [],
        )?;
        Ok(())
    }

    pub fn active_account(&self) -> Result<Option<(String, Uuid)>, rusqlite::Error> {
        let server = self.setting("active_server")?;
        let account = self.setting("active_account")?;
        match (server, account) {
            (Some(server), Some(account)) => Ok(Some((server, parse_uuid(account)?))),
            _ => Ok(None),
        }
    }

    #[allow(dead_code)]
    pub fn set_last_workspace(
        &self,
        saved: &SavedAccount,
        workspace_id: Option<Uuid>,
    ) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "UPDATE saved_accounts SET last_workspace_id = ?3
             WHERE server_key = ?1 AND account_id = ?2",
            params![
                saved.server.key(),
                saved.id.to_string(),
                workspace_id.map(|id| id.to_string())
            ],
        )?;
        Ok(())
    }

    fn setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn parse_uuid(value: String) -> Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

#[cfg(test)]
mod tests;
