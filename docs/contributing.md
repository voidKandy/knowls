# Espx-LS Contributing

## Builds
Build either of these by using the `./build.sh` script. Before you do, make sure to set `$TARGET_BIN` in `./build.sh`
There are currently two builds of `espx-ls`:
* **Headless** - `headless`
  > Contains all logic needed for the lsp to work without the gui running, it creates box ends of the unix socket channel every time the client attaches.
* **Relay** - `relay`
  > Contains only the logic necessary for relaying messages from the lsp client to the running gui, lsp server/gui needs to be started by user manually.

## Gettin'r goin' (NVIM)
Once you've built either of the builds and made it accessible through your `$PATH`, NeoVim should run the LSP as long as you've done the needed [setup steps](/README.md#neovim-setup).
If you've built the `headless` binary, the LSP should just attach and work as expected.
If you've built the `relay` binary, you will need to run the GUI by running the binary in `bin/gui.rs`, once the GUI is running it should attach to your running client
> **_NOTE_**:
  This will be indicated by a green checkmark in the top right corner of the GUI

## Developing the UI
For accurate compiler & linting errors make sure to de-comment "gui" from `default` in the `[features]` section of `Cargo.toml`
DO NOT forget to re-comment when you are done
