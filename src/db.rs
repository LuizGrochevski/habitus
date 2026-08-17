use anyhow::Result;
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection};

use crate::models::Habit;
use crate::streak::Frequency;

pub fn init_db(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS habits (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL
        );
        CREATE TABLE IF NOT EXISTS checkins (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            habit_id  INTEGER NOT NULL,
            date      TEXT NOT NULL,
            UNIQUE(habit_id, date),
            FOREIGN KEY(habit_id) REFERENCES habits(id)
        );
        ",
    )?;

    if !column_exists(&conn, "habits", "reminder_time")? {
        conn.execute_batch("ALTER TABLE habits ADD COLUMN reminder_time TEXT;")?;
    }
    if !column_exists(&conn, "habits", "daily_target")? {
        conn.execute_batch(
            "ALTER TABLE habits ADD COLUMN daily_target INTEGER NOT NULL DEFAULT 1;",
        )?;
    }
    if !column_exists(&conn, "habits", "weekly_target")? {
        conn.execute_batch("ALTER TABLE habits ADD COLUMN weekly_target INTEGER;")?;
    }

    if checkins_has_unique_constraint(&conn)? {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            ALTER TABLE checkins RENAME TO checkins_old;
            CREATE TABLE checkins (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                habit_id  INTEGER NOT NULL,
                date      TEXT NOT NULL,
                FOREIGN KEY(habit_id) REFERENCES habits(id)
            );
            INSERT INTO checkins (id, habit_id, date) SELECT id, habit_id, date FROM checkins_old;
            DROP TABLE checkins_old;
            ",
        )?;
    }

    Ok(conn)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for r in rows {
        if r? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn checkins_has_unique_constraint(conn: &Connection) -> Result<bool> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='checkins'",
            [],
            |row| row.get(0),
        )
        .ok();
    Ok(sql.map(|s| s.contains("UNIQUE(habit_id, date)")).unwrap_or(false))
}

pub fn create_habit(conn: &Connection, name: &str) -> Result<()> {
    conn.execute("INSERT OR IGNORE INTO habits (name) VALUES (?1)", params![name])?;
    Ok(())
}

pub fn list_habits(conn: &Connection) -> Result<Vec<Habit>> {
    let mut stmt = conn.prepare("SELECT id, name FROM habits ORDER BY name")?;
    let rows = stmt.query_map([], |row| {
        Ok(Habit {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    })?;
    let mut habits = Vec::new();
    for r in rows {
        habits.push(r?);
    }
    Ok(habits)
}

fn habit_id(conn: &Connection, name: &str) -> Result<i64> {
    let result: rusqlite::Result<i64> = conn.query_row(
        "SELECT id FROM habits WHERE name = ?1",
        params![name],
        |row| row.get(0),
    );
    result.map_err(|_| {
        anyhow::anyhow!(
            "Hábito '{}' não encontrado. Crie primeiro com: habitus habit \"{}\"",
            name, name
        )
    })
}

pub fn mark_done(conn: &Connection, name: &str) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    let today = Local::now().date_naive().to_string();
    conn.execute(
        "INSERT INTO checkins (habit_id, date) VALUES (?1, ?2)",
        params![h_id, today],
    )?;
    Ok(())
}

pub fn checkins_for(conn: &Connection, name: &str) -> Result<Vec<NaiveDate>> {
    let h_id = habit_id(conn, name)?;
    let mut stmt = conn.prepare("SELECT date FROM checkins WHERE habit_id = ?1")?;
    let rows = stmt.query_map(params![h_id], |row| row.get::<_, String>(0))?;

    let mut dates = Vec::new();
    for r in rows {
        let s = r?;
        dates.push(NaiveDate::parse_from_str(&s, "%Y-%m-%d")?);
    }
    Ok(dates)
}

pub fn daily_target(conn: &Connection, name: &str) -> Result<i32> {
    let h_id = habit_id(conn, name)?;
    let target: i32 = conn.query_row(
        "SELECT daily_target FROM habits WHERE id = ?1",
        params![h_id],
        |row| row.get(0),
    )?;
    Ok(target)
}

pub fn set_daily_target(conn: &Connection, name: &str, target: i32) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    conn.execute(
        "UPDATE habits SET daily_target = ?1 WHERE id = ?2",
        params![target, h_id],
    )?;
    Ok(())
}

pub fn set_weekly_target(conn: &Connection, name: &str, target: i32) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    conn.execute(
        "UPDATE habits SET weekly_target = ?1 WHERE id = ?2",
        params![target, h_id],
    )?;
    Ok(())
}

pub fn clear_weekly_target(conn: &Connection, name: &str) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    conn.execute(
        "UPDATE habits SET weekly_target = NULL WHERE id = ?1",
        params![h_id],
    )?;
    Ok(())
}

/// Decide qual "modo de meta" um hábito usa: se `weekly_target` estiver
/// definido, o hábito é avaliado por semana (N check-ins em qualquer dia
/// da semana); senão, cai no modo diário de sempre (`daily_target`).
pub fn habit_frequency(conn: &Connection, name: &str) -> Result<Frequency> {
    let h_id = habit_id(conn, name)?;
    let (daily, weekly): (i32, Option<i32>) = conn.query_row(
        "SELECT daily_target, weekly_target FROM habits WHERE id = ?1",
        params![h_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    Ok(match weekly {
        Some(w) => Frequency::Weekly { weekly_target: w },
        None => Frequency::Daily { daily_target: daily },
    })
}

pub fn delete_habit(conn: &Connection, name: &str) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    conn.execute("DELETE FROM checkins WHERE habit_id = ?1", params![h_id])?;
    conn.execute("DELETE FROM habits WHERE id = ?1", params![h_id])?;
    Ok(())
}

pub fn unmark_today(conn: &Connection, name: &str) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    let today = Local::now().date_naive().to_string();
    conn.execute(
        "DELETE FROM checkins WHERE id = (
            SELECT id FROM checkins WHERE habit_id = ?1 AND date = ?2 ORDER BY id DESC LIMIT 1
        )",
        params![h_id, today],
    )?;
    Ok(())
}

pub fn set_reminder(conn: &Connection, name: &str, time: &str) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    conn.execute(
        "UPDATE habits SET reminder_time = ?1 WHERE id = ?2",
        params![time, h_id],
    )?;
    Ok(())
}

pub fn clear_reminder(conn: &Connection, name: &str) -> Result<()> {
    let h_id = habit_id(conn, name)?;
    conn.execute(
        "UPDATE habits SET reminder_time = NULL WHERE id = ?1",
        params![h_id],
    )?;
    Ok(())
}

pub fn habits_with_reminders(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT name, reminder_time FROM habits WHERE reminder_time IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}
