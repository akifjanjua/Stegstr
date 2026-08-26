import { describe, it, expect, beforeAll } from "vitest";
import { webcrypto } from "node:crypto";

// Polyfill crypto.subtle for Node.js test environment
beforeAll(() => {
  if (!globalThis.crypto?.subtle) {
    Object.defineProperty(globalThis, "crypto", {
      value: webcrypto,
      writable: true,
    });
  }
});

describe("verifyEvent", () => {
  it("accepts a genuinely signed event", async () => {
    const { finishEventAsync, generateSecretKey, verifyEvent } = await import("../nostr-stub");
    const sk = generateSecretKey();
    const ev = await finishEventAsync({ kind: 1, content: "hello", tags: [], created_at: 1700000000 }, sk);
    expect(await verifyEvent(ev)).toBe(true);
  });

  it("rejects an event whose content was tampered with after signing", async () => {
    const { finishEventAsync, generateSecretKey, verifyEvent } = await import("../nostr-stub");
    const sk = generateSecretKey();
    const ev = await finishEventAsync({ kind: 1, content: "hello", tags: [], created_at: 1700000000 }, sk);
    const tampered = { ...ev, content: "attacker-controlled content" };
    expect(await verifyEvent(tampered)).toBe(false);
  });

  it("rejects an event with a forged id that doesn't match its content", async () => {
    const { finishEventAsync, generateSecretKey, verifyEvent } = await import("../nostr-stub");
    const sk = generateSecretKey();
    const ev = await finishEventAsync({ kind: 1, content: "hello", tags: [], created_at: 1700000000 }, sk);
    const tampered = { ...ev, id: "0".repeat(64) };
    expect(await verifyEvent(tampered)).toBe(false);
  });

  it("rejects an event with a garbage signature", async () => {
    const { finishEventAsync, generateSecretKey, verifyEvent } = await import("../nostr-stub");
    const sk = generateSecretKey();
    const ev = await finishEventAsync({ kind: 1, content: "hello", tags: [], created_at: 1700000000 }, sk);
    const tampered = { ...ev, sig: "ab".repeat(64) };
    expect(await verifyEvent(tampered)).toBe(false);
  });

  it("rejects an event claiming a pubkey it wasn't actually signed by (impersonation)", async () => {
    const { finishEventAsync, generateSecretKey, getPublicKey, verifyEvent } = await import("../nostr-stub");
    const attackerSk = generateSecretKey();
    const victimSk = generateSecretKey();
    const victimPubkey = getPublicKey(victimSk);
    // Attacker signs with their own key, then relabels the event as the victim's.
    const ev = await finishEventAsync({ kind: 1, content: "I never said this", tags: [], created_at: 1700000000 }, attackerSk);
    const impersonated = { ...ev, pubkey: victimPubkey };
    expect(await verifyEvent(impersonated)).toBe(false);
  });

  it("rejects malformed events without throwing", async () => {
    const { verifyEvent } = await import("../nostr-stub");
    expect(await verifyEvent({ id: "", pubkey: "", created_at: 0, kind: 1, tags: [], content: "", sig: "" })).toBe(false);
    expect(
      await verifyEvent({
        id: "not-hex",
        pubkey: "0".repeat(64),
        created_at: 1700000000,
        kind: 1,
        tags: [],
        content: "x",
        sig: "0".repeat(128),
      })
    ).toBe(false);
  });
});
