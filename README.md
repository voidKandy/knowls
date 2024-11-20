# `espx_ls` 
> Short for [espionox](https://github.com/voidKandy/espionox) language server

`esp_ls` utilizes the [language server protocol](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/) to provide an interface for interacting with language models. This is done through a command line tool like syntax within the comments of your code. The syntax is structured as follows: 
`<languge comment syntax> <command> <agent> option<args>`

## Configuration
Eventually this will be automated, but in order to get the LSP to attach within one of your projects, you must create the directory `$HOME/.espx`, and place a config file at `$HOME/.espx/config.toml`. The `[model]` section is required, all other sections are optional.
#### [model] 
* provider: either `Anthropic` or `OpenAi`
* api_key: an api for the corresponding provider

**Example:**
```toml
[model]
provider = "Anthropic"
api_key = "your_api_key_here"
```

#### [database] 
Include this if you would like to use the [surrealdb](https://github.com/surrealdb/surrealdb) integration. This will **CREATE** a database instance in the root of your project in the `.espx-ls` directory and does not require that you set one up yourself. 
* namespace: the namespace of the database
* database: the name of the database
* user: username for database access
* pass: password for database access

**Example:**
```toml
[database]
namespace = "espx"
database = "espx"
user = "root"
pass = "root"
```
#### [agents]
For defining custom agents, you can also adjust the global agent by using it's character (`_`).

**Example:**
```toml
[agents]
  [agents.c]
  [agents.b]
    sys_prompt = "Your prompt for agent B"
```
> **Note:** In the example above, agent `c` will use the default assistant prompt, while agent `b` will utilize the specified system prompt. Both agents can be accessed like any other agent. For instance, to prompt the model in agent `c`, you would use: `@c your prompt.`


## Relay or Headless
There are two options for binaries to run when your editor's LSP client gets upand running: **Headless** and **Relay**. Before building either of these, make sure to go into `build.sh` and change the `TARGET_BIN` to a folder within your `$PATH`, this should be suffixed by the name of the `espx-ls` binary.
> For example, if you would like to put the resulting binary directly into your `$HOME/.local/bin`, make sure to set `TARGET_BIN` to `$HOME/.local/bin/espx-ls`.

Once you have made the required changes to `build.sh`, build either **headless** or **relay** by running: 
```bash
./build.sh <headless/relay>
```
### Headless
If you build the **headless** binary this means your LSP client will boot up the server containing your agents, documents and database connection. If you quit your editor, the server will also quit.

### Relay
If you build the **relay** binary, it expects a server to be running on your computer. This will simply *relay* LSP JSON RPC messages from your editor to the process running the server. So, you will need to run the `tui` binary in the `bin` folder to use alongside the relay.
```bash
cargo run --bin tui
```

> **IMPORTANT**:
> Check the [Editor Setup](#neovim-setup), you will notice that NeoVim looks for a file called `.espx`. This is temporary, but until I find out a language agnostic way to find the root of a project this file will need to be included in the root of your project if you want the LSP to connect.
## Usage
### Agents 
> Additional agents can be added manually by the user, followed up in the [configuration section](#configuration).

A agent is a context that is associated with a character. By default, there are two agents: 
1. **Global**(`_`)
  * Is initialized with just the default assistant system prompt
  * Only changes when user either explicitly adds content or prompts 
2. **Document**(`^`)
  * Is initialized with the default assistant system prompt, and the entirety of the document it is associated with.
  * Will change based on user's current document
    >**NOTE:** Using the push command (`+`) with the document agent is redundant because the entirety of a current document is already included in the model's context

  

### Commands
Currently there are two supported commands: 
1. **Prompt**(`@`)
  * **Description**: Use this command to prompt the model within the specified agent.
  * **Usage**: `@<agent> your prompt here`
  * **Example**: 
    ```rust
    // @_ How do I read from stdin?
    ```
2. **Push**(`+`)
  * **Description**: This command allows you to push a block of code into the model's context within the specified agent.
  * **Usage**: `+<agent>` (At the top of a block of code)
  * **Example**: 
    ```rust
    // +_
    pub struct SomeStruct {
      id: Uuid,
      content: String,
    }
    impl SomeStruct {
      fn new() -> Self {
        Self {
          id: UUid::new_v4(),
          content: String::new(),
        }
      }
    }

    pub struct OtherStruct;
    ```
    >**NOTE:** In the example above, only the `SomeStruct` definition and its `impl` block will be pushed to the model's context. This is because the Push command only includes the code block that immediately follows it. Code blocks are separated by blank lines.


# IDE setup
As of right now I only know how to get this working in NeoVim ¯\_(ツ)\_/¯

I'm working on a VsCode integration, if you would like to help feel free to contact me at [voidkandy@gmail.com](mailto:voidkandy@gmail.com)

### NeoVim Setup  

First, manually compile espx-ls and put it in your `PATH`. Assuming you have `lspconfig`, the below snippet should work. You can set `filetypes` to any filetypes you want.

```lua
local lsp_config = require 'lspconfig'
local configs = require 'lspconfig.configs'


if not configs.espx_ls then
  configs.espx_ls = {
    default_config = {
      name = 'espx_ls',
      autostart = true,
      cmd = { 'espx-ls' },
      filetypes = { 'text', 'rust' },
      root_dir = function()
        -- this markerfile should be put in the root directory of any project you want to use with this LSP
        return vim.fs.dirname(vim.fs.find({ '.espx' }, { upward = true })[1])
      end
    },
  }
end

lsp_config.espx_ls.setup {}
```

As you can see above, as of right now, `espx-ls` requires that you have a `espx-ls.toml` in your project's root in order for the LSP to know to attach.

#### window/showMessage
> This is fully optional, but creates a slightly nicer user experience

Ensure your config has a way to handle `window/showMessage` requests from an LSP. NeoVim does support this out of the box, but in a way that isn't very condusive to great UX.
If you have the `notify` plugin and this snippet of code in your config should do the trick:

```lua
vim.lsp.handlers['window/showMessage'] = function(_, result, ctx)
  local notify = require 'notify'
  notify.setup {
    background_colour = '#000000',
    render = 'wrapped-compact',
    timeoute = 100,
  }
  notify(result.message)
end
```

From here you should be good to go!

If you have any questions, suggestions, or anything at all feel free to reach out to me at [voidkandy@gmail.com](mailto:voidkandy@gmail.com)


> Thanks to thePrimagen for making his [HTMX LSP](https://github.com/ThePrimeagen/htmx-lsp) Open Source so I could fork it and build it into this :D
