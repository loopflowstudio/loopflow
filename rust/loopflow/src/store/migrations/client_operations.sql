-- A database lifetime, independent of server restarts and Machine routing.
CREATE TABLE client_store_epoch (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    epoch TEXT NOT NULL
);
INSERT INTO client_store_epoch VALUES (1, lower(hex(randomblob(16))));
