import { workspace, ExtensionContext } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(_context: ExtensionContext): void {
  const command = workspace.getConfiguration("uaml").get<string>("serverPath", "uaml");
  const serverOptions: ServerOptions = {
    command,
    args: ["lsp", "--stdio"],
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "markdown" }],
  };
  client = new LanguageClient("uaml", "UAML", serverOptions, clientOptions);
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
