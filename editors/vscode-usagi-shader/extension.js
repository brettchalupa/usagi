const childProcess = require("child_process");
const path = require("path");
const vscode = require("vscode");

const LANGUAGE_ID = "usagi-shader";
const GENERATED_GLSL_METHOD = "usagi/generatedGlsl";
const CONFIG_SECTION = "usagi.shader";

function activate(context) {
  const client = new UsagiShaderClient(context);
  context.subscriptions.push(client);

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => client.onDidOpen(document)),
    vscode.workspace.onDidChangeTextDocument((event) => client.onDidChange(event.document)),
    vscode.workspace.onDidCloseTextDocument((document) => client.onDidClose(document)),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration(CONFIG_SECTION)) {
        client.restart();
      }
    }),
    vscode.languages.registerCompletionItemProvider(
      { language: LANGUAGE_ID },
      {
        provideCompletionItems: async (document, position) => {
          await client.ensureDocument(document);
          const result = await client.request("textDocument/completion", textDocumentParams(document, position));
          return toCompletionList(result);
        },
      },
      ".",
      "_",
      "u",
    ),
    vscode.languages.registerHoverProvider(
      { language: LANGUAGE_ID },
      {
        provideHover: async (document, position) => {
          await client.ensureDocument(document);
          const result = await client.request("textDocument/hover", textDocumentParams(document, position));
          return toHover(result);
        },
      },
    ),
    vscode.languages.registerSignatureHelpProvider(
      { language: LANGUAGE_ID },
      {
        provideSignatureHelp: async (document, position) => {
          await client.ensureDocument(document);
          const result = await client.request("textDocument/signatureHelp", textDocumentParams(document, position));
          return toSignatureHelp(result);
        },
      },
      "(",
      ",",
    ),
    vscode.languages.registerDocumentSymbolProvider(
      { language: LANGUAGE_ID },
      {
        provideDocumentSymbols: async (document) => {
          await client.ensureDocument(document);
          const result = await client.request("textDocument/documentSymbol", {
            textDocument: { uri: document.uri.toString() },
          });
          return toDocumentSymbols(result);
        },
      },
    ),
    vscode.languages.registerDefinitionProvider(
      { language: LANGUAGE_ID },
      {
        provideDefinition: async (document, position) => {
          await client.ensureDocument(document);
          const result = await client.request("textDocument/definition", textDocumentParams(document, position));
          return toDefinition(result);
        },
      },
    ),
    vscode.commands.registerCommand("usagiShader.selectTarget", () => selectTarget(client)),
    vscode.commands.registerCommand("usagiShader.showGeneratedGlsl", () => showGeneratedGlsl(client)),
    vscode.commands.registerCommand("usagiShader.checkProject", () => checkProjectShaders()),
    vscode.commands.registerCommand("usagiShader.restartServer", () => client.restart()),
  );

  if (vscode.workspace.textDocuments.some(isShaderDocument)) {
    client.ensureStarted().catch((error) => client.reportError("starting shader language server", error));
  }
}

function deactivate() {}

class UsagiShaderClient {
  constructor(context) {
    this.context = context;
    this.output = vscode.window.createOutputChannel("Usagi Shader");
    this.diagnostics = vscode.languages.createDiagnosticCollection("usagi-shader");
    this.process = undefined;
    this.ready = undefined;
    this.nextId = 1;
    this.pending = new Map();
    this.buffer = Buffer.alloc(0);
    this.documents = new Map();
  }

  dispose() {
    this.stop();
    this.diagnostics.dispose();
    this.output.dispose();
  }

  async restart() {
    this.stop();
    await this.ensureStarted();
  }

  async ensureDocument(document) {
    if (!isShaderDocument(document)) {
      return;
    }
    await this.ensureStarted();
    const uri = document.uri.toString();
    if (!this.documents.has(uri)) {
      this.sendDidOpen(document);
      return;
    }
    if (this.documents.get(uri) !== document.version) {
      this.sendDidChange(document);
    }
  }

  onDidOpen(document) {
    if (!isShaderDocument(document)) {
      return;
    }
    this.ensureDocument(document).catch((error) => this.reportError("opening shader document", error));
  }

  onDidChange(document) {
    if (!isShaderDocument(document)) {
      return;
    }
    this.ensureDocument(document).catch((error) => this.reportError("changing shader document", error));
  }

  onDidClose(document) {
    if (!isShaderDocument(document)) {
      return;
    }
    const uri = document.uri.toString();
    this.documents.delete(uri);
    this.diagnostics.delete(document.uri);
    if (this.process) {
      this.sendNotification("textDocument/didClose", {
        textDocument: { uri },
      });
    }
  }

  async ensureStarted() {
    if (this.process && this.ready) {
      return this.ready;
    }
    this.start();
    return this.ready;
  }

  start() {
    const executable = config().get("serverPath", "usagi");
    const workspaceFolder = primaryWorkspaceFolder();
    const cwd = workspaceFolder ? workspaceFolder.uri.fsPath : undefined;
    const args = ["shaders", "lsp"];

    this.output.appendLine(`Starting ${executable} ${args.join(" ")}`);
    const proc = childProcess.spawn(executable, args, {
      cwd,
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });
    this.process = proc;
    this.buffer = Buffer.alloc(0);

    proc.stdout.on("data", (chunk) => this.handleData(chunk));
    proc.stderr.on("data", (chunk) => this.output.append(chunk.toString()));
    proc.on("error", (error) => this.handleProcessFailure(proc, error));
    proc.on("exit", (code, signal) => {
      if (this.process === proc) {
        this.handleProcessFailure(proc, new Error(`server exited: code=${code}, signal=${signal}`));
      }
    });

    this.ready = this.sendRequest("initialize", {
      processId: process.pid,
      rootUri: workspaceFolder ? workspaceFolder.uri.toString() : null,
      capabilities: {},
      initializationOptions: {
        target: configuredTarget(),
      },
    }).then(() => {
      this.sendNotification("initialized", {});
      for (const document of vscode.workspace.textDocuments) {
        if (isShaderDocument(document)) {
          this.sendDidOpen(document);
        }
      }
    });
  }

  stop() {
    const proc = this.process;
    this.process = undefined;
    this.ready = undefined;
    this.documents.clear();
    if (proc && !proc.killed) {
      try {
        this.sendRaw(proc, {
          jsonrpc: "2.0",
          method: "exit",
          params: {},
        });
      } catch {
        // The process is already gone or its stdio pipe has closed.
      }
      proc.kill();
    }
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(new Error("Usagi shader language server stopped"));
    }
    this.pending.clear();
  }

  request(method, params) {
    return this.ensureStarted().then(() => this.sendRequest(method, params));
  }

  sendDidOpen(document) {
    this.documents.set(document.uri.toString(), document.version);
    this.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri: document.uri.toString(),
        languageId: LANGUAGE_ID,
        version: document.version,
        text: document.getText(),
      },
    });
  }

  sendDidChange(document) {
    this.documents.set(document.uri.toString(), document.version);
    this.sendNotification("textDocument/didChange", {
      textDocument: {
        uri: document.uri.toString(),
        version: document.version,
      },
      contentChanges: [
        {
          text: document.getText(),
        },
      ],
    });
  }

  sendRequest(method, params) {
    const id = this.nextId++;
    const payload = {
      jsonrpc: "2.0",
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Usagi shader LSP request timed out: ${method}`));
      }, 10000);
      this.pending.set(id, { resolve, reject, timeout });
      try {
        this.send(payload);
      } catch (error) {
        clearTimeout(timeout);
        this.pending.delete(id);
        reject(error);
      }
    });
  }

  sendNotification(method, params) {
    this.send({
      jsonrpc: "2.0",
      method,
      params,
    });
  }

  send(payload) {
    if (!this.process) {
      throw new Error("Usagi shader language server is not running");
    }
    this.sendRaw(this.process, payload);
  }

  sendRaw(proc, payload) {
    const body = JSON.stringify(payload);
    const header = `Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n`;
    proc.stdin.write(header);
    proc.stdin.write(body);
  }

  handleData(chunk) {
    this.buffer = Buffer.concat([this.buffer, Buffer.from(chunk)]);
    for (;;) {
      const header = headerBounds(this.buffer);
      if (!header) {
        return;
      }
      const headerText = this.buffer.slice(0, header.start).toString("utf8");
      const match = /Content-Length:\s*(\d+)/i.exec(headerText);
      if (!match) {
        this.output.appendLine("Usagi shader LSP message missing Content-Length");
        this.buffer = Buffer.alloc(0);
        return;
      }
      const length = Number(match[1]);
      const messageEnd = header.end + length;
      if (this.buffer.length < messageEnd) {
        return;
      }
      const body = this.buffer.slice(header.end, messageEnd).toString("utf8");
      this.buffer = this.buffer.slice(messageEnd);
      try {
        this.handleMessage(JSON.parse(body));
      } catch (error) {
        this.reportError("parsing LSP response", error);
      }
    }
  }

  handleMessage(message) {
    if (Object.prototype.hasOwnProperty.call(message, "id")) {
      const pending = this.pending.get(message.id);
      if (!pending) {
        return;
      }
      clearTimeout(pending.timeout);
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(new Error(message.error.message || "Usagi shader LSP error"));
      } else {
        pending.resolve(message.result);
      }
      return;
    }

    if (message.method === "textDocument/publishDiagnostics") {
      this.applyDiagnostics(message.params || {});
    }
  }

  applyDiagnostics(params) {
    if (!params.uri) {
      return;
    }
    const uri = vscode.Uri.parse(params.uri);
    const diagnostics = (params.diagnostics || []).map((diagnostic) => {
      const item = new vscode.Diagnostic(
        toRange(diagnostic.range),
        diagnostic.message || "Usagi shader diagnostic",
        toDiagnosticSeverity(diagnostic.severity),
      );
      item.source = diagnostic.source || "usagi shader";
      item.code = diagnostic.code;
      return item;
    });
    this.diagnostics.set(uri, diagnostics);
  }

  handleProcessFailure(proc, error) {
    if (this.process !== proc) {
      return;
    }
    this.output.appendLine(String(error.message || error));
    this.process = undefined;
    this.ready = undefined;
    this.documents.clear();
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }

  reportError(action, error) {
    this.output.appendLine(`Error ${action}: ${error.message || error}`);
  }
}

async function selectTarget(client) {
  const picked = await vscode.window.showQuickPick(
    [
      { label: "desktop", description: "GLSL 330 desktop diagnostics" },
      { label: "web", description: "GLSL ES 100 web diagnostics" },
      { label: "all", description: "ES 100, GLSL 330, and staged GLSL 440 diagnostics" },
    ],
    { placeHolder: "Select Usagi shader diagnostic target" },
  );
  if (!picked) {
    return;
  }
  await config().update("target", picked.label, configTarget());
}

async function showGeneratedGlsl(client) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || !isShaderDocument(editor.document)) {
    vscode.window.showWarningMessage("Open a .usagi.fs shader first.");
    return;
  }

  await client.ensureDocument(editor.document);
  const picked = await vscode.window.showQuickPick(
    [
      { label: "desktop", description: "GLSL 330" },
      { label: "web", description: "GLSL ES 100" },
      { label: "glsl440", description: "Staged GLSL 440" },
    ],
    { placeHolder: "Preview generated GLSL target" },
  );
  if (!picked) {
    return;
  }

  const result = await client.request(GENERATED_GLSL_METHOD, {
    textDocument: { uri: editor.document.uri.toString() },
    target: picked.label,
  });
  if (!result || result.ok !== true) {
    const messages = (result && result.diagnostics ? result.diagnostics : [])
      .map((diagnostic) => diagnostic.message)
      .filter(Boolean)
      .join("\n");
    vscode.window.showErrorMessage(messages || "Usagi shader generation failed.");
    return;
  }

  const document = await vscode.workspace.openTextDocument({
    content: result.source,
    language: "glsl",
  });
  await vscode.window.showTextDocument(document, { preview: true });
}

function checkProjectShaders() {
  const folder = activeWorkspaceFolder();
  const cwd = folder ? folder.uri.fsPath : activeDocumentDirectory();
  const target = configuredTarget();
  const terminal = vscode.window.createTerminal({ name: "Usagi Shader Check", cwd });
  terminal.show(true);
  terminal.sendText(`${commandInvocation(config().get("serverPath", "usagi"))} shaders check . --target ${target} --format json`);
}

function textDocumentParams(document, position) {
  return {
    textDocument: { uri: document.uri.toString() },
    position: {
      line: position.line,
      character: position.character,
    },
  };
}

function toCompletionList(result) {
  const rawItems = Array.isArray(result) ? result : (result && result.items) || [];
  const items = rawItems.map((raw) => {
    const item = new vscode.CompletionItem(raw.label || "", toCompletionKind(raw.kind));
    item.detail = raw.detail;
    item.documentation = toMarkdown(raw.documentation);
    if (raw.insertText) {
      item.insertText = raw.insertTextFormat === 2 ? new vscode.SnippetString(raw.insertText) : raw.insertText;
    }
    return item;
  });
  return new vscode.CompletionList(items, Boolean(result && result.isIncomplete));
}

function toHover(result) {
  if (!result || !result.contents) {
    return undefined;
  }
  return new vscode.Hover(toMarkdown(result.contents));
}

function toSignatureHelp(result) {
  if (!result || !Array.isArray(result.signatures)) {
    return undefined;
  }
  const help = new vscode.SignatureHelp();
  help.activeSignature = result.activeSignature || 0;
  help.activeParameter = result.activeParameter || 0;
  help.signatures = result.signatures.map((raw) => {
    const signature = new vscode.SignatureInformation(raw.label || "", toMarkdown(raw.documentation));
    signature.parameters = (raw.parameters || []).map(
      (parameter) => new vscode.ParameterInformation(parameter.label || "", toMarkdown(parameter.documentation)),
    );
    return signature;
  });
  return help;
}

function toDocumentSymbols(result) {
  if (!Array.isArray(result)) {
    return [];
  }
  return result.map((raw) => {
    return new vscode.DocumentSymbol(
      raw.name || "",
      raw.detail || "",
      toSymbolKind(raw.kind),
      toRange(raw.range),
      toRange(raw.selectionRange),
    );
  });
}

function toDefinition(result) {
  if (!result || !result.uri || !result.range) {
    return undefined;
  }
  return new vscode.Location(vscode.Uri.parse(result.uri), toRange(result.range));
}

function toMarkdown(value) {
  if (!value) {
    return undefined;
  }
  if (typeof value === "string") {
    return new vscode.MarkdownString(value);
  }
  if (value.kind === "markdown") {
    return new vscode.MarkdownString(value.value || "");
  }
  if (value.value) {
    return new vscode.MarkdownString(value.value);
  }
  return undefined;
}

function toRange(range) {
  if (!range) {
    return new vscode.Range(0, 0, 0, 1);
  }
  return new vscode.Range(
    range.start ? range.start.line || 0 : 0,
    range.start ? range.start.character || 0 : 0,
    range.end ? range.end.line || 0 : 0,
    range.end ? range.end.character || 0 : 1,
  );
}

function toDiagnosticSeverity(severity) {
  switch (severity) {
    case 2:
      return vscode.DiagnosticSeverity.Warning;
    case 3:
      return vscode.DiagnosticSeverity.Information;
    case 4:
      return vscode.DiagnosticSeverity.Hint;
    case 1:
    default:
      return vscode.DiagnosticSeverity.Error;
  }
}

function toCompletionKind(kind) {
  switch (kind) {
    case 3:
      return vscode.CompletionItemKind.Function;
    case 6:
      return vscode.CompletionItemKind.Variable;
    case 14:
      return vscode.CompletionItemKind.Keyword;
    default:
      return vscode.CompletionItemKind.Text;
  }
}

function toSymbolKind(kind) {
  switch (kind) {
    case 12:
      return vscode.SymbolKind.Function;
    case 13:
      return vscode.SymbolKind.Variable;
    default:
      return vscode.SymbolKind.String;
  }
}

function headerBounds(buffer) {
  const crlf = buffer.indexOf("\r\n\r\n");
  if (crlf >= 0) {
    return { start: crlf, end: crlf + 4 };
  }
  const lf = buffer.indexOf("\n\n");
  if (lf >= 0) {
    return { start: lf, end: lf + 2 };
  }
  return undefined;
}

function isShaderDocument(document) {
  return document.languageId === LANGUAGE_ID || document.fileName.endsWith(".usagi.fs");
}

function configuredTarget() {
  return config().get("target", "desktop");
}

function config() {
  return vscode.workspace.getConfiguration(CONFIG_SECTION);
}

function configTarget() {
  return vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
}

function primaryWorkspaceFolder() {
  return vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0
    ? vscode.workspace.workspaceFolders[0]
    : undefined;
}

function activeWorkspaceFolder() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    return primaryWorkspaceFolder();
  }
  return vscode.workspace.getWorkspaceFolder(editor.document.uri) || primaryWorkspaceFolder();
}

function activeDocumentDirectory() {
  const editor = vscode.window.activeTextEditor;
  return editor && editor.document.uri.scheme === "file" ? path.dirname(editor.document.uri.fsPath) : undefined;
}

function commandInvocation(value) {
  return process.platform === "win32" ? `& ${quotePowerShell(value)}` : quotePosix(value);
}

function quotePowerShell(value) {
  const escaped = String(value).replace(/`/g, "``").replace(/"/g, '`"');
  return `"${escaped}"`;
}

function quotePosix(value) {
  const escaped = String(value).replace(/'/g, "'\\''");
  return `'${escaped}'`;
}

module.exports = {
  activate,
  deactivate,
};
