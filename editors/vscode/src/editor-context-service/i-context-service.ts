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

import { commands, ExtensionContext, Position, TextDocument } from 'vscode';
import type IDisposable from "../IDisposable";

interface IContextService extends IDisposable {
    onActivate(context: ExtensionContext): void;

    /**
     * handler of onDidChangeActiveTextEditor
     * implement this method to handle that event to update context state
     */
    onDidChangeActiveTextEditor(document: TextDocument, cursorPos: Position): void;
    /**
     * handler of onDidChangeTextEditorSelection
     * implement this method to handle that event to update context state
     */
    onDidChangeTextEditorSelection(document: TextDocument, cursorPos: Position): void;
}

export abstract class AbsContextService implements IContextService {
    public abstract readonly contextName: string;

    /**
     * activate context service
     * @param context ExtensionContext
     */
    public abstract onActivate(context: ExtensionContext): void;
    public abstract dispose(): void;

    /**
     * default handler of onDidChangeActiveTextEditor, do nothing.
     * override this method to handle that event to update context state.
     */
    public abstract onDidChangeActiveTextEditor(document: TextDocument, cursorPos: Position): void;

    /**
    * default handler of onDidChangeTextEditorSelection, do nothing.
    * override this method to handle that event to update context state.
    */
    public abstract onDidChangeTextEditorSelection(document: TextDocument, cursorPos: Position): void;

    /**
     * set state of context
     */
    protected setState(state: any) {
        commands.executeCommand('setContext', this.contextName, state);
    }
}