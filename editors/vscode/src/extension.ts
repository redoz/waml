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

export async function activate(context: ExtensionContext): Promise<void> {
  await deactivate();

  const ctx: ServerPathContext = {
    env: process.env,
    extensionPath: context.extensionPath,
    platform: process.platform,
    configInspection: workspace.getConfiguration("waml").inspect<string>("serverPath"),
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
        `WAML language server not started. ${resolution.reason ?? ""} ` +
          "After fixing this, reload the window (Developer: Reload Window).",
        "Open WAML Settings",
      )
      .then((choice) => {
        if (choice === "Open WAML Settings") {
          void commands.executeCommand("workbench.action.openSettings", "waml.serverPath");
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
    initializationOptions: undefined,
    // Pin vscode-languageclient's bounded default crash-restart policy.
    connectionOptions: { maxRestartCount: 4 },
  };
  const nextClient = new LanguageClient("waml", "WAML", serverOptions, clientOptions);
  client = nextClient;
  try {
    await nextClient.start();
  } catch (error) {
    if (client === nextClient) client = undefined;
    await nextClient.stop();
    const message = error instanceof Error ? error.message : String(error);
    await window.showErrorMessage(`WAML language server failed to start. ${message}`);
  }
}

export async function deactivate(): Promise<void> {
  const activeClient = client;
  client = undefined;
  await activeClient?.stop();
}
