// From https://github.com/yzhang-gh/vscode-markdown/ , under the following license:
// 
// MIT License

// Copyright (c) 2017 张宇

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
// 

'use strict'

import type IDisposable from "../IDisposable";
import { ExtensionContext, window } from 'vscode';
import { AbsContextService } from "./i-context-service";
import { ContextServiceEditorInList } from "./context-service-in-list";
// import { ContextServiceEditorInFencedCodeBlock } from "./context-service-in-fenced-code-block";
// import { ContextServiceEditorInMathEn } from "./context-service-in-math-env";

export class ContextServiceManager implements IDisposable {
    private readonly contextServices: Array<AbsContextService> = [];

    public constructor() {
        // push context services
        this.contextServices.push(new ContextServiceEditorInList());
        // this.contextServices.push(new ContextServiceEditorInFencedCodeBlock());
        // this.contextServices.push(new ContextServiceEditorInMathEn());
    }

    public activate(context: ExtensionContext) {
        for (const service of this.contextServices) {
            service.onActivate(context);
        }
        // subscribe update handler for context
        context.subscriptions.push(
            window.onDidChangeActiveTextEditor(() => this.onDidChangeActiveTextEditor()),
            window.onDidChangeTextEditorSelection(() => this.onDidChangeTextEditorSelection())
        );
        // initialize context state
        this.onDidChangeActiveTextEditor();
    }

    public dispose(): void {
        while (this.contextServices.length > 0) {
            const service = this.contextServices.pop();
            service!.dispose();
        }
    }

    private onDidChangeActiveTextEditor() {
        const editor = window.activeTextEditor;
        if (editor === undefined) {
            return;
        }

        const cursorPos = editor.selection.start;
        const document = editor.document;

        for (const service of this.contextServices) {
            service.onDidChangeActiveTextEditor(document, cursorPos);
        }
    }

    private onDidChangeTextEditorSelection() {
        const editor = window.activeTextEditor;
        if (editor === undefined) {
            return;
        }

        const cursorPos = editor.selection.start;
        const document = editor.document;

        for (const service of this.contextServices) {
            service.onDidChangeTextEditorSelection(document, cursorPos);
        }
    }
}

export const contextServiceManager = new ContextServiceManager();