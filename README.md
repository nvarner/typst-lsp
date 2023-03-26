# Typst LSP

A brand-new language server for [Typst](https://typst.app/).

## Features

- Syntax highlighting, error reporting, code completion, and function signature
    help
- Compiles to PDF on save (configurable to as-you-type, or can be disabled)

This repo consists of:

- an LSP server, written in Rust
- [a corresponding VS Code(ium) extension](https://github.com/nvarner/typst-lsp/tree/master/addons/vscode).
The extension is available on the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=nvarner.typst-lsp)
and [OpenVSX](https://open-vsx.org/extension/nvarner/typst-lsp).

## Near future goals

- Improved preview (e.g. built-in PDF viewer, render to image for speed)
- Support for more editors

## Development guide

### Prerequisites

Install:

- [Rust](https://www.rust-lang.org/) for the LSP itself
- [node](https://nodejs.org/en) for the VS Code extension; it may be easiest to
    install via [fnm](https://github.com/Schniz/fnm)

### First time setup

1. Clone this repository locally
2. Open it in VS Code; it's needed to run the extension
3. In the `addons/vscode` subdirectory:
    1. Run `npm install` to install extension dependencies
    2. Run `npm run compile` to build the extension
4. Run through the development cycle once to initialize and test everything
5. (Optional: install the dev version of the extension): Press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>,
and choose `Developer: Install Extension from Location...` and choose
the directory for the extension, `addons/vscode/`. There will not be any messages, but
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
