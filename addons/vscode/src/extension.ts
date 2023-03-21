import { ExtensionContext } from "vscode";

import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions
} from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(_context: ExtensionContext) {
    const serverCommand = "./typst-lsp";

    const serverOptions: ServerOptions = {
        run: { command: serverCommand },
        debug: { command: serverCommand }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "typst" }]
    };

    client = new LanguageClient(
        "typst-lsp",
        "Typst Language Server",
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
