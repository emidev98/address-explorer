CREATE TABLE IF NOT EXISTS hello_world (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO hello_world (message)
VALUES ('Hello, world!');
