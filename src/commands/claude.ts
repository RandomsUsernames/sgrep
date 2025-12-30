import chalk from "chalk";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const HOME = homedir();

const SEARCHGREP_MCP = {
  command: "searchgrep-mcp",
  args: [],
  env: {},
};

interface AIAgent {
  name: string;
  configPath: string;
  configDir: string;
  key: string;
}

const AI_AGENTS: AIAgent[] = [
  {
    name: "Claude Desktop",
    configPath: join(HOME, ".config", "claude-desktop", "mcp.json"),
    configDir: join(HOME, ".config", "claude-desktop"),
    key: "mcpServers",
  },
  {
    name: "Claude Code",
    configPath: join(HOME, ".claude", "mcp_servers.json"),
    configDir: join(HOME, ".claude"),
    key: "mcpServers",
  },
  {
    name: "Claude Code (config)",
    configPath: join(HOME, ".config", "claude-code", "mcp_servers.json"),
    configDir: join(HOME, ".config", "claude-code"),
    key: "mcpServers",
  },
];

function registerWithAgent(agent: AIAgent): boolean {
  try {
    // Ensure config directory exists
    if (!existsSync(agent.configDir)) {
      mkdirSync(agent.configDir, { recursive: true });
    }

    // Read existing config or create new one
    let config: Record<string, unknown> = {};
    if (existsSync(agent.configPath)) {
      const content = readFileSync(agent.configPath, "utf-8");
      try {
        config = JSON.parse(content);
      } catch {
        // Invalid JSON, start fresh
        config = {};
      }
    }

    // Ensure mcpServers object exists
    if (!config[agent.key] || typeof config[agent.key] !== "object") {
      config[agent.key] = {};
    }

    // Add searchgrep MCP server
    (config[agent.key] as Record<string, unknown>)["searchgrep"] =
      SEARCHGREP_MCP;

    // Write updated config
    writeFileSync(agent.configPath, JSON.stringify(config, null, 2));

    return true;
  } catch {
    return false;
  }
}

export async function claudeCommand(): Promise<void> {
  console.log(chalk.blue("🔧 Registering searchgrep MCP with Claude...\n"));

  let registered = 0;

  for (const agent of AI_AGENTS) {
    // Check if the config directory exists or if we should try to create it
    const dirExists = existsSync(agent.configDir);
    const configExists = existsSync(agent.configPath);

    if (dirExists || configExists) {
      const success = registerWithAgent(agent);
      if (success) {
        console.log(chalk.green(`  ✓ ${agent.name}`));
        registered++;
      } else {
        console.log(chalk.red(`  ✗ ${agent.name} (failed to write config)`));
      }
    } else {
      console.log(chalk.gray(`  - ${agent.name} (not installed)`));
    }
  }

  console.log();

  if (registered > 0) {
    console.log(
      chalk.green(`Successfully registered with ${registered} Claude agent(s)`)
    );
    console.log(
      chalk.yellow("\nRestart Claude to activate the searchgrep MCP server.")
    );
  } else {
    console.log(
      chalk.yellow(
        "No Claude installations found. Install Claude Desktop or Claude Code first."
      )
    );
  }
}
