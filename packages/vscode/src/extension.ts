import { workspace, window, commands, ExtensionContext } from "vscode";
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import {
  resolveServerPath,
  setServerCommand,
  getServerCommand,
  type ServerPathContext,
} from "./serverPath";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext): void {
  const ctx: ServerPathContext = {
    env: process.env,
    extensionPath: context.extensionPath,
    platform: process.platform,
    configInspection: workspace.getConfiguration("uaml").inspect<string>("serverPath"),
    fileExists: (p) => existsSync(p),
    probeCommand: (command) => {
      const result = spawnSync(command, ["--version"], { stdio: "ignore" });
      const code = (result.error as NodeJS.ErrnoException | undefined)?.code;
      return code !== "ENOENT";
    },
  };

  const resolution = resolveServerPath(ctx);
  if (!resolution.runnable) {
    void window
      .showErrorMessage(
        `UAML language server not started. ${resolution.reason ?? ""} ` +
          "After fixing this, reload the window (Developer: Reload Window).",
        "Open UAML Settings",
      )
      .then((choice) => {
        if (choice === "Open UAML Settings") {
          void commands.executeCommand("workbench.action.openSettings", "uaml.serverPath");
        }
      });
    return;
  }

  setServerCommand(resolution.command);
  const serverOptions: ServerOptions = {
    command: getServerCommand(),
    // Only the subcommand — TransportKind.stdio makes the client append `--stdio`.
    args: ["lsp"],
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
