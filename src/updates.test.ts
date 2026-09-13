import { beforeEach, expect, test, vi } from "vitest";
const api = vi.hoisted(() => ({
  android: false,
  command: vi.fn(),
  mobile: vi.fn(),
}));
vi.mock("./api", () => api);
import { checkForUpdate, installUpdate } from "./updates";
const manifest = {
  version: "0.3.0",
  version_code: 3000,
  apk_url: "https://example.org/a.apk",
  sha256: "hash",
  signature: "sig",
};
beforeEach(() => {
  vi.resetAllMocks();
  api.android = false;
});
test("Linux checks and installs the release retained by the native updater", async () => {
  api.command
    .mockResolvedValueOnce({ version: "0.3.0" })
    .mockResolvedValueOnce(undefined);
  const update = await checkForUpdate();
  await installUpdate(update!);
  expect(api.command.mock.calls).toEqual([
    ["check_desktop_update"],
    ["install_desktop_update"],
  ]);
});
test("no new release returns null", async () => {
  api.command.mockResolvedValue(null);
  expect(await checkForUpdate()).toBeNull();
});
test("Android downloads the signed manifest's APK before requesting installation", async () => {
  api.android = true;
  api.mobile.mockResolvedValue({ connected: true });
  api.command
    .mockResolvedValueOnce(manifest)
    .mockResolvedValueOnce("/cache/update.apk");
  const update = await checkForUpdate();
  await installUpdate(update!);
  expect(api.command.mock.calls).toEqual([
    ["check_update"],
    ["download_update", { manifest }],
  ]);
  expect(api.mobile).toHaveBeenLastCalledWith("install", {
    path: "/cache/update.apk",
    versionCode: 3000,
  });
});
test("Android rechecks Wi-Fi before download, including after a successful check", async () => {
  api.android = true;
  api.mobile
    .mockResolvedValueOnce({ connected: true })
    .mockResolvedValueOnce({ connected: false });
  api.command.mockResolvedValue(manifest);
  const update = await checkForUpdate();
  await expect(installUpdate(update!)).rejects.toThrow("Wi-Fi");
  expect(api.command).toHaveBeenCalledTimes(1);
});
test("failed download never opens the installer; a retry can succeed", async () => {
  api.android = true;
  api.mobile.mockResolvedValue({ connected: true });
  api.command.mockRejectedValueOnce(
    new Error("Update checksum does not match"),
  );
  await expect(
    installUpdate({ version: manifest.version, manifest }),
  ).rejects.toThrow("checksum");
  expect(api.mobile).not.toHaveBeenCalledWith("install", expect.anything());
  api.command.mockResolvedValueOnce("/cache/update.apk");
  await expect(
    installUpdate({ version: manifest.version, manifest }),
  ).resolves.toContain("cancel");
});
