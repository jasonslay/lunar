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
  const coarsePointer = window.matchMedia("(hover: none) and (pointer: coarse)").matches;
  let selectionFrame = 0;

  function syncThrust() {
    state.main = 0;
    state.tilt_left = 0;
    state.tilt_right = 0;
    for (const thrust of activeThrust.values()) {
      if (thrust === "main") state.main = 1;
      if (thrust === "left") state.tilt_left = 1;
      if (thrust === "right") state.tilt_right = 1;
    }

    document.documentElement.classList.toggle("lunar-thrust-active", activeThrust.size > 0);

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

  function clearSelection() {
    const selection = window.getSelection?.();
    if (selection && selection.rangeCount > 0) {
      selection.removeAllRanges();
    }
  }

  function preventTouchDefaults(event) {
    event.preventDefault();
    clearSelection();
  }

  function inTouchZone(target) {
    return (
      target &&
      target.closest &&
      (target.closest("#touch-controls") || target.closest(".game-shell"))
    );
  }

  function startSelectionGuard() {
    if (selectionFrame) return;
    const tick = () => {
      clearSelection();
      if (activeThrust.size > 0) {
        selectionFrame = requestAnimationFrame(tick);
      } else {
        selectionFrame = 0;
      }
    };
    selectionFrame = requestAnimationFrame(tick);
  }

  function bindGlobalTouchLock() {
    const captureOptions = { capture: true, passive: false };

    if (coarsePointer) {
      document.documentElement.classList.add("lunar-mobile");

      const blockBrowserDefaults = (event) => {
        preventTouchDefaults(event);
      };

      document.addEventListener("touchstart", blockBrowserDefaults, captureOptions);
      document.addEventListener("touchmove", blockBrowserDefaults, captureOptions);
      document.addEventListener("contextmenu", blockBrowserDefaults, captureOptions);
      document.addEventListener("selectstart", blockBrowserDefaults, captureOptions);

      document.addEventListener(
        "selectionchange",
        () => {
          clearSelection();
        },
        { passive: true }
      );

      return;
    }

    document.addEventListener(
      "touchstart",
      (event) => {
        if (inTouchZone(event.target)) {
          preventTouchDefaults(event);
        }
      },
      captureOptions
    );

    document.addEventListener(
      "touchmove",
      (event) => {
        if (activeThrust.size > 0 || inTouchZone(event.target)) {
          preventTouchDefaults(event);
        }
      },
      captureOptions
    );

    document.addEventListener(
      "touchend",
      () => {
        clearSelection();
      },
      { capture: true, passive: true }
    );

    document.addEventListener(
      "selectstart",
      (event) => {
        if (inTouchZone(event.target) || activeThrust.size > 0) {
          preventTouchDefaults(event);
        }
      },
      captureOptions
    );

    document.addEventListener(
      "contextmenu",
      (event) => {
        if (inTouchZone(event.target) || activeThrust.size > 0) {
          preventTouchDefaults(event);
        }
      },
      captureOptions
    );

    document.addEventListener(
      "selectionchange",
      () => {
        if (activeThrust.size > 0) {
          clearSelection();
        }
      },
      { passive: true }
    );
  }

  function bindThrustButtonTouch(button) {
    const thrust = button.dataset.thrust;
    if (!thrust) return;

    const press = (event) => {
      preventTouchDefaults(event);
      for (const touch of event.changedTouches) {
        activeThrust.set(touch.identifier, thrust);
      }
      syncThrust();
      startSelectionGuard();
    };

    const release = (event) => {
      preventTouchDefaults(event);
      for (const touch of event.changedTouches) {
        activeThrust.delete(touch.identifier);
      }
      syncThrust();
      clearSelection();
    };

    button.addEventListener("touchstart", press, { passive: false });
    button.addEventListener("touchmove", preventTouchDefaults, { passive: false });
    button.addEventListener("touchend", release, { passive: false });
    button.addEventListener("touchcancel", release, { passive: false });
  }

  function bindThrustButtonPointer(button) {
    const thrust = button.dataset.thrust;
    if (!thrust) return;

    button.addEventListener("pointerdown", (event) => {
      preventTouchDefaults(event);
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
      clearSelection();
    };

    button.addEventListener("pointerup", release);
    button.addEventListener("pointercancel", release);
    button.addEventListener("lostpointercapture", release);
  }

  function bindActionButton(button) {
    const action = button.dataset.action;
    if (!action) return;

    const fire = (event) => {
      preventTouchDefaults(event);
      clearPendingActions();
      if (action === "autopilot") state.toggle_autopilot = true;
      if (action === "reset") state.reset = true;
      if (action === "new") state.new_level = true;
    };

    if (coarsePointer) {
      let armed = false;

      button.addEventListener(
        "touchstart",
        (event) => {
          preventTouchDefaults(event);
          armed = true;
        },
        { passive: false }
      );

      button.addEventListener(
        "touchend",
        (event) => {
          if (!armed) return;
          armed = false;
          fire(event);
        },
        { passive: false }
      );

      button.addEventListener(
        "touchcancel",
        () => {
          armed = false;
        },
        { passive: true }
      );
    } else {
      button.addEventListener("click", fire);
    }
  }

  function init() {
    window.__lunarTouch = state;
    bindGlobalTouchLock();

    document.querySelectorAll("[data-thrust]").forEach((button) => {
      if (coarsePointer) {
        bindThrustButtonTouch(button);
      } else {
        bindThrustButtonPointer(button);
      }
    });
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
