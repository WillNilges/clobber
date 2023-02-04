#!/bin/bash

set -e

cargo build --release
sudo install target/release/clobber /usr/local/sbin; echo "Installed clobber."
sudo cp res/clobber.sh /etc/profile.d; echo "Added profile.d script."
