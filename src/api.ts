import { invoke, isTauri } from "@tauri-apps/api/core";
export const native = isTauri();
export const android = native && /Android/i.test(navigator.userAgent);
export async function command<T>(
  name: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (native) return invoke<T>(name, args);
  const bridge = import.meta.env.DEV && import.meta.env.VITE_DEV_BRIDGE_URL;
  if (!bridge)
    throw new Error(
      "Open Time Ledger as a desktop or Android app. For browser development, start the Rust development bridge.",
    );
  const res = await fetch(`${bridge}/${name}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
  const body = await res.json();
  if (body.error) throw new Error(body.error);
  return body.value as T;
}
export function mobile<T>(
  name: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  return invoke<T>(`plugin:nearby|${name}`, args);
}
