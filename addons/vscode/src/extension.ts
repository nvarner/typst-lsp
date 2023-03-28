import { type ExtensionContext, workspace, window, commands } from "vscode";
import * as path from "path";
import * as fs from "fs";

import {
    LanguageClient,
    DidChangeConfigurationNotification,
    type LanguageClientOptions,
    type ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined = undefined;

export function activate(context: ExtensionContext): Promise<void> {
    const serverCommand = getServer();
    const serverOptions: ServerOptions = {
        run: { command: serverCommand },
        debug: { command: serverCommand },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "typst" }],
    };

    client = new LanguageClient("typst-lsp", "Typst Language Server", serverOptions, clientOptions);

    workspace.onDidChangeConfiguration(async (_) => {
        await client?.sendNotification(DidChangeConfigurationNotification.type, {
            settings: workspace.getConfiguration("typst-lsp"),
        });
    }, null);

    context.subscriptions.push(
        commands.registerCommand("typst-lsp.exportCurrentPdf", commandExportCurrentPdf)
    );

    return client.start();
}

export function deactivate(): Promise<void> | undefined {
    return client?.stop();
}

function getServer(): string {
    const windows = process.platform === "win32";
    const suffix = windows ? ".exe" : "";
    const binaryName = "typst-lsp" + suffix;

    const bundledPath = path.resolve(__dirname, binaryName);

    if (fileExists(bundledPath)) {
        return bundledPath;
    }

    return binaryName;
}

function fileExists(path: string): boolean {
    try {
        fs.accessSync(path);
        return true;
    } catch (error) {
        return false;
    }
}

async function commandExportCurrentPdf() {
    const activeEditor = window.activeTextEditor;
    if (!activeEditor) {
        return;
    }
    await commands.executeCommand("typst.doPdfExport", activeEditor.document.uri);
}
