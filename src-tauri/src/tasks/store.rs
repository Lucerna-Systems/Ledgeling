use super::types::*;
use rusqlite::{params, Connection as Database};
use std::path::Path;

pub struct Store(pub Database);
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Database::open(path).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS tasks(id INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS config(id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL);")
            .map_err(|e| e.to_string())?;
        let store = Self(db);
        for mut task in store.list()? {
            if matches!(
                task.status.as_str(),
                "queued" | "running" | "waiting" | "cancelling" | "undoing"
            ) {
                task.status = "interrupted".into();
                task.approval = None;
                task.progress = "Interrupted by restart. No work was automatically resumed. Review the operation journal before undoing.".into();
                store.save(&task)?;
            }
        }
        Ok(store)
    }
    pub fn insert(&self, mut task: Task) -> Result<Task, String> {
        self.0
            .execute(
                "INSERT INTO tasks(body) VALUES (?1)",
                [serde_json::to_string(&task).map_err(|e| e.to_string())?],
            )
            .map_err(|e| e.to_string())?;
        task.id = self.0.last_insert_rowid();
        self.save(&task)?;
        Ok(task)
    }
    pub fn save(&self, task: &Task) -> Result<(), String> {
        self.0
            .execute(
                "UPDATE tasks SET body=?1 WHERE id=?2",
                params![
                    serde_json::to_string(task).map_err(|e| e.to_string())?,
                    task.id
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Task>, String> {
        let mut statement = self
            .0
            .prepare("SELECT body FROM tasks ORDER BY id DESC")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn get(&self, id: i64) -> Result<Task, String> {
        let s: String = self
            .0
            .query_row("SELECT body FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .map_err(|_| "Task not found".to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())
    }
    pub fn config(&self) -> Connections {
        self.0
            .query_row("SELECT body FROM config WHERE id=1", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| serde_json::from_str::<Connections>(&s).ok())
            .map(|mut config| {
                super::setup::normalize(&mut config);
                config
            })
            .unwrap_or_default()
    }
    pub fn set_config(&self, config: &Connections) -> Result<(), String> {
        self.0
            .execute(
                "INSERT OR REPLACE INTO config(id,body) VALUES(1,?1)",
                [serde_json::to_string(config).map_err(|e| e.to_string())?],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
