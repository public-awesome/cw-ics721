#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd ) # source: https://stackoverflow.com/a/246128/3437868
WORKSPACE_DIR=$(cd $SCRIPT_DIR/.. && pwd)
echo "sudo rm -rf $WORKSPACE_DIR/artifacts"
sudo rm -rf $WORKSPACE_DIR/artifacts
$WORKSPACE_DIR/ts-relayer-tests/build.sh
echo "running tests..."
go test $SCRIPT_DIR