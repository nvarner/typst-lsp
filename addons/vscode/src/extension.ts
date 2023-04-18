import {
    type ExtensionContext,
    workspace,
    window,
    commands,
    ViewColumn,
    Uri,
    WorkspaceConfiguration,
} from "vscode";
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
    const config = workspace.getConfiguration("typst-lsp");
    const serverCommand = getServer(config);
    const serverOptions: ServerOptions = {
        run: { command: serverCommand, options: { env: { RUST_BACKTRACE: "1" } } },
        debug: { command: serverCommand, options: { env: { RUST_BACKTRACE: "1" } } },
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
    context.subscriptions.push(commands.registerCommand("typst-lsp.showPdf", commandShowPdf));

    return client.start();
}

export function deactivate(): Promise<void> | undefined {
    return client?.stop();
}

function getServer(conf: WorkspaceConfiguration): string {
    const pathInConfig = conf.get<string | null>("serverPath");
    if (pathInConfig !== undefined && pathInConfig !== null && fileExists(pathInConfig)) {
        return pathInConfig;
    }
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

async function commandExportCurrentPdf(): Promise<void> {
    const activeEditor = window.activeTextEditor;
    if (activeEditor === undefined) {
        return;
    }

    const uri = activeEditor.document.uri.toString();

    await client?.sendRequest("workspace/executeCommand", {
        command: "typst-lsp.doPdfExport",
        arguments: [uri],
    });
}

/**
 * Implements the functionality for the 'Show PDF' button shown in the editor title
 * if a `.typ` file is opened.
 *
 */
async function commandShowPdf(): Promise<void> {
    const activeEditor = window.activeTextEditor;
    if (activeEditor === undefined) {
        return;
    }

    const uri = activeEditor.document.uri;
    // change the file extension to `.pdf` as we want to open the pdf file
    // and not the currently opened `.typ` file.
    const n = uri.toString().lastIndexOf(".");
    const pdf_uri = Uri.parse(uri.toString().slice(0, n) + ".pdf");

    try {
        await workspace.fs.stat(pdf_uri);
    } catch {
        // only create pdf if it does not exist yet
        await commandExportCurrentPdf();
    } finally {
        // here we can be sure that the pdf exists
        await commands.executeCommand("vscode.open", pdf_uri, ViewColumn.Beside);
    }
}
