(() => {
  const state = {
    main: 0,
    tilt_left: 0,
    tilt_right: 0,
    toggle_autopilot: false,
    reset: false,
    new_level: false,
  };

  const activeThrust = new Map();

  function syncThrust() {
    state.main = 0;
    state.tilt_left = 0;
    state.tilt_right = 0;
    for (const thrust of activeThrust.values()) {
      if (thrust === "main") state.main = 1;
      if (thrust === "left") state.tilt_left = 1;
      if (thrust === "right") state.tilt_right = 1;
    }

    document.querySelectorAll("[data-thrust]").forEach((button) => {
      const thrust = button.dataset.thrust;
      const active = [...activeThrust.values()].includes(thrust);
      button.classList.toggle("is-active", active);
    });
  }

  function clearPendingActions() {
    state.toggle_autopilot = false;
    state.reset = false;
    state.new_level = false;
  }

  function bindThrustButton(button) {
    const thrust = button.dataset.thrust;
    if (!thrust) return;

    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      activeThrust.set(event.pointerId, thrust);
      syncThrust();
    });

    const release = (event) => {
      if (activeThrust.delete(event.pointerId)) {
        syncThrust();
      }
      if (button.hasPointerCapture(event.pointerId)) {
        button.releasePointerCapture(event.pointerId);
      }
    };

    button.addEventListener("pointerup", release);
    button.addEventListener("pointercancel", release);
    button.addEventListener("lostpointercapture", release);
  }

  function bindActionButton(button) {
    const action = button.dataset.action;
    if (!action) return;

    button.addEventListener("click", (event) => {
      event.preventDefault();
      clearPendingActions();
      if (action === "autopilot") state.toggle_autopilot = true;
      if (action === "reset") state.reset = true;
      if (action === "new") state.new_level = true;
    });
  }

  function init() {
    window.__lunarTouch = state;

    document.querySelectorAll("[data-thrust]").forEach(bindThrustButton);
    document.querySelectorAll("[data-action]").forEach(bindActionButton);

    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") {
        activeThrust.clear();
        syncThrust();
      }
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
