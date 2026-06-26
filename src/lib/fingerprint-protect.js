/**
 * fingerprint-protect.js
 *
 * Injected as a Tauri initialization_script into every browser page.
 * Runs in the page's JS context before any page scripts.
 *
 * Protections:
 *   1. Canvas API noise         (breaks canvas fingerprinting)
 *   2. AudioContext noise       (breaks audio fingerprinting)
 *   3. WebGL vendor/renderer    (spoofed to generic Intel)
 *   4. Navigator properties     (hardwareConcurrency, deviceMemory, platform)
 *   5. Screen dimensions        (normalised to 1920×1080)
 *   6. Performance timing noise (breaks timing side-channels)
 *   7. Font enumeration block   (document.fonts size randomised)
 *   8. Battery API block        (always returns fake full battery)
 *   9. Referrer policy          (Referer header stripped on cross-origin)
 *  10. WebRTC IP leak block     (RTCPeerConnection intercepted)
 */

(function (w, d, n) {
  "use strict";

  // ── Tiny PRNG seeded per-session (not per-page) so noise is stable
  // within a session but changes between restarts.
  const _seed = (Math.random() * 0x7fffffff) | 0;
  function _rng(x) {
    let s = (x ^ _seed) >>> 0;
    s = Math.imul(s ^ (s >>> 16), 0x45d9f3b);
    s = Math.imul(s ^ (s >>> 16), 0x45d9f3b);
    s = (s ^ (s >>> 16)) >>> 0;
    return (s / 0x100000000);
  }
  let _rngi = 0;
  const rnd  = () => _rng(_rngi++);
  const rnd1 = () => (rnd() - 0.5) * 2; // −1 to +1

  // ── 1. Canvas fingerprint noise ─────────────────────────────────────
  try {
    const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
    const origToBlob    = HTMLCanvasElement.prototype.toBlob;
    const origGetImgData = CanvasRenderingContext2D.prototype.getImageData;

    function noiseCanvas(canvas) {
      const ctx = canvas.getContext("2d");
      if (!ctx || canvas.width === 0 || canvas.height === 0) return;
      // Add sub-pixel noise to 1 in every 200 pixels (imperceptible visually)
      try {
        const id = origGetImgData.call(ctx, 0, 0, canvas.width, canvas.height);
        const data = id.data;
        const step = 200;
        for (let i = 0; i < data.length; i += step * 4) {
          const delta = (rnd() < 0.5 ? 1 : -1);
          data[i] = Math.max(0, Math.min(255, data[i] + delta));
        }
        ctx.putImageData(id, 0, 0);
      } catch {}
    }

    HTMLCanvasElement.prototype.toDataURL = function (type, quality) {
      noiseCanvas(this);
      return origToDataURL.call(this, type, quality);
    };

    HTMLCanvasElement.prototype.toBlob = function (cb, type, quality) {
      noiseCanvas(this);
      origToBlob.call(this, cb, type, quality);
    };
  } catch {}

  // ── 2. AudioContext fingerprint noise ────────────────────────────────
  try {
    const AudioCtx = w.AudioContext || w.webkitAudioContext;
    if (AudioCtx) {
      const origGetChannelData = AudioBuffer.prototype.getChannelData;
      AudioBuffer.prototype.getChannelData = function (channel) {
        const data = origGetChannelData.call(this, channel);
        // Add imperceptible noise (< -96 dB)
        for (let i = 0; i < data.length; i += 100) {
          data[i] += rnd1() * 0.0000005;
        }
        return data;
      };

      // AnalyserNode.getFloatFrequencyData
      const origGetFloat = AnalyserNode.prototype.getFloatFrequencyData;
      AnalyserNode.prototype.getFloatFrequencyData = function (arr) {
        origGetFloat.call(this, arr);
        for (let i = 0; i < arr.length; i += 10) {
          arr[i] += rnd1() * 0.001;
        }
      };
    }
  } catch {}

  // ── 3. WebGL vendor / renderer spoofing ──────────────────────────────
  try {
    const origGetParameter = WebGLRenderingContext.prototype.getParameter;
    WebGLRenderingContext.prototype.getParameter = function (param) {
      // UNMASKED_VENDOR_WEBGL / UNMASKED_RENDERER_WEBGL
      if (param === 37445) return "Intel Inc.";
      if (param === 37446) return "Intel(R) UHD Graphics";
      return origGetParameter.call(this, param);
    };

    // WebGL2 same
    if (w.WebGL2RenderingContext) {
      const orig2 = WebGL2RenderingContext.prototype.getParameter;
      WebGL2RenderingContext.prototype.getParameter = function (param) {
        if (param === 37445) return "Intel Inc.";
        if (param === 37446) return "Intel(R) UHD Graphics";
        return orig2.call(this, param);
      };
    }
  } catch {}

  // ── 4. Navigator property spoofing ───────────────────────────────────
  try {
    const navProps = {
      hardwareConcurrency: 4,
      deviceMemory:        8,
      platform:            "Win32",
      maxTouchPoints:      0,
      languages:           ["en-US", "en"],
      language:            "en-US",
    };
    for (const [k, v] of Object.entries(navProps)) {
      try {
        Object.defineProperty(n, k, {
          get: () => v,
          configurable: true,
          enumerable: true,
        });
      } catch {}
    }
  } catch {}

  // ── 5. Screen dimensions normalisation ───────────────────────────────
  try {
    const screenProps = {
      width:       1920,
      height:      1080,
      availWidth:  1920,
      availHeight: 1040,
      colorDepth:  24,
      pixelDepth:  24,
    };
    for (const [k, v] of Object.entries(screenProps)) {
      try {
        Object.defineProperty(w.screen, k, {
          get: () => v,
          configurable: true,
        });
      } catch {}
    }
    try {
      Object.defineProperty(w, "devicePixelRatio", { get: () => 1, configurable: true });
    } catch {}
  } catch {}

  // ── 6. Performance timing noise ──────────────────────────────────────
  try {
    const origNow = performance.now.bind(performance);
    performance.now = function () {
      // Round to nearest 100 µs — reduces timing precision
      return Math.round(origNow() * 10) / 10 + rnd() * 0.1;
    };
  } catch {}

  // ── 7. Font enumeration — randomise FontFaceSet.size ─────────────────
  try {
    if (d.fonts && typeof d.fonts[Symbol.iterator] === "function") {
      const origSize = Object.getOwnPropertyDescriptor(FontFaceSet.prototype, "size");
      if (origSize) {
        Object.defineProperty(FontFaceSet.prototype, "size", {
          get() { return (origSize.get.call(this) | 0) + (Math.random() > 0.5 ? 1 : 0); },
          configurable: true,
        });
      }
    }
  } catch {}

  // ── 8. Battery API — always return a fake full battery ───────────────
  try {
    if (n.getBattery) {
      n.getBattery = () => Promise.resolve({
        charging:        true,
        chargingTime:    0,
        dischargingTime: Infinity,
        level:           1.0,
        addEventListener:    () => {},
        removeEventListener: () => {},
      });
    }
  } catch {}

  // ── 9. Referrer policy — strip on cross-origin ───────────────────────
  try {
    // Inject a meta referrer policy if the page hasn't set one
    d.addEventListener("DOMContentLoaded", () => {
      if (!d.querySelector('meta[name="referrer"]')) {
        const m = d.createElement("meta");
        m.name    = "referrer";
        m.content = "strict-origin-when-cross-origin";
        d.head?.appendChild(m);
      }
    }, { once: true });
  } catch {}

  // ── 10. WebRTC IP leak block ─────────────────────────────────────────
  try {
    // Override RTCPeerConnection to prevent local IP exposure
    const OrigRTC = w.RTCPeerConnection;
    if (OrigRTC) {
      w.RTCPeerConnection = function (config, constraints) {
        // Strip mDNS / local ICE servers, keep only relay
        if (config?.iceServers) {
          config.iceServers = config.iceServers.filter(s => {
            const urls = Array.isArray(s.urls) ? s.urls : [s.urls];
            return !urls.some(u => typeof u === "string" && u.startsWith("stun:"));
          });
        }
        return new OrigRTC(config, constraints);
      };
      Object.assign(w.RTCPeerConnection, OrigRTC);
      w.RTCPeerConnection.prototype = OrigRTC.prototype;
    }
  } catch {}

})(window, document, navigator);
