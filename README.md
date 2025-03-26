# `knowls`
`knowls` is a [language server](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/) that allows users to embed *knowledge* into their development environment.  


## TUI
The main `knowls` application is a TUI. This is where knowledge sources can be managed and viewed. The TUI must be running in order for the LSP to run. If the tui application is not running when the LSP connects to your editor, it will **not work**.

## LSP
In the configuration file of the tui, you specify which prefix is required for the lsp to trigger a completion/hover. By default this prefix is `$$@`.
For example, if you have a knowledge source called `LspWiki`, you would need to type (or have the LSP complete) `$$@LspWiki`in order to utilie hover and gotodefinition features.
Provides the following basic functionality:
* **Hover** - When a knowledge source is hovered a hover shows it's text content
* **Completions** - as long as your cursor is over defined prefix appended with any or none of the characters in a knowledge source's name, the lsp will suggest knowledge sources that match what you've typed
* **Goto Definition** - Going to the definition over a knowledge source will take you to a buffer where you can edit the knowledge source.

### Checklist
- [x] Hover
- [x] Completions
- [x] TUI configuration
- [ ] Knowledge source definition through TUI Config
- [ ] WebPage knowledge sources
- [ ] Edits made to gotodef buffer persist

