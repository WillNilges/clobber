#!/bin/bash

set -e

cargo build --release
cp clobber.sh /etc/profile.d
install target/release/clobber /usr/local/sbin
