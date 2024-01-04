# Typst LSP

A language server for [Typst](https://typst.app/).

## Features

- Syntax highlighting, error reporting, code completion, and function signature
    help
- Compiles to PDF on save (configurable to as-you-type, or can be disabled)
- Experimental formatting using [typstfmt](https://github.com/astrale-sharp/typstfmt)

This repo consists of:

- an LSP server, written in Rust
- [a corresponding VS Code(ium) extension](https://github.com/nvarner/typst-lsp/tree/master/editors/vscode).
The extension is available on the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=nvarner.typst-lsp)
and [OpenVSX](https://open-vsx.org/extension/nvarner/typst-lsp).

## Near future goals

- Improved preview (e.g. built-in PDF viewer, render to image for speed)
- Support for more editors

## Building from source

### Prerequisites
Install [Rust](https://www.rust-lang.org/), which comes with `cargo`. Among other things, `cargo` is the build tool used for Rust projects.

### Building
```sh
cargo build --release
```
Cargo will download and compile Rust dependencies as needed. The `--release` flag produces and optimized binary. The resulting executable will be at `target/release/typst-lsp` (with `.exe` extension on Windows).

### Cargo features
Cargo features allow you to customize the build by enabling chunks of code at compile time.

We need an HTTP client to download Typst packages. The client needs a TLS
implementation; by default, [Rustls](https://github.com/rustls/rustls) is used. If you would like to disable or change this, the following Cargo features are available:

- `remote-packages` (default): use an HTTP client to download Typst packages
- `rustls-tls` (default): use Rustls for TLS
- `native-tls`: use your platform's TLS implementation

For example, the following command will build with `native-tls`:
```sh
cargo build --release --no-default-features --features remote-packages,fontconfig,native-tls
```

For Linux, `native-tls` means OpenSSL. You will need to install its headers to compile with `native-tls`.

Additionally, the `fontconfig` feature is used to enable minimal support for `fontconfig`. This is necessary
for the LSP to detect fonts in certain Linux distributions, such as NixOS, and is thus enabled by default.
If this causes any problems for your distribution, you can disable support for `fontconfig` by not enabling that feature,
such as by compiling with the flags `--no-default-features --features remote-packages,rustls-tls`.

**NOTE:** If the LSP fails to find your fonts through `fontconfig`, try setting (when running the LSP) the `FONTCONFIG_FILE`
environment variable to the absolute path of the fontconfig configuration file specifying your desired font paths. This can happen
when using VSCode through Flatpak on distributions such as NixOS, and can lead to some fonts not being detected when compiling your
Typst documents. To set the environment variable through Flatpak, you may either use Flatseal or run a command such as below
(assuming a `--user` installation) - make sure to replace the path after `FONTCONFIG_FILE` with the appropriate one for your system:

```sh
# For VSCode:
flatpak override --user --env=FONTCONFIG_FILE=$HOME/.config/fontconfig/conf.d/configfilenamehere.conf com.visualstudio.code

# For VSCodium:
flatpak override --user --env=FONTCONFIG_FILE=$HOME/.config/fontconfig/conf.d/configfilenamehere.conf com.vscodium.codium
```

### Bumping the Typst version
**Warning:** at time of writing, the Typst API has changed in every release. Until the language is stable, you may need Rust knowledge to address changes in Typst to successfully compile against a new version of Typst.

You will need to modify the Typst dependencies in `Cargo.toml`. These are at the top of the `[dependencies]` section, have names starting with `typst`, and reference `git = "https://github.com/typst/typst.git"`.

If you want to compile against a versioned release of Typst, change the `tag`s to the tag for your desired release. Usually, the tags are named `vX.X.X`.

If you want to compile against a commit that hasn't been released, replace the `tag`s with `rev`s, and set their values to the Git commit hash. Commit hashes are hexadecimal strings, and have long and short versions that both name the commit. Among other places, a commit's hash can be found at the end of the GitHub URL for that commit.

## Development guide

### Prerequisites

Install:

- [Rust](https://www.rust-lang.org/) for the LSP itself
- [Rust Analyzer](https://rust-analyzer.github.io/) an extension for Rust LSP for VS Code
- [node](https://nodejs.org/en) for the VS Code extension; it may be easiest to
    install via [fnm](https://github.com/Schniz/fnm)

### First time setup

1. Clone this repository locally
2. Open it in VS Code; it's needed to run the extension
3. In the `editors/vscode` subdirectory:
    1. Run `npm install` to install extension dependencies
    2. Run `npm run compile` to build the extension
4. Run through the development cycle once to initialize and test everything
5. (Optional: install the dev version of the extension): Press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>,
and choose `Developer: Install Extension from Location...` and choose
the directory for the extension, `editors/vscode/`. There will not be any messages, but
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
    - If prompted, choose "Run Extension"
4. Within the Extension Development Host, the extension will be active and ready
    for testing

#### Tracing with Jaeger

[Jaeger](https://www.jaegertracing.io/) is a tool to visualize tracing data. It
shows spans (e.g. a span corresponds to each time a file is opened, each time we
calculate semantic tokens, etc.) and associated data (e.g. the URL of the file
opened), which provides timing and debugging data.

By default, the LSP does not send data to Jaeger. To enable it:

1. Launch the Jaeger server. The [`opentelemetry_jaeger`](https://docs.rs/opentelemetry-jaeger/latest/opentelemetry_jaeger/)
    crate recommends the following:
    ```
    $ docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest
    ```
2. Compile the LSP with the `jaeger` feature enabled. In the terminal, run:
    ```
    $ cargo build --features jaeger
    ```
    In VS Code, you can use the "Run Extension [Jaeger]" task to launch the
    extension with Jaeger support.
3. Run the LSP, then eventually close it.
4. From Jaeger, search for traces. It may be best to restrict the search to
    traces with a minimum length, such as 2 seconds, to hide smaller traces that
    come from the task sending data to Jaeger.

## Installation guide

### Visual Studio Code

- Install it from [Marketplace](https://marketplace.visualstudio.com/items?itemName=nvarner.typst-lsp).

### Neovim

#### Basic setup

> Prerequisites: [`mason-lspconfig.nvim`](https://github.com/williamboman/mason-lspconfig.nvim), [`mason.nvim`](https://github.com/williamboman/mason.nvim) and [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig) (Optional for advanced users, but required for this guide).

1. Run `MasonInstall typst-lsp`.
2. Edit your `init.lua` settings (For more details, you may consult [server_configurations.md#typst_lsp](https://github.com/neovim/nvim-lspconfig/blob/master/doc/server_configurations.md#typst_lsp)):
```lua
require'lspconfig'.typst_lsp.setup{
	settings = {
		exportPdf = "onType" -- Choose onType, onSave or never.
        -- serverPath = "" -- Normally, there is no need to uncomment it.
	}
}
```
3. You may also install [`typst.vim`](https://github.com/kaarmu/typst.vim) for more capabilities in nvim.

#### Additional steps for `coc.nvim` users

Run `CocConfig` to edit the settings so that `coc.nvim` could offer functions such as auto-completion:
```
{
"languageserver": {
    "typst": {
        "command": "typst-lsp",
        "filetypes": ["typst"]
        }
    }
}
```


### Sublime Text

Follow the [configuration instructions](https://lsp.sublimetext.io/language_servers/#typst) for the LSP plugin for Sublime Text.
