#!/bin/zsh

docker run --name my-postgres -e POSTGRES_USER=admin -e POSTGRES_PASSWORD=admin -e POSTGRES_DB=defaultdb -p 5432:5432 -d postgres