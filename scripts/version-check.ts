import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");

const TOC_PATH = resolve(ROOT, "vendor/libsharedmedia-3.0/LibSharedMedia-3.0.toc");
const WAGO_API = "https://addons.wago.io/api/data/game";

function patchToInterface(patch: string): number {
	const [major, minor, patch_n] = patch.split(".").map(Number);
	return major * 10000 + (minor ?? 0) * 100 + (patch_n ?? 0);
}

function getVendorMaxInterface(): number {
	const toc = readFileSync(TOC_PATH, "utf-8");
	const line = toc.split("\n").find((l) => l.startsWith("## Interface:"));
	if (!line) {
		throw new Error("No ## Interface: line found in vendor TOC");
	}
	const versions = line
		.replace("## Interface:", "")
		.split(",")
		.map((v) => Number(v.trim()));
	return Math.max(...versions);
}

async function getWagoRetailPatch(): Promise<string> {
	const resp = await fetch(WAGO_API);
	if (!resp.ok) {
		throw new Error(`Wago API returned ${resp.status}: ${resp.statusText}`);
	}
	const data = (await resp.json()) as {
		live_patches: { supported_retail_patches: string };
	};
	return data.live_patches.supported_retail_patches;
}

const vendorMax = getVendorMaxInterface();
console.log(`Vendor TOC max:  Interface ${vendorMax}`);

try {
	const wagoPatch = await getWagoRetailPatch();
	const wagoInterface = patchToInterface(wagoPatch);
	console.log(`Wago API retail: ${wagoPatch} -> Interface ${wagoInterface}`);

	if (vendorMax < wagoInterface) {
		console.error(
			`::warning::Vendor Interface (${vendorMax}) is behind Wago latest (${wagoInterface}). Run 'mise run vendor:refresh' to update.`,
		);
		process.exit(1);
	}

	console.log("OK — vendor is up to date.");
} catch (err) {
	console.error(`Failed to check Wago API: ${err}`);
	console.error("Skipping freshness check.");
	process.exit(0);
}
