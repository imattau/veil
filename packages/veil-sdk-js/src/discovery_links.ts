export function buildVpsDiscoveryLink(options: {
  wsEndpoints?: string[];
  quicEndpoint?: string;
  quicCertHex?: string;
  quicCertB64?: string;
  peers?: string[];
  tags?: string[];
}): string {
  const params = new URLSearchParams();
  const ws = dedupe(options.wsEndpoints ?? []);
  for (const endpoint of ws) {
    if (endpoint) params.append("ws", endpoint);
  }
  if (options.quicEndpoint) {
    params.set("quic", options.quicEndpoint);
  }
  if (options.quicCertB64) {
    params.set("certb64", options.quicCertB64);
  } else if (options.quicCertHex) {
    params.set("cert", options.quicCertHex);
  }
  for (const peer of dedupe(options.peers ?? [])) {
    if (peer) params.append("peer", peer);
  }
  for (const tag of dedupe(options.tags ?? [])) {
    if (tag) params.append("tag", tag);
  }
  const query = params.toString();
  return query ? `veil://vps?${query}` : "veil://vps";
}

function dedupe(values: string[]): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const value of values) {
    if (!seen.has(value)) {
      seen.add(value);
      out.push(value);
    }
  }
  return out;
}
