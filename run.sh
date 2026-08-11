#!/bin/bash

# Для bluefin, где установка была через brew
export PKG_CONFIG_PATH="/home/linuxbrew/.linuxbrew/lib/pkgconfig:$PKG_CONFIG_PATH";
cargo run