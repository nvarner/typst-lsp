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
    2. Run `npm compile` to build the extension
4. Run through the development cycle once to initialize and test everything

### Development cycle
1. Make any changes
2. Run `cargo install`; at present, the VS Code extension just invokes the
    `typst-lsp` command to start the LSP, and this command will compile and
    replace that binary with the latest version
    - If modifying the extension, keep `npm watch` running, or `npm compile`
        after changes
3. Press `ctrl+F5` to launch the "Extension Development Host"; if it's already
    running, invoke "Developer: Reload Window" from the command palette in the
    Extension Development Host
4. Within the Extension Development Host, the extension will be active and ready
    for testing
