export function rgbToCmyk(r, g, b) {
  const r1 = r / 255;
  const g1 = g / 255;
  const b1 = b / 255;
  const k = 1 - Math.max(r1, g1, b1);

  if (k >= 0.999) {
    return { c: 0, m: 0, y: 0, k: 1 };
  }

  return {
    c: clamp01((1 - r1 - k) / (1 - k)),
    m: clamp01((1 - g1 - k) / (1 - k)),
    y: clamp01((1 - b1 - k) / (1 - k)),
    k: clamp01(k),
  };
}

export function luminance(r, g, b) {
  // Rec. 709 luma; returned as darkness so 1 means strong/dark mark.
  const lightness = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  return clamp01(1 - lightness);
}

export function mapPixelToChannels(pixel, mode, singleChannel = "k", enabledChannels = null) {
  const [r, g, b, a] = pixel;
  if (a === 0) {
    return { c: 0, m: 0, y: 0, k: 0 };
  }

  if (mode === "cmyk") {
    return rgbToCmyk(r, g, b);
  }

  const dark = luminance(r, g, b);

  if (mode === "crosshatch-luminance") {
    return crosshatchLuminanceChannels(dark, enabledChannels);
  }

  if (mode === "inverted-luminance") {
    const inverted = 1 - dark;
    return { c: inverted, m: inverted, y: inverted, k: inverted };
  }

  if (mode === "single-channel") {
    return {
      c: singleChannel === "c" ? dark : 0,
      m: singleChannel === "m" ? dark : 0,
      y: singleChannel === "y" ? dark : 0,
      k: singleChannel === "k" ? dark : 0,
    };
  }

  return { c: dark, m: dark, y: dark, k: dark };
}

function crosshatchLuminanceChannels(darkness, enabledChannels) {
  const layerOrder = ["k", "c", "m", "y"].filter((channel) =>
    Array.isArray(enabledChannels) ? enabledChannels.includes(channel) : true,
  );
  if (layerOrder.length === 0) return { c: 0, m: 0, y: 0, k: 0 };

  const layerSpan = 1 / layerOrder.length;
  const normalizedDarkness = snapUnitInterval(darkness);
  const values = { c: 0, m: 0, y: 0, k: 0 };

  layerOrder.forEach((channel, index) => {
    values[channel] = snapUnitInterval(
      Math.min(layerSpan, Math.max(0, normalizedDarkness - index * layerSpan)),
    );
  });

  return values;
}

function snapUnitInterval(value) {
  const clamped = clamp01(value);
  if (clamped <= 1e-12) return 0;
  if (clamped >= 1 - 1e-12) return 1;
  return clamped;
}

export function clamp01(value) {
  return Math.min(1, Math.max(0, Number.isFinite(value) ? value : 0));
}
