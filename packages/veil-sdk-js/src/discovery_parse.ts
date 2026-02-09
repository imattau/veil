import {
  contactBundleFromQr,
  type ContactBundle,
} from "./identity";

export type DiscoveryParseResult = {
  wsEndpoints: string[];
  quicEndpoints: string[];
  peers: string[];
  tags: string[];
  quicCertHex?: string;
  quicCertB64?: string;
  contactBundle?: ContactBundle;
  isVpsProfile: boolean;
};

export function parseDiscoveryInput(value: string): DiscoveryParseResult | null {
  const raw = value.trim();
  if (!raw) return null;
  if (raw.startsWith("veil://contact")) {
    try {
      const bundle = contactBundleFromQr(raw);
      return {
        wsEndpoints: [],
        quicEndpoints: [],
        peers: [],
        tags: [],
        contactBundle: bundle,
        isVpsProfile: false,
      };
    } catch {
      return null;
    }
  }
  if (raw.startsWith("veil://") || raw.startsWith("veil:vps:") || raw.startsWith("vps:")) {
    const uri = normalizeVpsUri(raw);
    if (!uri) return null;
    const ws = uri.searchParams.getAll("ws");
    const peers = uri.searchParams.getAll("peer");
    const tags = uri.searchParams.getAll("tag");
    const quic = uri.searchParams.getAll("quic");
    const cert = uri.searchParams.get("cert") ?? undefined;
    const certb64 = uri.searchParams.get("certb64") ?? undefined;
    return {
      wsEndpoints: ws,
      quicEndpoints: quic,
      peers,
      tags,
      quicCertHex: cert,
      quicCertB64: certb64,
      isVpsProfile: true,
    };
  }
  if (raw.startsWith("http://") || raw.startsWith("https://")) {
    try {
      const url = new URL(raw);
      if (!url.pathname || url.pathname === "/" || url.pathname.endsWith("/config.js")) {
        return {
          wsEndpoints: [],
          quicEndpoints: [],
          peers: [],
          tags: [],
          isVpsProfile: true,
        };
      }
    } catch {
      return null;
    }
  }
  if (raw.startsWith("ws://") || raw.startsWith("wss://")) {
    return {
      wsEndpoints: [raw],
      quicEndpoints: [],
      peers: [],
      tags: [],
      isVpsProfile: false,
    };
  }
  if (raw.startsWith("quic://")) {
    return {
      wsEndpoints: [],
      quicEndpoints: [raw],
      peers: [],
      tags: [],
      isVpsProfile: false,
    };
  }
  if (raw.startsWith("peer:")) {
    return {
      wsEndpoints: [],
      quicEndpoints: [],
      peers: [raw.slice(5)],
      tags: [],
      isVpsProfile: false,
    };
  }
  if (raw.startsWith("tag:")) {
    return {
      wsEndpoints: [],
      quicEndpoints: [],
      peers: [],
      tags: [raw.slice(4)],
      isVpsProfile: false,
    };
  }
  const hex = raw.toLowerCase().replace(/[^0-9a-f]/g, "");
  if (hex.length === 64) {
    return {
      wsEndpoints: [],
      quicEndpoints: [],
      peers: [],
      tags: [hex],
      isVpsProfile: false,
    };
  }
  return null;
}

function normalizeVpsUri(raw: string): URL | null {
  const lower = raw.toLowerCase();
  if (lower.startsWith("veil://")) {
    return new URL(raw);
  }
  if (lower.startsWith("vps:")) {
    return new URL(`veil://${raw.slice(4)}`);
  }
  if (lower.startsWith("veil:vps:")) {
    return new URL(`veil://${raw.slice(9)}`);
  }
  return null;
}
