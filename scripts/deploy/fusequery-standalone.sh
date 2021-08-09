#!/bin/bash
# Copyright 2020-2021 The Datafuse Authors.
# SPDX-License-Identifier: Apache-2.0.

SCRIPT_PATH="$(cd "$(dirname "$0")" >/dev/null 2>&1 && pwd)"
cd "$SCRIPT_PATH/../.." || exit

killall fuse-store
killall fuse-query
sleep 1

BIN=${1:-debug}

echo 'Start FuseStore...'
nohup target/${BIN}/fuse-store --single true &
echo "Waiting on fuse-store 10 seconds..."
python3 scripts/ci/wait_tcp.py --timeout 5 --port 9191


echo 'Start FuseQuery...'
nohup target/${BIN}/fuse-query -c scripts/deploy/config/fusequery-node-1.toml &
echo "Waiting on fuse-query 10 seconds..."
python3 scripts/ci/wait_tcp.py --timeout 5 --port 3307
