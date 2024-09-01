#!/bin/sh

cargo lambda build --release --arm64
zip -rj bootstrap.zip target/lambda/proxy2/bootstrap
