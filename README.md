# TinyURL

## Setup
Run a Postgres in docker
```sh
docker run -d -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres
```

## Test
```sh
cargo run
```

In a second terminal window:
```sh
> curl -XPOST localhost:9876 -H "Content-Type: application/json" -d '{"url": "https://www.postgresql.org/docs/9.1/sql-createdatabase.html"}'
{"url":"127.0.0.1:9876/QxidHT"}
```