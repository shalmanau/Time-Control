import { android, command, mobile } from "./api";
import type { Manifest } from "./types";

export interface AvailableUpdate {
  version: string;
  manifest?: Manifest;
}

export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  if (android) {
    await requireWifi();
    const manifest = await command<Manifest | null>("check_update");
    return manifest ? { version: manifest.version, manifest } : null;
  }
  return command<AvailableUpdate | null>("check_desktop_update");
}

async function requireWifi() {
  const wifi = await mobile<{ connected: boolean }>("wifi");
  if (!wifi.connected) throw new Error("Connect to Wi-Fi to update the app.");
}

export async function installUpdate(update: AvailableUpdate): Promise<string> {
  if (android) {
    await requireWifi();
    if (!update.manifest) throw new Error("Check for updates again.");
    const path = await command<string>("download_update", {
      manifest: update.manifest,
    });
    await mobile("install", {
      path,
      versionCode: update.manifest.version_code,
    });
    return "Finish installing in the Android system dialog. If you cancel, you can try again here.";
  }
  await command("install_desktop_update");
  return "Update installed. Restarting…";
}
