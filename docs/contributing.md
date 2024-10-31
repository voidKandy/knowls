# Espx-LS Contributing

## Builds
There are currently two builds of `espx-ls`:
  ### **Headless** - `headless`
  Contains all logic needed for the lsp to work without the gui running, it creates box ends of the unix socket channel every time the client attaches.
  ### **Relay** - `relay`
  Contains only the logic necessary for relaying messages from the lsp client to the running gui, lsp server/gui needs to be started by user manually.
Build either of these by using the `./build.sh` script.
> **_Note_**:
  Before you do, make sure to set `$TARGET_BIN` in `./build.sh`

