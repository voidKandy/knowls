TARGET_BIN=~/mybin/knowls



# Build the specified binary
cargo build --bin "server"

# Move the built binary to the desired location
mv ../target/debug/server $TARGET_BIN

echo "saved server as knowls binary"
