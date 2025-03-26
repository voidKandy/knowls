# LLM Removal plan
I have decided that LLMs should just be removed completely from this project.
Here is an outline of what this lsp should *actually* do.

## Knowledge LSP
We will stick to the current use of an external server with a relay on the client side. But this time, the server will store **external knowledge sources**, these will provide Hover and Gotodef interactions.

### External Knowledge
To start, add support for:
* Markdown documents
* PDFs
* Youtube videos (transcribe)
* Webpages (as Urls)
When the server is given any of these it will do the following:
1. Process the knowledge into a **markdown document**
2. Store the knowledge in the surreal DB

Once Knowledge has been added to the database, providing hovers and gotodefs should be easy.
**Hovers** will simply display the knowledge source in a hover menu
**GotoDef** will do two things:
1. cache the knowledge source in a temp file (if one for the knowledge source does not exist yet)
2. direct the user to the temp file
> There is potential here to allow users to edit the knowledge source this way, but hold off on this for now

## What needs to be done?
- [x] `espionox` needs to be removed as a dependency
- [x] `agents` needs to be removed wherever it is used
- [ ] lsp client should *no longer relay* lsp messages to the Knowledge Management Application
- [ ] The Knowledge Management Application should handle all knowledge 
- [ ] *knowledge processing* needs to be implemented
- [ ] lsp functionality needs to be reworked

## Important considerations
what is currently called `interacts` needs to be reworked to allow users to define how they can access their knowledge from within their documents
