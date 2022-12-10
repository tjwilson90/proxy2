#!/bin/sh

cargo lambda build --release --arm64
zip -rj bootstrap.zip ../rust-target/lambda/proxy2/bootstrap
