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
import * as child_process from "child_process";

import {
    LanguageClient,
    type LanguageClientOptions,
    type ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined = undefined;

export function activate(context: ExtensionContext): Promise<void> {
    return startClient(context).catch((e) => {
        void window.showErrorMessage(`Failed to activate typst-lsp: ${e}`);
        throw e;
    });
}

async function startClient(context: ExtensionContext): Promise<void> {
    const config = workspace.getConfiguration("typst-lsp");
    const serverCommand = getServer(config);
    const run = {
        command: serverCommand,
        options: { env: Object.assign({}, process.env, { RUST_BACKTRACE: "1" }) },
    };
    const serverOptions: ServerOptions = {
        run,
        debug: run,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "typst" }],
        initializationOptions: config,
    };

    client = new LanguageClient("typst-lsp", "Typst Language Server", serverOptions, clientOptions);

    context.subscriptions.push(
        commands.registerCommand("typst-lsp.exportCurrentPdf", commandExportCurrentPdf)
    );
    context.subscriptions.push(
        commands.registerCommand("typst-lsp.pinMainToCurrent", () => commandPinMain(true))
    );
    context.subscriptions.push(
        commands.registerCommand("typst-lsp.unpinMain", () => commandPinMain(false))
    );
    context.subscriptions.push(commands.registerCommand("typst-lsp.showPdf", commandShowPdf));
    context.subscriptions.push(commands.registerCommand("typst-lsp.clearCache", commandClearCache));

    return client.start();
}

export function deactivate(): Promise<void> | undefined {
    return client?.stop();
}

function getServer(conf: WorkspaceConfiguration): string {
    const pathInConfig = conf.get<string | null>("serverPath");
    if (pathInConfig !== undefined && pathInConfig !== null && pathInConfig !== "") {
        const validation = validateServer(pathInConfig);
        if (!validation.valid) {
            throw new Error(
                `\`typst-lsp.serverPath\` (${pathInConfig}) does not point to a valid typst-lsp binary:\n${validation.message}`
            );
        }
        return pathInConfig;
    }
    const windows = process.platform === "win32";
    const suffix = windows ? ".exe" : "";
    const binaryName = "typst-lsp" + suffix;

    const bundledPath = path.resolve(__dirname, binaryName);

    const bundledValidation = validateServer(bundledPath);
    if (bundledValidation.valid) {
        return bundledPath;
    }

    const binaryValidation = validateServer(binaryName);
    if (binaryValidation.valid) {
        return binaryName;
    }

    throw new Error(
        `Could not find a valid typst-lsp binary.\nBundled: ${bundledValidation.message}\nIn PATH: ${binaryValidation.message}`
    );
}

function validateServer(path: string): { valid: true } | { valid: false; message: string } {
    try {
        const result = child_process.spawnSync(path);
        if (result.status === 0) {
            return { valid: true };
        } else {
            const statusMessage = result.status !== null ? [`return status: ${result.status}`] : [];
            const errorMessage =
                result.error?.message !== undefined ? [`error: ${result.error.message}`] : [];
            const messages = [statusMessage, errorMessage];
            const messageSuffix =
                messages.length !== 0 ? `:\n\t${messages.flat().join("\n\t")}` : "";
            const message = `Failed to launch '${path}'${messageSuffix}`;
            return { valid: false, message };
        }
    } catch (e) {
        if (e instanceof Error) {
            return { valid: false, message: `Failed to launch '${path}': ${e.message}` };
        } else {
            return { valid: false, message: `Failed to launch '${path}': ${JSON.stringify(e)}` };
        }
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

async function commandPinMain(isPin: boolean): Promise<void> {
    if (!isPin) {
        await client?.sendRequest("workspace/executeCommand", {
            command: "typst-lsp.doPinMain",
            arguments: ["detached"],
        });
    }

    const activeEditor = window.activeTextEditor;
    if (activeEditor === undefined) {
        return;
    }

    const uri = activeEditor.document.uri.toString();

    await client?.sendRequest("workspace/executeCommand", {
        command: "typst-lsp.doPinMain",
        arguments: [uri],
    });
}

/**
 * Implements the functionality for the 'Show PDF' button shown in the editor title
 * if a `.typ` file is opened.
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

async function commandClearCache(): Promise<void> {
    const activeEditor = window.activeTextEditor;
    if (activeEditor === undefined) {
        return;
    }

    const uri = activeEditor.document.uri.toString();

    await client?.sendRequest("workspace/executeCommand", {
        command: "typst-lsp.doClearCache",
        arguments: [uri],
    });
}
