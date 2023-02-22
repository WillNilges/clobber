#!/bin/bash

set -e

cargo build --release
sudo install target/release/clobber /usr/local/sbin && chmod u+s /usr/local/sbin/clobber && echo "Installed clobber."
sudo install target/release/clobberd /usr/local/sbin && echo "Installed clobberd."
sudo cp res/clobber.sh /etc/profile.d; echo "Added profile.d script."
