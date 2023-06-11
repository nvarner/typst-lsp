# Typst LSP VS Code Extension

A brand-new VS Code extension for Typst.

## Features

-   Syntax highlighting, error reporting, code completion, and function signature
    help
-   Compiles to PDF on save (configurable to as-you-type, or can be disabled)

## Usage Tips

-   This extension compiles to PDF, but it doesn't have a PDF viewer yet. To view
    the output as you work, install a PDF viewer extension, such as
    `vscode-pdf`.
-   To configure when PDFs are compiled:
    1. Open settings
        - File -> Preferences -> Settings (Linux, Windows)
        - Code -> Preferences -> Settings (Mac)
    2. Search for "Typst Export PDF"
    3. Change the Export PDF setting
        - `onSave` makes a PDF after saving the Typst file
        - `onType` makes PDF files live, as you type
        - `never` disables PDF compilation

## Technical

The extension uses [Typst LSP](https://github.com/nvarner/typst-lsp) on the
backend.

Both `language-configuration.json` and `typst.tmLanguage.json` are from
Typst. See [the source here](https://github.com/typst/typst/tree/main/tools/support).
