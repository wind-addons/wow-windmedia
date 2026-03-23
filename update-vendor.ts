#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const VENDOR_ROOT = resolve(import.meta.dir, "vendor");
const LSM_SVN = "https://repos.wowace.com/wow/libsharedmedia-3-0/trunk";
const SERPENT_URL = "https://raw.githubusercontent.com/pkulchenko/serpent/master/src/serpent.lua";

mkdirSync(VENDOR_ROOT, { recursive: true });

async function updateSerpent() {
	const dir = resolve(VENDOR_ROOT, "serpent");
	mkdirSync(dir, { recursive: true });

	const response = await fetch(SERPENT_URL);
	if (!response.ok) {
		throw new Error(`HTTP ${response.status} from ${SERPENT_URL}`);
	}
	const content = await response.text();
	if (content.length === 0) {
		throw new Error(`Empty response from ${SERPENT_URL}`);
	}

	writeFileSync(resolve(dir, "serpent.lua"), content, "utf-8");
}

function updateLibSharedMedia() {
	const dir = resolve(VENDOR_ROOT, "libsharedmedia-3.0");
	const result = Bun.spawnSync(["svn", "export", "--force", "--quiet", LSM_SVN, dir]);
	if (result.exitCode !== 0) {
		const stderr = new TextDecoder().decode(result.stderr);
		throw new Error(`svn export failed: ${stderr}`);
	}
}

try {
	console.log("Updating vendor libraries...");

	updateLibSharedMedia();
	console.log("  ✓ libsharedmedia-3.0");

	await updateSerpent();
	console.log("  ✓ serpent");

	console.log("\nDone.");
} catch (err) {
	console.error(`\n${err instanceof Error ? err.message : String(err)}`);
	process.exit(1);
}
