# typst-lsp

A brand-new language server for Typst. It is still in development and not ready
for regular use.

Currently, it has an LSP component and a corresponding VS Code(ium) extension.

## Current features

- Syntax highlighting
- Compiles Typst into a PDF as you type
- Compile errors appear in the Output pane

## Near future goals

- Improved preview
- Completion
- Improved development experience
- Package extension for users
- Create extensions for more editors

## Development guide

### Prerequisites

Install:

- [Rust](https://www.rust-lang.org/) for the LSP itself
- [node](https://nodejs.org/en) for the VS Code extension; it may be easiest to
    install via [nvm](https://github.com/nvm-sh/nvm)

### First time setup

1. Clone this repository locally
2. Open it in VS Code; it's needed to run the extension
3. In the `addons/vscode` subdirectory:
    1. Run `npm install` to install extension dependencies
    2. Run `npm run compile` to build the extension
4. Run through the development cycle once to initialize and test everything
5. If you want to install the dev version of the extension: After finishing the
last step in Development Cycle, then press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>,
and choose `Developer: Install Extension from Location...` and choose the
the directory for the extension: `addons/vscode/`. There will not be any messages, but
the extension can be found in the Extensions `@installed` list.

### Development cycle

1. Make any changes
2. Run `cargo install --path .`; at present, the VS Code extension just invokes
    the `typst-lsp` command to start the LSP, and this command will compile and
    replace that binary with the latest version
    - If modifying the extension, keep `npm run watch` running, or `npm run compile`
        after changes
3. Press <kbd>Ctrl</kbd>+<kbd>F5</kbd> to launch the "Extension Development Host"; if it's already
    running, invoke "Developer: Reload Window" from the command palette in the
    Extension Development Host
4. Within the Extension Development Host, the extension will be active and ready
    for testing
