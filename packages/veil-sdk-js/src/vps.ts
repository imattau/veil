export type VpsProfile = {
  host: string;
  wsUrl: string;
  quicEndpoint?: string;
  quicCertB64?: string;
};

export function vpsProfileFromDomain(
  host: string,
  opts?: { quicPort?: number; quicCertB64?: string; secure?: boolean }
): VpsProfile {
  const secure = opts?.secure ?? true;
  const wsUrl = `${secure ? "wss" : "ws"}://${host}/ws`;
  const quicEndpoint =
    opts?.quicPort !== undefined ? `quic://${host}:${opts.quicPort}` : undefined;
  return {
    host,
    wsUrl,
    quicEndpoint,
    quicCertB64: opts?.quicCertB64,
  };
}

export function parseVpsConfigJs(host: string, body: string): VpsProfile | null {
  const quicPortRaw = extractConfigValue(body, "VEIL_VPS_QUIC_PORT");
  const quicCertB64 = extractConfigValue(body, "VEIL_VPS_QUIC_CERT_B64") || undefined;
  const quicPort = quicPortRaw ? Number.parseInt(quicPortRaw, 10) : undefined;
  return vpsProfileFromDomain(host, {
    quicPort: Number.isFinite(quicPort) ? quicPort : undefined,
    quicCertB64,
  });
}

export function toVpsProfileUri(profile: VpsProfile): string {
  const params = new URLSearchParams({ ws: profile.wsUrl });
  if (profile.quicEndpoint) params.set("quic", profile.quicEndpoint);
  if (profile.quicCertB64) params.set("certb64", profile.quicCertB64);
  return `veil://vps?${params.toString()}`;
}

function extractConfigValue(body: string, key: string): string | null {
  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line.startsWith("window.")) continue;
    if (!line.includes(key)) continue;
    const parts = line.split("=");
    if (parts.length < 2) continue;
    let value = parts[1].trim();
    if (value.endsWith(";")) value = value.slice(0, -1);
    value = value.replace(/["']/g, "").trim();
    if (!value) return null;
    return value;
  }
  return null;
}
