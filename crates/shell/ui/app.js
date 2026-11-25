// CipherLayer Shell evidence panel (trusted shell chrome, Zone C).
// Polls the host's display-state endpoint over the label-gated bridge.
// Display only: nothing here can approve, sign, or talk to the vault.
"use strict";

const $ = (id) => document.getElementById(id);

function riskClass(level) {
  if (!level) return "";
  const l = level.toLowerCase();
  if (l.startsWith("high") || l.startsWith("critical")) return "risk-high";
  if (l.startsWith("medium")) return "risk-medium";
  if (l.startsWith("low")) return "risk-low";
  return "risk-unknown";
}

function stateBadge(state) {
  if (!state) return ["none yet", ""];
  if (state === "CONFIRMED" || state === "BROADCAST") return [state, "ok"];
  if (state === "POLICY_DENIED" || state === "BLOCKED") return [state, "bad"];
  return [state, "warn"];
}

function setList(el, items, render) {
  el.replaceChildren();
  for (const item of items) {
    const li = document.createElement("li");
    render(li, item);
    el.appendChild(li);
  }
}

function renderStatus(s) {
  $("st-transport").textContent = s.wallet_protocol
    ? `connected (protocol v${s.wallet_protocol}) — ${s.socket}`
    : `not connected — ${s.socket}`;
  $("st-attested").textContent = s.attested
    ? "attested as launched host (v0.3)"
    : "not attested";
  $("st-capsule").textContent = `${s.capsule_origin} (${s.capsule_id}, host-assigned)`;
  $("st-chain").textContent = `${s.chain_id} (local Anvil dev chain)`;
  $("st-accounts").textContent = s.accounts.length ? s.accounts.join(", ") : "none granted";
  $("nav-current").textContent = s.capsule_url || "—";
}

function renderReview(v) {
  const [label, cls] = stateBadge(v && v.state);
  const badge = $("rv-state");
  badge.textContent = label;
  badge.className = "badge " + cls;
  $("rv-empty").hidden = !!v;
  $("rv-body").hidden = !v;
  if (!v) return;

  const ev = v.evidence;
  $("rv-action").textContent = ev ? ev.decoded_action : "(no evidence attached)";
  $("rv-confidence").textContent = ev ? ev.intent_confidence : "—";
  $("rv-simulation").textContent = (ev && ev.simulation) || "—";
  const risk = $("rv-risk");
  risk.textContent = (ev && ev.risk_level) || "—";
  risk.className = ev ? riskClass(ev.risk_level) : "";
  const verdict = $("rv-verdict");
  verdict.textContent = (ev && ev.policy_verdict) || "—";
  verdict.className = ev && ev.policy_verdict ? "verdict-" + ev.policy_verdict.toLowerCase() : "";

  setList($("rv-reasons"), (ev && ev.policy_reasons) || [], (li, r) => (li.textContent = r));
  $("rv-reasons-wrap").hidden = !ev || ev.policy_reasons.length === 0;

  setList($("rv-findings"), (ev && ev.risk_findings) || [], (li, f) => {
    li.className = "finding";
    const level = document.createElement("span");
    level.className = riskClass(f.risk_level);
    level.textContent = f.risk_level;
    const codes = document.createElement("span");
    codes.className = "codes";
    codes.textContent = f.reason_codes.length ? ` [${f.reason_codes.join(", ")}] ` : " ";
    li.append(`${f.detector}: `, level, codes, f.explanation);
  });
  $("rv-findings-wrap").hidden = !ev || ev.risk_findings.length === 0;

  setList($("rv-unavailable"), (ev && ev.detectors_unavailable) || [], (li, d) => (li.textContent = d));
  $("rv-unavailable-wrap").hidden = !ev || ev.detectors_unavailable.length === 0;

  $("rv-refusals").textContent = v.refusal_codes.length ? v.refusal_codes.join(", ") : "—";
  $("rv-txhash").textContent = v.tx_hash || "—";
  $("rv-audit").textContent = `#${v.audit_head_sequence} (hash-chained wallet audit log)`;
}

function renderActivity(entries) {
  const items = entries.slice().reverse();
  setList($("activity"), items, (li, e) => {
    const t = document.createElement("time");
    t.textContent = e.at.replace(/^.*T/, "").replace(/\..*$/, "");
    li.append(t, e.line);
  });
}

async function tick() {
  try {
    const res = await fetch("clbridge://localhost/state");
    const s = await res.json();
    renderStatus(s);
    renderReview(s.last_signing);
    renderActivity(s.activity);
  } catch (e) {
    $("st-transport").textContent = "bridge unreachable: " + e;
  }
}

$("nav-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const url = $("nav-url").value.trim();
  const err = $("nav-error");
  err.hidden = true;
  if (!url) return;
  try {
    const res = await fetch("clbridge://localhost/navigate", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ url }),
    });
    if (!res.ok) {
      err.textContent = await res.text();
      err.hidden = false;
      return;
    }
    $("nav-url").value = "";
  } catch (e2) {
    err.textContent = "bridge unreachable: " + e2;
    err.hidden = false;
  }
});

tick();
setInterval(tick, 700);
