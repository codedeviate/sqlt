INSERT INTO users (name) VALUES ('alice') RETURNING id;
UPDATE users SET name = 'bob' WHERE id = 1 RETURNING id, name;
DELETE FROM users WHERE id = 1 RETURNING id
