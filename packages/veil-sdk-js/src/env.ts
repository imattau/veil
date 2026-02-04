export function isReactNativeRuntime(): boolean {
  // React Native sets navigator.product = 'ReactNative'.
  return typeof navigator !== "undefined" && navigator.product === "ReactNative";
}

export function isBrowserRuntime(): boolean {
  return typeof window !== "undefined" && typeof document !== "undefined";
}

export function isNodeRuntime(): boolean {
  const g = globalThis as { process?: { versions?: { node?: string } } };
  return !!g.process?.versions?.node;
}
