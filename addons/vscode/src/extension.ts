import { type ExtensionContext } from "vscode";
import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";

import {
    LanguageClient,
    type LanguageClientOptions,
    type ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(_context: ExtensionContext) {
    const serverCommand = getServer();
    if (!serverCommand) return;

    const serverOptions: ServerOptions = {
        run: { command: serverCommand },
        debug: { command: serverCommand },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "typst" }],
    };

    client = new LanguageClient("typst-lsp", "Typst Language Server", serverOptions, clientOptions);

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

function getServer(): string {
    const windows = process.platform === "win32";
    const suffix = windows ? ".exe" : "";

    const bundledPath = path.resolve(__dirname, "typst-lsp" + suffix);

    if (fileExists(bundledPath)) {
        return bundledPath;
    }

    return "typst-lsp" + suffix;
}

function fileExists(path: string) {
    try {
        fs.accessSync(path);
        return true;
    } catch (error) {
        return false;
    }
}
