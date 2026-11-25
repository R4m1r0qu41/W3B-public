/**
 * CipherLayer injected provider (EIP-1193 shape) + EIP-6963 discovery.
 *
 * Zone B artifact: this script runs inside an untrusted dapp capsule. It has
 * zero authority — every request crosses to the broker (Zone C) via the
 * host-supplied `__cipherlayerBrokerPost` bridge, and the broker forwards to
 * the wallet over the pinned contract. Nothing here can sign, confirm, or
 * see key material; a hostile page rewriting this object gains nothing the
 * page could not already do by calling the bridge itself.
 *
 * Discovery (ADR-B005): the provider is exposed both as `window.cipherlayer`
 * (back-compat with the fixture dapp) and `window.ethereum` (the majority
 * legacy-detection pattern real dapps still use), and announced via EIP-6963
 * (`eip6963:announceProvider`) so multi-injected-provider discovery UIs
 * (wagmi, RainbowKit, etc.) find it without a rendezvous step — there is no
 * "which of several extensions is CipherLayer" problem inside our own shell,
 * so discovery is zero-interaction by design.
 */
(function () {
  "use strict";

  if (window.cipherlayer) return; // idempotent injection

  const pending = new Map();
  let nextId = 1;

  // The host (webview shell) supplies the bridge and delivers replies via
  // window.__cipherlayerBrokerReply. In a plain page (no host), requests
  // fail cleanly with 4900 (disconnected) instead of hanging.
  window.__cipherlayerBrokerReply = function (id, result, error) {
    const entry = pending.get(id);
    if (!entry) return;
    pending.delete(id);
    if (error) entry.reject(Object.assign(new Error(error.message), { code: error.code }));
    else entry.resolve(result);
  };

  function request(args) {
    if (!args || typeof args.method !== "string") {
      return Promise.reject(Object.assign(new Error("invalid request"), { code: -32602 }));
    }
    if (typeof window.__cipherlayerBrokerPost !== "function") {
      return Promise.reject(
        Object.assign(new Error("wallet transport disconnected"), { code: 4900 })
      );
    }
    return new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      window.__cipherlayerBrokerPost(
        JSON.stringify({ id, method: args.method, params: args.params ?? [] })
      );
    });
  }

  const provider = Object.freeze({
    isCipherLayer: true,
    request,
    // Legacy niceties some dapps still probe for:
    on: function () { return this; },
    removeListener: function () { return this; },
  });

  // window.cipherlayer: back-compat with the fixture dapp and any
  // direct-access dapp code. window.ethereum: the majority legacy-detection
  // pattern (ethers' BrowserProvider(window.ethereum), wagmi/viem's
  // "injected" connector, plain window.ethereum.request(...)). Only claimed
  // if unclaimed — CipherLayer never displaces another injected provider.
  window.cipherlayer = provider;
  if (!window.ethereum) window.ethereum = provider;

  // --- EIP-6963 multi-provider discovery -------------------------------
  // https://eips.ethereum.org/EIPS/eip-6963
  function uuidv4() {
    if (window.crypto && typeof window.crypto.randomUUID === "function") {
      return window.crypto.randomUUID();
    }
    // Fallback v4 generator (RFC 4122 §4.4) for hosting webviews without
    // crypto.randomUUID.
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      const v = c === "x" ? r : (r & 0x3) | 0x8;
      return v.toString(16);
    });
  }

  const ICON =
    "data:image/svg+xml;base64," +
    btoa(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">' +
        '<rect width="32" height="32" rx="6" fill="#1b1f3b"/>' +
        '<path d="M16 6l9 5v10l-9 5-9-5V11z" fill="#6ee7ff"/>' +
        "</svg>"
    );

  const info = Object.freeze({
    // Fresh per page load — identifies this injected instance, not the
    // wallet product itself (that's `rdns`).
    uuid: uuidv4(),
    name: "CipherLayer",
    icon: ICON,
    rdns: "org.cipherlayer.browser",
  });

  function announce() {
    window.dispatchEvent(
      new CustomEvent("eip6963:announceProvider", {
        detail: Object.freeze({ info, provider }),
      })
    );
  }

  // Re-announce on every request — a discovery UI that starts listening
  // after injection must still find us (EIP-6963 requirement).
  window.addEventListener("eip6963:requestProvider", announce);
  announce();
})();
