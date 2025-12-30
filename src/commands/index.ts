import ora from "ora";
import chalk from "chalk";
import { VectorStore } from "../lib/store.js";
import { createFileSystem } from "../lib/file.js";

interface IndexOptions {
  store?: string;
}

export async function indexCommand(
  path?: string,
  options: IndexOptions = {}
): Promise<void> {
  const cwd = path || process.cwd();
  const storeName = options.store || "searchgrep";

  console.log(chalk.cyan("\nSearchgrep") + " - Indexing codebase\n");

  const spinner = ora("Scanning files...").start();

  try {
    const fs = createFileSystem({ cwd });
    const files = await fs.getAllFiles();

    spinner.text = `Found ${files.length} files. Initializing...`;

    const store = new VectorStore(storeName);

    spinner.text = `Indexing ${files.length} files...`;

    let indexed = 0;
    let errors = 0;

    for (const file of files) {
      try {
        const hash = Buffer.from(file.content).toString("base64").slice(0, 32);
        await store.uploadFile(
          file.path,
          file.content,
          hash,
          file.content.length,
          file.lastModified
        );
        indexed++;
        spinner.text = `Indexing... ${indexed}/${files.length} files`;
      } catch (e) {
        errors++;
      }
    }

    spinner.succeed(`Indexed ${indexed} files`);

    if (errors > 0) {
      console.log(chalk.yellow(`  ${errors} files skipped (binary or too large)`));
    }

    const info = store.getInfo();
    console.log(chalk.gray(`  Store: ${storeName}`));
    console.log(chalk.gray(`  Total size: ${(info.totalSize / 1024).toFixed(1)} KB`));

    console.log(
      chalk.green("\nReady to search!") +
        chalk.gray(" Run: ") +
        chalk.white("searchgrep search 'your query'")
    );
  } catch (error) {
    spinner.fail("Indexing failed");
    console.error(
      chalk.red("Error:"),
      error instanceof Error ? error.message : String(error)
    );
    process.exit(1);
  }
}
