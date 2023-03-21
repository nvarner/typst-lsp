"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.deactivate = exports.activate = void 0;
const node_1 = require("vscode-languageclient/node");
let client;
function activate(_context) {
    const serverCommand = "./typst-lsp";
    const serverOptions = {
        run: { command: serverCommand },
        debug: { command: serverCommand }
    };
    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "typst" }]
    };
    client = new node_1.LanguageClient("typst-lsp", "Typst Language Server", serverOptions, clientOptions);
    client.start();
}
exports.activate = activate;
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
exports.deactivate = deactivate;
//# sourceMappingURL=extension.js.map