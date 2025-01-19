use std::error::Error;

pub trait Save {
    fn persist(&self, conn: &sqlite::Connection) -> Result<(), Box<dyn Error>>;
}

pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

impl Save for KeyValuePair {
    fn persist(&self, conn: &sqlite::Connection) -> Result<(), Box<dyn Error>> {
        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS key_value_pairs (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ",
        )?;

        // Check if the key already exists, then update it, if not insert it
        let upsert_query = "
            INSERT INTO key_value_pairs (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE
                SET value = excluded.value;
        ";

        let mut statement = conn.prepare(upsert_query)?;
        statement.bind((1, self.key.as_str()))?;
        statement.bind((2, self.value.as_str()))?;
        statement.next()?;

        Ok(())
    }
}
