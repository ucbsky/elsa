#!/bin/sh
cd $(dirname "$0")
RUSTFLAGS='-C target-cpu=native' cargo bench