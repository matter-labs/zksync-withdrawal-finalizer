#!/usr/bin/env bash

docker stop pg-finalizer
docker rm pg-finalizer

docker run -d --name pg-finalizer \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=finalizer \
  -p 5432:5432 postgres:16

cd ./storage

env DATABASE_URL=postgres://postgres:postgres@localhost:5432/finalizer sqlx database create
env DATABASE_URL=postgres://postgres:postgres@localhost:5432/finalizer sqlx migrate run

cd ..

run_helpers_test/run.sh
