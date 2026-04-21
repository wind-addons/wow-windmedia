#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

type VendorManifest = {
	version: number;
	vendors: Record<string, VendorSpec>;
};

type VendorSpec = SvnExportVendorSpec | RemoteFileVendorSpec;

type SvnExportVendorSpec = {
	type: "svn-export";
	repository: string;
	revision: number;
	outputDir: string;
	files: Record<string, string>;
};

type RemoteFileVendorSpec = {
	type: "remote-file";
	repository: string;
	revision: string;
	url: string;
	outputDir: string;
	files: Record<string, string>;
	refresh: {
		type: "github-commit-path";
		owner: string;
		repo: string;
		path: string;
	};
};

const ROOT = import.meta.dir;
const MANIFEST_PATH = resolve(ROOT, "vendor.lock.json");
const VENDOR_ROOT = resolve(ROOT, "vendor");
const REFRESH_FLAG = "--refresh";

function readManifest(): VendorManifest {
	return JSON.parse(readFileSync(MANIFEST_PATH, "utf-8")) as VendorManifest;
}

function writeManifest(manifest: VendorManifest) {
	writeFileSync(MANIFEST_PATH, `${JSON.stringify(manifest, null, "\t")}\n`, "utf-8");
}

function sha256(data: Uint8Array | string): string {
	return createHash("sha256").update(data).digest("hex");
}

function ensureDirFor(filePath: string) {
	mkdirSync(dirname(filePath), { recursive: true });
}

function rmIfExists(path: string) {
	if (existsSync(path)) {
		rmSync(path, { recursive: true, force: true });
	}
}

function formatCommandError(stderr: Uint8Array<ArrayBufferLike>) {
	const text = new TextDecoder().decode(stderr).trim();
	return text.length > 0 ? text : "no stderr output";
}

function verifyFiles(name: string, outputDir: string, files: Record<string, string>) {
	for (const [relativePath, expectedHash] of Object.entries(files)) {
		const filePath = resolve(outputDir, relativePath);
		if (!existsSync(filePath)) {
			throw new Error(`[${name}] missing expected file: ${relativePath}`);
		}
		const actualHash = sha256(readFileSync(filePath));
		if (actualHash !== expectedHash) {
			throw new Error(`[${name}] checksum mismatch for ${relativePath}: expected ${expectedHash}, got ${actualHash}`);
		}
	}
}

function exportSvnVendor(name: string, spec: SvnExportVendorSpec) {
	const outputDir = resolve(ROOT, spec.outputDir);
	rmIfExists(outputDir);
	ensureDirFor(resolve(outputDir, ".keep"));

	const result = Bun.spawnSync([
		"svn",
		"export",
		"--force",
		"--quiet",
		"-r",
		String(spec.revision),
		spec.repository,
		outputDir,
	]);
	if (result.exitCode !== 0) {
		throw new Error(`[${name}] svn export failed: ${formatCommandError(result.stderr)}`);
	}

	verifyFiles(name, outputDir, spec.files);
	console.log(`  ✓ ${name} @ r${spec.revision}`);
	return outputDir;
}

async function downloadRemoteFile(name: string, spec: RemoteFileVendorSpec) {
	const outputDir = resolve(ROOT, spec.outputDir);
	rmIfExists(outputDir);
	mkdirSync(outputDir, { recursive: true });

	const entries = Object.entries(spec.files);
	if (entries.length !== 1) {
		throw new Error(`[${name}] remote-file vendors must declare exactly one output file`);
	}

	const [[relativePath, expectedHash]] = entries;
	const filePath = resolve(outputDir, relativePath);
	ensureDirFor(filePath);

	const response = await fetch(spec.url);
	if (!response.ok) {
		throw new Error(`[${name}] HTTP ${response.status} from ${spec.url}`);
	}

	const content = await response.text();
	if (content.length === 0) {
		throw new Error(`[${name}] empty response from ${spec.url}`);
	}

	const actualHash = sha256(content);
	if (actualHash !== expectedHash) {
		throw new Error(`[${name}] checksum mismatch for ${relativePath}: expected ${expectedHash}, got ${actualHash}`);
	}

	writeFileSync(filePath, content, "utf-8");
	console.log(`  ✓ ${name} @ ${spec.revision.slice(0, 12)}`);
	return outputDir;
}

async function fetchRemoteFile(name: string, url: string): Promise<string> {
	const response = await fetch(url);
	if (!response.ok) {
		throw new Error(`[${name}] HTTP ${response.status} from ${url}`);
	}

	const content = await response.text();
	if (content.length === 0) {
		throw new Error(`[${name}] empty response from ${url}`);
	}

	return content;
}

async function materializeSnapshot(manifest: VendorManifest) {
	rmIfExists(VENDOR_ROOT);
	mkdirSync(VENDOR_ROOT, { recursive: true });
	for (const [name, spec] of Object.entries(manifest.vendors)) {
		if (spec.type === "svn-export") {
			exportSvnVendor(name, spec);
		} else {
			await downloadRemoteFile(name, spec);
		}
	}
	console.log("\nSnapshot materialized.");
}

function getLatestSvnRevision(repository: string): number {
	const result = Bun.spawnSync(["svn", "info", repository]);
	if (result.exitCode !== 0) {
		throw new Error(`svn info failed: ${formatCommandError(result.stderr)}`);
	}

	const info = new TextDecoder().decode(result.stdout);
	const match = info.match(/^Revision:\s+(\d+)$/m);
	if (!match) {
		throw new Error(`unable to parse svn revision from: ${repository}`);
	}
	return Number(match[1]);
}

async function getLatestGitHubCommit(owner: string, repo: string, path: string): Promise<string> {
	const params = new URLSearchParams({ path, per_page: "1" });
	const url = `https://api.github.com/repos/${owner}/${repo}/commits?${params.toString()}`;
	const response = await fetch(url, {
		headers: {
			"User-Agent": "wow-sharedmedia-vendor-refresh",
			Accept: "application/vnd.github+json",
		},
	});
	if (!response.ok) {
		throw new Error(`GitHub commit lookup failed: HTTP ${response.status} from ${url}`);
	}

	const payload = (await response.json()) as Array<{ sha?: string }>;
	const sha = payload[0]?.sha;
	if (!sha) {
		throw new Error(`no commit found for ${owner}/${repo}:${path}`);
	}
	return sha;
}

function updateHashesFromDirectory(spec: VendorSpec, outputDir: string): VendorSpec {
	const files = Object.fromEntries(
		Object.keys(spec.files).map((relativePath) => {
			const filePath = resolve(outputDir, relativePath);
			if (!existsSync(filePath)) {
				throw new Error(`missing expected file after refresh: ${relativePath}`);
			}
			return [relativePath, sha256(readFileSync(filePath))];
		}),
	);

	return { ...spec, files };
}

async function refreshManifest(manifest: VendorManifest) {
	console.log("Refreshing vendor snapshot...");
	const nextManifest: VendorManifest = {
		...manifest,
		vendors: {},
	};

	for (const [name, spec] of Object.entries(manifest.vendors)) {
		if (spec.type === "svn-export") {
			const revision = getLatestSvnRevision(spec.repository);
			const nextSpec: SvnExportVendorSpec = { ...spec, revision };
			const outputDir = exportSvnVendor(name, nextSpec);
			nextManifest.vendors[name] = updateHashesFromDirectory(nextSpec, outputDir);
			continue;
		}

		const sha = await getLatestGitHubCommit(spec.refresh.owner, spec.refresh.repo, spec.refresh.path);
		const url = `https://raw.githubusercontent.com/${spec.refresh.owner}/${spec.refresh.repo}/${sha}/${spec.refresh.path}`;
		const content = await fetchRemoteFile(name, url);
		const relativePath = Object.keys(spec.files)[0];
		const outputDir = resolve(ROOT, spec.outputDir);
		rmIfExists(outputDir);
		mkdirSync(outputDir, { recursive: true });
		const filePath = resolve(outputDir, relativePath);
		ensureDirFor(filePath);
		writeFileSync(filePath, content, "utf-8");

		const nextSpec: RemoteFileVendorSpec = {
			...spec,
			revision: sha,
			url,
		};
		nextManifest.vendors[name] = updateHashesFromDirectory(nextSpec, outputDir);
	}

	writeManifest(nextManifest);
	console.log(`\nUpdated ${MANIFEST_PATH}`);
}

async function main() {
	const refresh = process.argv.includes(REFRESH_FLAG);
	const manifest = readManifest();

	if (refresh) {
		await refreshManifest(manifest);
		return;
	}

	console.log("Materializing pinned vendor snapshot...");
	await materializeSnapshot(manifest);
}

try {
	await main();
} catch (err) {
	console.error(`\n${err instanceof Error ? err.message : String(err)}`);
	process.exit(1);
}
