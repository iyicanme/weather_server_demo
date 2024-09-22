-- Add migration script here
CREATE TABLE user (
    id              INTEGER             PRIMARY KEY,
    username        TEXT                NOT NULL                UNIQUE,
    email           TEXT                NOT NULL                UNIQUE,
    password        TEXT                NOT NULL
);