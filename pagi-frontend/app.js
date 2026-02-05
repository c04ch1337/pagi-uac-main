async function runAutonomousGoal() {
  const intent = document.getElementById("intent")?.value ?? "";
  const tenantId = document.getElementById("tenant")?.value ?? "default";
  const out = document.getElementById("out");
  if (out) out.textContent = "Running...";

  const body = {
    tenant_id: tenantId,
    goal: {
      AutonomousGoal: {
        intent,
        context: null,
      },
    },
  };

  const res = await fetch("/v1/execute", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });

  const text = await res.text();
  if (out) out.textContent = text;
}

document.getElementById("run")?.addEventListener("click", () => {
  runAutonomousGoal().catch((e) => {
    const out = document.getElementById("out");
    if (out) out.textContent = String(e);
  });
});

