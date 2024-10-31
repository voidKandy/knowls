if [ -z "$1" ]; then
    echo "Usage: $0 <binary_name>"
    exit 1
fi

BINARY_NAME=$1
TARGET_BIN=~/mybin/espx-ls


if [[ "$BINARY_NAME" == *"gui"* ]]; then
    echo "You should not be building the GUI in this way. This script is for building other components."
    echo "(headless/relay)"
    exit 1
fi

# Build the specified binary
cargo build --bin "$BINARY_NAME"

# Move the built binary to the desired location
mv target/debug/$BINARY_NAME $TARGET_BIN

echo "saved $BINARY_NAME as espx-ls binary"
