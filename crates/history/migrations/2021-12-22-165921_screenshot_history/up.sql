CREATE TABLE screenshots (
    id INTEGER UNIQUE NOT NULL,
    path TEXT,
    time TEXT NOT NULL,
    url TEXT,
    PRIMARY KEY(id AUTOINCREMENT)
);
