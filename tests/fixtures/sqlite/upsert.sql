INSERT INTO users (id, name) VALUES (1, 'alice') ON CONFLICT(id) DO UPDATE SET name = excluded.name
