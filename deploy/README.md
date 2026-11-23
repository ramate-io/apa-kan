# Admin panel deploy

Kestrel (`ramate-io/kestrel`) is not user-ready yet, so local orchestration uses **process-compose** or **docker compose**.

## Docker

```bash
docker compose -f deploy/docker-compose.yml up --build
```

- Postgres + pgvector: `localhost:5432` (`apa` / `apa` / `apa_kan`)
- Server: `http://localhost:8080/health`
- Admin: `http://localhost:3000`

The first signed-in account becomes admin (`APA_KAN_ALLOW_FIRST_ADMIN=1`). Pin an account with `APA_KAN_ADMIN_ACCOUNT=<64-char hex>`.

## process-compose

```bash
process-compose -f deploy/process-compose.yaml up
```

Requires Docker for Postgres, and a Rust toolchain for the server/admin binaries.

## SQLite (no Postgres)

```bash
APA_KAN_DATABASE_URL=sqlite:./apa-kan.db APA_KAN_ALLOW_FIRST_ADMIN=1 cargo run -p apa-kan-server
APA_KAN_SERVER_URL=http://127.0.0.1:8080 cargo run -p apa-kan-admin
```

SQLite uses brute-force cosine. Generation against a published version matches Postgres because both load the same snapshot into `assign_character`.
